// Copyright (c) 2021 Quark Container Authors / 2018 The gVisor Authors.
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
// limitations under

#![allow(dead_code)]
#![allow(non_snake_case)]
#![allow(non_camel_case_types)]
#![allow(non_upper_case_globals)]
#![allow(deprecated)]

#[macro_use]
extern crate log;
extern crate simple_logging;

pub mod func_agent_mgr;
pub mod func_worker;
pub mod http_gateway;
pub mod obj_repo;
//pub mod tsot_client;

use obj_repo::{NamespaceStore, ObjRepo};
use once_cell::sync::OnceCell;

use http_gateway::*;
use qshare::common::*;
use qshare::tsot_client::TsotClient;

pub static OBJ_REPO: OnceCell<ObjRepo> = OnceCell::new();
pub static NAMESPACE_STORE: OnceCell<NamespaceStore> = OnceCell::new();
pub static TSOT_CLIENT: OnceCell<TsotClient> = OnceCell::new();

#[tokio::main]
async fn main() -> Result<()> {
    log4rs::init_file("/etc/quark/gateway_logging_config.yaml", Default::default()).unwrap();

    OBJ_REPO
        .set(ObjRepo::New(vec!["http://127.0.0.1:8890".to_owned()]).await?)
        .unwrap();
    NAMESPACE_STORE
        .set(NamespaceStore::New(&vec!["http://127.0.0.1:2379".to_owned()]).await?)
        .unwrap();
    TSOT_CLIENT.set(TsotClient::New().await?).unwrap();

    error!("gateway ...");
    let gateway = HttpGateway {};

    tokio::select! {
        res = gateway.HttpServe() => {
            error!("HttpServe finish with res {:?}", &res);
            res?;
        }
        _ = TSOT_CLIENT.get().unwrap().Process() => {
            error!("TSOT_CLIENT finish with res");
        }
    }
    return Ok(());
}
