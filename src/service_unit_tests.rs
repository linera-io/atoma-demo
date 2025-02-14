// Copyright (c) Zefchain Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

use atoma_demo::{ChatInteraction, Operation};
use linera_sdk::{bcs, http, util::BlockingWait, Service, ServiceRuntime};
use serde_json::json;
use test_strategy::proptest;

use super::{ApplicationService, ATOMA_CLOUD_URL};

/// Tests if `chat` mutations perform an HTTP request to the Atoma proxy, and generates the
/// operation to log a chat interaction.
#[proptest]
fn performs_http_query(
    #[strategy("[A-Za-z0-9%=]*")] api_token: String,
    interaction: ChatInteraction,
) {
    let service = setup_service(ServiceRuntime::new());

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

    service.runtime.lock().unwrap().add_expected_http_request(
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
