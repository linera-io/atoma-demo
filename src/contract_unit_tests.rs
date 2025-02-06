// Copyright (c) Zefchain Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

use atoma_demo::{ChatInteraction, Operation};
use linera_sdk::{util::BlockingWait, Contract, ContractRuntime};
use test_strategy::proptest;

use super::ApplicationContract;

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
