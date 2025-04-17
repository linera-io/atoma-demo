// Copyright (c) Zefchain Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

use std::{collections::HashSet, sync::Arc};

use atoma_demo::{ChatInteraction, Operation, PublicKey};
use linera_sdk::{
    bcs, http,
    util::BlockingWait,
    views::{RootView, View},
    Service, ServiceRuntime, ViewStorageContext,
};
use serde_json::json;
use test_strategy::proptest;

use super::{state::Application, ApplicationService, ATOMA_CLOUD_URL};

/// Tests if the chat logged on chain can be inspected with GraphQL.
#[proptest]
fn read_chat_log(interactions: Vec<ChatInteraction>) {
    let runtime = ServiceRuntime::new();
    let storage = runtime.key_value_store().to_mut();

    let mut initial_state = Application::load(ViewStorageContext::new_unsafe(storage, vec![], ()))
        .blocking_wait()
        .expect("Failed to load state from mock storage");

    for interaction in interactions.iter().cloned() {
        initial_state.chat_log.push(interaction);
    }

    initial_state
        .save()
        .blocking_wait()
        .expect("Failed to save initial state to mock storage");

    let service = setup_service(runtime);

    let request = async_graphql::Request::new("query { chatLog { entries { prompt, response } } }");

    let response = service.handle_query(request).blocking_wait();

    let async_graphql::Value::Object(response_data) = response.data else {
        panic!("Unexpected response data type");
    };
    let async_graphql::Value::Object(ref chat_log) = response_data["chatLog"] else {
        panic!("Unexpected response chat log type");
    };
    let async_graphql::Value::List(ref entries) = chat_log["entries"] else {
        panic!("Unexpected response entries type");
    };

    let persisted_interactions = entries
        .iter()
        .map(|entry_value| {
            let async_graphql::Value::Object(entry) = entry_value else {
                panic!("Unexpected interaction entry type");
            };
            let async_graphql::Value::String(ref prompt) = entry["prompt"] else {
                panic!("Unexpected interaction prompt type");
            };
            let async_graphql::Value::String(ref response) = entry["response"] else {
                panic!("Unexpected interaction response type");
            };

            ChatInteraction {
                prompt: prompt.clone(),
                response: response.clone(),
            }
        })
        .collect::<Vec<_>>();

    assert_eq!(persisted_interactions, interactions);
}

/// Tests if the set of active Atoma nodes stored on chain can be inspected with GraphQL.
#[proptest]
fn read_active_atoma_nodes(nodes: HashSet<PublicKey>) {
    let runtime = ServiceRuntime::new();
    let storage = runtime.key_value_store().to_mut();

    let mut initial_state = Application::load(ViewStorageContext::new_unsafe(storage, vec![], ()))
        .blocking_wait()
        .expect("Failed to load state from mock storage");

    for node in &nodes {
        initial_state
            .active_atoma_nodes
            .insert(node)
            .expect("Failed to insert node key in initial state");
    }

    initial_state
        .save()
        .blocking_wait()
        .expect("Failed to save initial state to mock storage");

    let service = setup_service(runtime);

    let request = async_graphql::Request::new("query { activeAtomaNodes }");

    let response = service.handle_query(request).blocking_wait();

    let async_graphql::Value::Object(ref response_data) = response.data else {
        panic!("Unexpected response data type");
    };
    let async_graphql::Value::List(ref active_nodes) = response_data["activeAtomaNodes"] else {
        panic!("Unexpected active atoma nodes set type");
    };

    let persisted_nodes = active_nodes
        .iter()
        .map(|node_value| {
            let async_graphql::Value::List(byte_list) = node_value else {
                panic!("Unexpected node entry type");
            };

            let bytes = byte_list
                .iter()
                .map(|byte_value| {
                    let async_graphql::Value::Number(byte_number) = byte_value else {
                        panic!("Unexpected node key byte type");
                    };
                    let byte = byte_number.as_u64().expect("Invalid value for a byte");

                    u8::try_from(byte).expect("Invalid integer for a byte")
                })
                .collect::<Vec<u8>>();

            let byte_array =
                <[u8; 32]>::try_from(&*bytes).expect("Invalid number of bytes for a public key");

            PublicKey::from(byte_array)
        })
        .map(PublicKey::from)
        .collect::<HashSet<_>>();

    assert_eq!(persisted_nodes, nodes);
}

/// Tests if `chat` mutations perform an HTTP request to the Atoma proxy, and generates the
/// operation to log a chat interaction.
#[proptest]
fn performs_http_query(
    #[strategy("[A-Za-z0-9%=]*")] api_token: String,
    interaction: ChatInteraction,
) {
    let mut service = setup_service(ServiceRuntime::new());

    let prompt = &interaction.prompt;
    let request = async_graphql::Request::new(format!(
        "mutation {{ \
            chat(\
                apiToken: \"{api_token}\", \
                message: {{ \
                    content: {prompt:?}, \
                    role: \"user\"
                }}\
            ) \
        }}"
    ));

    let expected_body = format!(
        "{{\
            \"stream\":false,\
            \"messages\":[\
                {{\"content\":{prompt:?},\"role\":\"user\"}}\
            ],\
            \"model\":\"meta-llama/Llama-3.3-70B-Instruct\",\
            \"max_tokens\":128\
        }}"
    );
    let mock_response = format!(
        "{{ \
            \"choices\": [\
                {{
                     \"message\": {{\
                         \"content\": {:?},
                         \"role\": \"\"
                    }}\
                }}\
            ] \
        }}",
        interaction.response
    );

    Arc::get_mut(&mut service.runtime)
        .expect("`ServiceRuntime` should not be shared before configuring expected HTTP requests")
        .add_expected_http_request(
            http::Request::post(
                format!("{ATOMA_CLOUD_URL}/v1/chat/completions"),
                expected_body,
            )
            .with_header("Content-Type", b"application/json")
            .with_header("Authorization", format!("Bearer {api_token}").as_bytes()),
            http::Response::ok(mock_response),
        );

    let response = service.handle_query(request).blocking_wait();

    let expected_operation = Operation::LogChatInteraction { interaction };
    let expected_bytes =
        bcs::to_bytes(&expected_operation).expect("`Operation` should be serializable");
    let expected_response = async_graphql::Response::new(
        async_graphql::Value::from_json(json!({"chat": expected_bytes})).unwrap(),
    );

    assert_eq!(response, expected_response);
}

/// Creates a [`ApplicationService`] instance to be tested.
fn setup_service(runtime: ServiceRuntime<ApplicationService>) -> ApplicationService {
    ApplicationService::new(runtime).blocking_wait()
}
