// Copyright (c) Zefchain Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

use atoma_demo::{ChatInteraction, PublicKey};
use linera_sdk::views::{linera_views, LogView, RootView, SetView, ViewStorageContext};

#[derive(RootView, async_graphql::SimpleObject)]
#[view(context = "ViewStorageContext")]
pub struct Application {
    pub active_atoma_nodes: SetView<PublicKey>,
    pub chat_log: LogView<ChatInteraction>,
}
