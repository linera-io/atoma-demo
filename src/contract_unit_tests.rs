// Copyright (c) Zefchain Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

use std::{
    collections::{BTreeSet, HashSet},
    iter,
};

use atoma_demo::{ChatInteraction, Operation, PublicKey};
use linera_sdk::{base::ApplicationId, util::BlockingWait, Contract, ContractRuntime};
use proptest::{
    prelude::{Arbitrary, BoxedStrategy},
    sample::size_range,
    strategy::Strategy,
};
use rand::Rng;
use test_strategy::proptest;

use super::ApplicationContract;

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

/// Tests if chat interactions are logged on chain.
#[proptest]
fn chat_interactions_are_logged_on_chain(interactions: Vec<ChatInteraction>) {
    let mut contract = setup_contract();

    for interaction in interactions.clone() {
        contract
            .execute_operation(Operation::LogChatInteraction { interaction })
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
