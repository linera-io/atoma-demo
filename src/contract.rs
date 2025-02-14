// Copyright (c) Zefchain Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

#![cfg_attr(target_arch = "wasm32", no_main)]

mod state;
#[cfg(test)]
#[path = "./contract_unit_tests.rs"]
mod tests;

use atoma_demo::{ChatInteraction, Operation, PublicKey};
use linera_sdk::{
    base::WithContractAbi,
    views::{RootView, View},
    Contract, ContractRuntime,
};
use serde::{Deserialize, Serialize};

use self::state::Application;

pub struct ApplicationContract {
    state: Application,
    runtime: ContractRuntime<Self>,
}

linera_sdk::contract!(ApplicationContract);

impl WithContractAbi for ApplicationContract {
    type Abi = atoma_demo::ApplicationAbi;
}

impl Contract for ApplicationContract {
    type Message = Message;
    type Parameters = ();
    type InstantiationArgument = ();

    async fn load(runtime: ContractRuntime<Self>) -> Self {
        let state = Application::load(runtime.root_view_storage_context())
            .await
            .expect("Failed to load state");
        ApplicationContract { state, runtime }
    }

    async fn instantiate(&mut self, _argument: Self::InstantiationArgument) {}

    async fn execute_operation(&mut self, operation: Self::Operation) -> Self::Response {
        match operation {
            Operation::UpdateNodes { add, remove } => self.update_nodes(add, remove),
            Operation::LogChatInteraction { interaction } => self.log_chat_interaction(interaction),
        }
    }

    async fn execute_message(&mut self, message: Self::Message) {
        match message {
            Message::VerifySignature(interaction) => self.verify_signature(interaction),
            Message::LogVerifiedChatInteraction(interaction) => {
                self.log_verified_chat_interaction(interaction)
            }
        }
    }

    async fn store(mut self) {
        self.state.save().await.expect("Failed to save state");
    }
}

/// Cross-chain messages sent privately between the application shards.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub enum Message {
    /// Request to verify a [`ChatInteraction`]'s signature.
    VerifySignature(ChatInteraction),

    /// Response indicating that the [`ChatInteraction`]'s signature was verified and approved.
    LogVerifiedChatInteraction(ChatInteraction),
}

impl ApplicationContract {
    /// Handles an [`Operation::UpdateNodes`] by adding the `nodes_to_add` and removing the
    /// `nodes_to_remove`.
    fn update_nodes(&mut self, nodes_to_add: Vec<PublicKey>, nodes_to_remove: Vec<PublicKey>) {
        assert!(
            self.runtime.chain_id() == self.runtime.application_id().creation.chain_id,
            "Only the chain that created the application can manage the set of active nodes"
        );

        Self::assert_key_sets_are_disjoint(&nodes_to_add, &nodes_to_remove);

        for node in nodes_to_remove {
            self.state
                .active_atoma_nodes
                .remove(&node)
                .expect("Failed to remove a node from the set of active Atoma nodes");
        }

        for node in nodes_to_add {
            self.state
                .active_atoma_nodes
                .insert(&node)
                .expect("Failed to add a node to the set of active Atoma nodes");
        }
    }

    /// Checks if two sets of [`PublicKey`]s are disjoint.
    fn assert_key_sets_are_disjoint(left: &[PublicKey], right: &[PublicKey]) {
        let (smallest_set, largest_set) = if left.len() < right.len() {
            (left, right)
        } else {
            (right, left)
        };

        let disjoint = largest_set.iter().all(|key| !smallest_set.contains(key));

        assert!(
            disjoint,
            "Conflicting request to add and remove the same node"
        );
    }

    /// Handles an [`Operation::LogChatInteraction`] by requesting the [`ChatInteraction`]'s
    /// signature to be verified.
    fn log_chat_interaction(&mut self, interaction: ChatInteraction) {
        let creation_chain_id = self.runtime.application_id().creation.chain_id;

        self.runtime
            .send_message(creation_chain_id, Message::VerifySignature(interaction));
    }

    /// Handles a [`Message::VerifySignature`] by verifying the signature and if accepted,
    /// responding with a [`Message::LogVerifiedChatInteraction`].
    fn verify_signature(&mut self, interaction: ChatInteraction) {
        let requester_chain_id = self
            .runtime
            .message_id()
            .expect(
                "`verify_signature` should only be called \
                when handling a `Message::VerifySignature`",
            )
            .chain_id;

        self.runtime.send_message(
            requester_chain_id,
            Message::LogVerifiedChatInteraction(interaction),
        );
    }

    /// Handles a [`Message::LogVerifiedChatInteraction`] by adding the [`ChatInteraction`] to the
    /// chat log.
    fn log_verified_chat_interaction(&mut self, interaction: ChatInteraction) {
        self.state.chat_log.push(interaction);
    }
}
