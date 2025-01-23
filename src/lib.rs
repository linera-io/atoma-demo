// Copyright (c) Zefchain Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

use linera_sdk::base::{ContractAbi, ServiceAbi};
use serde::{Deserialize, Serialize};

pub struct ApplicationAbi;

impl ContractAbi for ApplicationAbi {
    type Operation = ();
    type Response = ();
}

impl ServiceAbi for ApplicationAbi {
    type Query = ();
    type QueryResponse = ();
}

/// A single interaction with the AI chat.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, async_graphql::SimpleObject)]
pub struct ChatInteraction {
    pub prompt: String,
    pub response: String,
}
