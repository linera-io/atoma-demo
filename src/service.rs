// Copyright (c) Zefchain Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

#![cfg_attr(target_arch = "wasm32", no_main)]

mod state;
#[cfg(test)]
#[path = "./service_unit_tests.rs"]
mod tests;

use std::sync::{Arc, Mutex};

use async_graphql::{connection::EmptyFields, EmptySubscription, Schema};
use atoma_demo::{ChatInteraction, Operation};
use linera_sdk::{base::WithServiceAbi, bcs, ensure, http, Service, ServiceRuntime};
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
        model: Option<String>,
        max_tokens: Option<usize>,
        atoma_proxy_url: Option<String>,
    ) -> async_graphql::Result<Vec<u8>> {
        let request = ChatCompletionRequest {
            stream: false,
            messages: &[&message],
            model: model.unwrap_or_else(|| "meta-llama/Llama-3.3-70B-Instruct".to_owned()),
            max_tokens: max_tokens.unwrap_or(128),
        };

        let response = self.query_chat_completion(
            atoma_proxy_url.as_deref().unwrap_or(ATOMA_CLOUD_URL),
            &api_token,
            &request,
        )?;

        let interaction = ChatInteractionResponse::parse_from_completion_response(response)?
            .with_prompt(message.content);

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

impl Mutation {
    /// Queries the Atoma network for a chat completion.
    fn query_chat_completion(
        &self,
        base_url: &str,
        api_token: &str,
        request: &ChatCompletionRequest,
    ) -> async_graphql::Result<ChatCompletionResponse> {
        let mut runtime = self
            .runtime
            .lock()
            .expect("Locking should never fail because service runs in a single thread");

        let body = serde_json::to_vec(request)?;

        let response = runtime.http_request(
            http::Request::post(format!("{base_url}/v1/chat/completions"), body)
                .with_header("Content-Type", b"application/json")
                .with_header("Authorization", format!("Bearer {api_token}").as_bytes()),
        );

        ensure!(
            response.status == 200,
            async_graphql::Error::new(format!(
                "Failed to perform chat completion API query. Status code: {}",
                response.status
            ))
        );

        serde_json::from_slice::<ChatCompletionResponse>(&response.body).map_err(|error| {
            async_graphql::Error::new(format!(
                "Failed to deserialize chat completion response: {error}\n{:?}",
                String::from_utf8_lossy(&response.body),
            ))
        })
    }
}

/// The POST body to be sent to the chat completion API.
#[derive(Clone, Debug, Serialize)]
pub struct ChatCompletionRequest<'message> {
    stream: bool,
    messages: &'message [&'message ChatMessage],
    model: String,
    max_tokens: usize,
}

/// The response received from the chat completion API.
#[derive(Clone, Debug, Deserialize)]
pub struct ChatCompletionResponse {
    choices: Vec<ChatCompletionChoice>,
}

/// A choice received in the response from a chat completion API.
#[derive(Clone, Debug, Deserialize)]
pub struct ChatCompletionChoice {
    message: ChatMessage,
}

/// Only the response for a [`ChatInteraction`].
#[derive(Clone, Debug)]
pub struct ChatInteractionResponse {
    response: String,
}

impl ChatInteractionResponse {
    /// Parses the first choice from a [`ChatCompletionResponse`] to extract the
    /// [`ChatInteractionResponse`].
    pub fn parse_from_completion_response(
        response: ChatCompletionResponse,
    ) -> async_graphql::Result<Self> {
        ensure!(
            !response.choices.is_empty(),
            async_graphql::Error::new(
                "Chat completion response has an empty `choices` list".to_owned()
            )
        );

        let first_choice = response
            .choices
            .into_iter()
            .next()
            .expect("Response should have at least one choice element");

        Ok(ChatInteractionResponse {
            response: first_choice.message.content,
        })
    }

    /// Builds a [`ChatInteraction`] using this response and the provided `prompt`.
    pub fn with_prompt(self, prompt: String) -> ChatInteraction {
        ChatInteraction {
            prompt,
            response: self.response,
        }
    }
}

/// The base URL to access the Atoma Cloud proxy.
const ATOMA_CLOUD_URL: &str = "https://api.atoma.network";
