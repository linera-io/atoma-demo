// Copyright (c) Zefchain Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

use linera_sdk::linera_base_types::{ContractAbi, ServiceAbi};
use serde::{Deserialize, Serialize};

pub struct ApplicationAbi;

impl ContractAbi for ApplicationAbi {
    type Operation = Operation;
    type Response = ();
}

impl ServiceAbi for ApplicationAbi {
    type Query = async_graphql::Request;
    type QueryResponse = async_graphql::Response;
}

/// Operations that the contract can execute.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum Operation {
    /// Update the set of active Atoma nodes.
    UpdateNodes {
        add: Vec<PublicKey>,
        remove: Vec<PublicKey>,
    },

    /// Log an interaction with the AI.
    LogChatInteraction { interaction: ChatInteraction },
}

/// A single interaction with the AI chat.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, async_graphql::SimpleObject)]
#[cfg_attr(feature = "test", derive(test_strategy::Arbitrary))]
pub struct ChatInteraction {
    #[cfg_attr(feature = "test", strategy("[A-Za-z0-9., ]*"))]
    pub prompt: String,
    #[cfg_attr(feature = "test", strategy("[A-Za-z0-9., ]*"))]
    pub response: String,
}

/// Representation of an Atoma node's public key.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Deserialize, Serialize)]
#[cfg_attr(feature = "test", derive(test_strategy::Arbitrary))]
pub struct PublicKey([u8; 32]);
async_graphql::scalar!(PublicKey);

impl From<[u8; 32]> for PublicKey {
    fn from(bytes: [u8; 32]) -> Self {
        PublicKey(bytes)
    }
}
