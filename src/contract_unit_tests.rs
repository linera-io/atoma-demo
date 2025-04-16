// Copyright (c) Zefchain Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

use std::{
    collections::{BTreeSet, HashSet},
    iter, panic,
};

use atoma_demo::{ChatInteraction, Operation, PublicKey};
use linera_sdk::{
    linera_base_types::{ApplicationId, ChainId, Destination},
    util::BlockingWait,
    Contract, ContractRuntime, Resources, SendMessageRequest,
};
use proptest::{
    prelude::{Arbitrary, BoxedStrategy},
    sample::size_range,
    strategy::Strategy,
};
use rand::Rng;
use test_strategy::proptest;

use super::{ApplicationContract, Message};

/// Tests if nodes can be added to and removed from the set of active Atoma nodes.
#[proptest]
fn updating_nodes(
    application_id: ApplicationId<atoma_demo::ApplicationAbi>,
    test_operations: TestUpdateNodesOperations,
) {
    let mut test = NodeSetTest::new(application_id);

    for test_operation in test_operations.0 {
        let operation = test.prepare_operation(test_operation);

        test.contract.execute_operation(operation).blocking_wait();

        test.check_active_atoma_nodes();
    }
}

/// Tests if the set of active Atoma nodes can only be changed in the chain where the application
/// was created.
#[proptest]
fn only_creation_chain_can_track_nodes(
    application_id: ApplicationId<atoma_demo::ApplicationAbi>,
    chain_id: ChainId,
    test_operation: TestUpdateNodesOperation,
) {
    let result = panic::catch_unwind(move || {
        let mut test = NodeSetTest::new(application_id).with_chain_id(chain_id);
        let operation = test.prepare_operation(test_operation);

        test.contract.execute_operation(operation).blocking_wait();

        test
    });

    match result {
        Ok(test) => {
            assert_eq!(
                chain_id, application_id.creation.chain_id,
                "Contract executed `Operation::UpdateNodes` \
                outside of the application's creation chain"
            );
            test.check_active_atoma_nodes();
        }
        Err(_panic_cause) => {
            assert_ne!(
                chain_id, application_id.creation.chain_id,
                "Contract failed to execute `Operation::UpdateNodes` \
                on the application's creation chain"
            );
        }
    }
}

/// Tests if the contract rejects adding a node twice.
#[proptest]
fn cant_add_and_remove_node_in_the_same_operation(
    application_id: ApplicationId<atoma_demo::ApplicationAbi>,
    #[any(size_range(1..5).lift())] conflicting_nodes: HashSet<PublicKey>,
    mut test_operation: TestUpdateNodesOperation,
) {
    let result = panic::catch_unwind(move || {
        let mut test = NodeSetTest::new(application_id);

        test_operation.add.extend(conflicting_nodes.iter().copied());
        test_operation.remove.extend(conflicting_nodes);

        let operation = test.prepare_operation(test_operation);

        test.contract.execute_operation(operation).blocking_wait();
    });

    assert!(result.is_err());
}

/// Tests if chat interactions are requested to be verified.
#[proptest]
fn chat_interaction_is_requested_to_be_verified(
    application_id: ApplicationId<atoma_demo::ApplicationAbi>,
    interaction: ChatInteraction,
) {
    let mut contract = setup_contract();

    contract.runtime.set_application_id(application_id);

    contract
        .execute_operation(Operation::LogChatInteraction {
            interaction: interaction.clone(),
        })
        .blocking_wait();

    let messages = contract.runtime.created_send_message_requests();

    assert_eq!(messages.len(), 1);
    assert_eq!(
        messages[0],
        SendMessageRequest {
            destination: Destination::Recipient(application_id.creation.chain_id),
            authenticated: false,
            is_tracked: false,
            grant: Resources::default(),
            message: Message::VerifySignature(interaction),
        }
    );
}

/// Tests if chat interactions are logged on chain.
#[proptest]
fn verified_chat_interactions_are_logged_on_chain(interactions: Vec<ChatInteraction>) {
    let mut contract = setup_contract();

    for interaction in interactions.clone() {
        contract
            .execute_message(Message::LogVerifiedChatInteraction(interaction))
            .blocking_wait();
    }

    let logged_interactions = contract
        .state
        .chat_log
        .read(..)
        .blocking_wait()
        .expect("Failed to read logged chat interactions from the state");

    assert_eq!(logged_interactions, interactions);
}

/// Creates a [`ApplicationContract`] instance to be tested.
fn setup_contract() -> ApplicationContract {
    let runtime = ContractRuntime::new();

    ApplicationContract::load(runtime).blocking_wait()
}

/// Helper type with shared code for active Atoma node set tests.
pub struct NodeSetTest {
    contract: ApplicationContract,
    expected_nodes: HashSet<PublicKey>,
}

impl NodeSetTest {
    /// Creates a new [`NodeSetTest`], setting up the contract and the runtime.
    ///
    /// The test configures the contract to run on the specified `chain_id`, or on the
    /// application's creation chain if it's [`None`].
    pub fn new(application_id: ApplicationId<atoma_demo::ApplicationAbi>) -> Self {
        let mut contract = setup_contract();
        let chain_id = application_id.creation.chain_id;

        contract.runtime.set_application_id(application_id);
        contract.runtime.set_chain_id(chain_id);

        NodeSetTest {
            contract,
            expected_nodes: HashSet::new(),
        }
    }

    /// Changes the [`ChainId`] for the chain that executes the contract.
    pub fn with_chain_id(mut self, chain_id: ChainId) -> Self {
        self.contract.runtime.set_chain_id(chain_id);
        self
    }

    /// Prepares an [`Operation::UpdateNodes`] based on the configured
    /// [`TestUpdateNodesOperation`].
    ///
    /// Updates the expected active Atoma nodes state to reflect the execution of the operation.
    pub fn prepare_operation(&mut self, test_operation: TestUpdateNodesOperation) -> Operation {
        let nodes_to_add = test_operation.add;
        let nodes_to_remove = test_operation.remove;

        for node_to_remove in &nodes_to_remove {
            self.expected_nodes.remove(node_to_remove);
        }

        self.expected_nodes.extend(nodes_to_add.iter().copied());

        Operation::UpdateNodes {
            add: nodes_to_add,
            remove: nodes_to_remove,
        }
    }

    /// Asserts that the contract's state has exactly the same nodes as the expected nodes.
    pub fn check_active_atoma_nodes(&self) {
        let node_count = self
            .contract
            .state
            .active_atoma_nodes
            .count()
            .blocking_wait()
            .expect("Failed to read active Atoma node set size");

        let mut active_nodes = HashSet::with_capacity(node_count);
        self.contract
            .state
            .active_atoma_nodes
            .for_each_index(|node| {
                assert!(
                    active_nodes.insert(node),
                    "`SetView` should not have duplicate elements"
                );
                Ok(())
            })
            .blocking_wait()
            .expect("Failed to read active Atoma nodes from state");

        assert_eq!(node_count, self.expected_nodes.len());
        assert_eq!(active_nodes, self.expected_nodes);
    }
}

/// A list of test configurations for a sequence of [`Operation::UpdateNodes`].
#[derive(Clone, Debug)]
pub struct TestUpdateNodesOperations(Vec<TestUpdateNodesOperation>);

impl Arbitrary for TestUpdateNodesOperations {
    type Parameters = ();
    type Strategy = BoxedStrategy<Self>;

    /// Creates an arbitrary [`TestUpdateNodesOperations`].
    ///
    /// This is done by creating a random set of nodes, and partitioning it into an arbitrary
    /// number of operations. After an operation X adds a node for the first time, that node can be
    /// removed at an operation Y > X, and then re-added at an operation Z > Y, and so on.
    fn arbitrary_with((): Self::Parameters) -> Self::Strategy {
        (1_usize..10)
            .prop_flat_map(|operation_count| {
                let node_keys = BTreeSet::<PublicKey>::arbitrary_with(size_range(1..100).lift())
                    .prop_map(Vec::from_iter)
                    .prop_shuffle();

                node_keys.prop_perturb(move |node_keys, mut random| {
                    let mut add_operations = iter::repeat(vec![])
                        .take(operation_count)
                        .collect::<Vec<_>>();
                    let mut remove_operations = add_operations.clone();

                    for node_key in node_keys {
                        let mut is_active = false;
                        let mut index = 0;

                        random.gen_range(0..operation_count);

                        while index < operation_count {
                            if is_active {
                                remove_operations[index].push(node_key);
                            } else {
                                add_operations[index].push(node_key);
                            }

                            is_active = !is_active;
                            index = random.gen_range(index..operation_count) + 1;
                        }
                    }

                    TestUpdateNodesOperations(
                        add_operations
                            .into_iter()
                            .zip(remove_operations)
                            .map(|(add, remove)| TestUpdateNodesOperation { add, remove })
                            .collect(),
                    )
                })
            })
            .boxed()
    }
}

/// The test configuration for an [`Operation::UpdateNodes`].
#[derive(Clone, Debug)]
pub struct TestUpdateNodesOperation {
    add: Vec<PublicKey>,
    remove: Vec<PublicKey>,
}

impl Arbitrary for TestUpdateNodesOperation {
    type Parameters = ();
    type Strategy = BoxedStrategy<Self>;

    /// Creates an arbitrary [`TestUpdateNodesOperation`].
    ///
    /// This is done by creating a random set of nodes, and splitting it in two, one with the nodes
    /// to add and one with the nodes to remove.
    fn arbitrary_with((): Self::Parameters) -> Self::Strategy {
        (..20_usize, ..20_usize)
            .prop_flat_map(|(add_count, remove_count)| {
                BTreeSet::<PublicKey>::arbitrary_with(size_range(add_count + remove_count).lift())
                    .prop_map(Vec::from_iter)
                    .prop_shuffle()
                    .prop_map(move |mut node_keys| {
                        let nodes_to_remove = node_keys.split_off(add_count);
                        let nodes_to_add = node_keys;

                        TestUpdateNodesOperation {
                            add: nodes_to_add,
                            remove: nodes_to_remove,
                        }
                    })
            })
            .boxed()
    }
}
