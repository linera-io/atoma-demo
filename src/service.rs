// Copyright (c) Zefchain Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

#![cfg_attr(target_arch = "wasm32", no_main)]

mod state;

use async_graphql::{connection::EmptyFields, EmptyMutation, EmptySubscription, Schema};
use linera_sdk::{base::WithServiceAbi, Service, ServiceRuntime};

#[derive(Clone)]
pub struct ApplicationService;

linera_sdk::service!(ApplicationService);

impl WithServiceAbi for ApplicationService {
    type Abi = atoma_demo::ApplicationAbi;
}

impl Service for ApplicationService {
    type Parameters = ();

    async fn new(_runtime: ServiceRuntime<Self>) -> Self {
        ApplicationService
    }

    async fn handle_query(&self, query: Self::Query) -> Self::QueryResponse {
        Schema::build(EmptyFields, EmptyMutation, EmptySubscription)
            .finish()
            .execute(query)
            .await
    }
}
