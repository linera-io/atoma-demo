// Copyright (c) Zefchain Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

#![cfg_attr(target_arch = "wasm32", no_main)]

mod state;
#[cfg(test)]
#[path = "./contract_unit_tests.rs"]
mod tests;

use atoma_demo::{ChatInteraction, Operation};
use linera_sdk::{
    base::WithContractAbi,
    views::{RootView, View},
    Contract, ContractRuntime,
};

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
    type Message = ();
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
        let Operation::LogChatInteraction { interaction } = operation;

        self.log_chat_interaction(interaction);
    }

    async fn execute_message(&mut self, _message: Self::Message) {}

    async fn store(mut self) {
        self.state.save().await.expect("Failed to save state");
    }
}

impl ApplicationContract {
    /// Handles an [`Operation::LogChatInteraction`] by adding a [`ChatInteraction`] to the chat
    /// log.
    fn log_chat_interaction(&mut self, interaction: ChatInteraction) {
        self.state.chat_log.push(interaction);
    }
}
