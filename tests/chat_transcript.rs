// Copyright (c) Zefchain Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

#![cfg(not(target_arch = "wasm32"))]

use std::env;

use atoma_demo::{ApplicationAbi, ChatInteraction, Operation, PublicKey};
use linera_sdk::{
    bcs,
    test::{QueryOutcome, TestValidator},
};

/// Tests if the service queries the Atoma network when handling a `chat` mutation.
#[test_log::test(tokio::test)]
async fn service_queries_atoma() {
    let (_validator, application_id, chain) =
        TestValidator::with_current_application::<ApplicationAbi, _, _>((), ()).await;

    let api_token = env::var("ATOMA_API_TOKEN")
        .expect("Missing ATOMA_API_TOKEN environment variable to run integration test");

    let query = format!(
        "mutation {{ \
            chat(\
                apiToken: \"{api_token}\", \
                message: {{
                    content: \"What was the capital of Brazil in 1940\",
                    role: \"user\"
                }}\
            ) \
        }}"
    );

    let QueryOutcome { response, .. } = chain.graphql_query(application_id, query).await;

    let response_object = response
        .as_object()
        .expect("Unexpected response from service");

    let operation_list = response_object["chat"]
        .as_array()
        .expect("Unexpected operation representation returned from service");

    let operation_bytes = operation_list
        .iter()
        .map(|value| {
            let byte_integer = value
                .as_u64()
                .expect("Invalid byte type in serialized operation");

            byte_integer
                .try_into()
                .expect("Invalid byte value in serialized operation")
        })
        .collect::<Vec<u8>>();

    let operation =
        bcs::from_bytes::<Operation>(&operation_bytes).expect("Failed to deserialize operation");

    let Operation::LogChatInteraction {
        interaction: ChatInteraction { response, .. },
    } = operation
    else {
        panic!("Unexpected operation returned from service");
    };

    assert!(response.contains("Rio de Janeiro"));
}

/// Tests if a chat interaction is verified on the creation chain and logged on the requesting
/// chain.
#[test_log::test(tokio::test)]
async fn chat_interaction_verification_and_logging() {
    let (validator, application_id, creation_chain) =
        TestValidator::with_current_application::<ApplicationAbi, _, _>((), ()).await;

    let fake_node = PublicKey::from([0_u8; 32]);
    let chat_prompt = "What is one plus one?";
    let chat_response = "2";

    creation_chain
        .add_block(|block| {
            block.with_operation(
                application_id,
                Operation::UpdateNodes {
                    add: vec![fake_node],
                    remove: vec![],
                },
            );
        })
        .await;

    let chat_chain = validator.new_chain().await;

    let request_certificate = chat_chain
        .add_block(|block| {
            block.with_operation(
                application_id,
                Operation::LogChatInteraction {
                    interaction: ChatInteraction {
                        prompt: chat_prompt.to_owned(),
                        response: chat_response.to_owned(),
                    },
                },
            );
        })
        .await;

    let verification_certificate = creation_chain
        .add_block(|block| {
            block.with_messages_from(&request_certificate);
        })
        .await;

    chat_chain
        .add_block(|block| {
            block.with_messages_from(&verification_certificate);
        })
        .await;

    let QueryOutcome { response, .. } = chat_chain
        .graphql_query(
            application_id,
            "query { chatLog { entries { prompt, response } } }",
        )
        .await;

    assert_eq!(
        response.to_string(),
        format!(
            "{{\
            \"chatLog\":{{\
                \"entries\":[\
                    {{\
                        \"prompt\":{chat_prompt:?},\
                        \"response\":{chat_response:?}\
                    }}\
                ]\
            }}\
        }}"
        )
    );
}
