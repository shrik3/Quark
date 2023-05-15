// Copyright (c) 2021 Quark Container Authors
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

#![allow(dead_code)]
#![allow(non_snake_case)]
#[allow(non_camel_case_types)]

extern crate reqwest;
#[macro_use]
extern crate log;
extern crate simple_logging;

use once_cell::sync::OnceCell;

use nm_store::NodeMgrCache;
use qobjs::common::Result as QResult;

pub mod nm_svc;
pub mod na_client;
pub mod nodemgr;
pub mod types;
pub mod nm_store;

use crate::nm_svc::*;

pub static NM_CACHE : OnceCell<NodeMgrCache> = OnceCell::new();


#[tokio::main]
async fn main() -> QResult<()> {
    log4rs::init_file("logging_config.yaml", Default::default()).unwrap();
    NM_CACHE.set(NodeMgrCache::New().await.unwrap()).unwrap();

    GrpcService().await.unwrap();

    Ok(())
}

