// Copyright (c) Zefchain Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

#![cfg_attr(target_arch = "wasm32", no_main)]

mod state;

use std::sync::{Arc, Mutex};

use async_graphql::{connection::EmptyFields, EmptySubscription, Schema};
use atoma_demo::{ChatInteraction, Operation};
use linera_sdk::{base::WithServiceAbi, bcs, Service, ServiceRuntime};
use serde::{Deserialize, Serialize};

#[derive(Clone)]
pub struct ApplicationService {
    runtime: Arc<Mutex<ServiceRuntime<Self>>>,
}

linera_sdk::service!(ApplicationService);

impl WithServiceAbi for ApplicationService {
    type Abi = atoma_demo::ApplicationAbi;
}

impl Service for ApplicationService {
    type Parameters = ();

    async fn new(runtime: ServiceRuntime<Self>) -> Self {
        ApplicationService {
            runtime: Arc::new(Mutex::new(runtime)),
        }
    }

    async fn handle_query(&self, query: Self::Query) -> Self::QueryResponse {
        Schema::build(
            EmptyFields,
            Mutation {
                runtime: self.runtime.clone(),
            },
            EmptySubscription,
        )
        .finish()
        .execute(query)
        .await
    }
}

/// Root type that defines all the GraphQL mutations available from the service.
pub struct Mutation {
    runtime: Arc<Mutex<ServiceRuntime<ApplicationService>>>,
}

#[async_graphql::Object]
impl Mutation {
    /// Executes a chat completion using the Atoma Network.
    async fn chat(
        &self,
        api_token: String,
        message: ChatMessage,
    ) -> async_graphql::Result<Vec<u8>> {
        let interaction = ChatInteraction {
            prompt: message.content,
            response: "".to_owned(),
        };

        Ok(
            bcs::to_bytes(&Operation::LogChatInteraction { interaction })
                .expect("`LogChatInteraction` should be serializable"),
        )
    }
}

/// A message to be sent to the AI chat.
#[derive(Clone, Debug, Deserialize, Serialize, async_graphql::InputObject)]
pub struct ChatMessage {
    content: String,
    role: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    name: Option<String>,
}
