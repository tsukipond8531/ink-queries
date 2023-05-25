// Copyright (C) 2022-2023 <company>
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//      http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

mod contract_call;
mod runtime_api;

use anyhow::{
    Ok,
    Result,
};
use jsonrpsee::rpc_params;
use jsonrpsee::{
    core::client::ClientT,
    ws_client::WsClientBuilder,
};
use scale::{Decode, Encode};
use sp_core::{Bytes, crypto::Pair, sr25519};

pub use subxt::{
    Config,
    OnlineClient,
    PolkadotConfig as DefaultConfig,
    tx,
};

type Client = OnlineClient<DefaultConfig>;
type Balance = u128;
type CodeHash = <DefaultConfig as Config>::Hash;

type PairSigner = tx::PairSigner<DefaultConfig, sr25519::Pair>;

enum CallType {
    DryRun,
    StateChange,
}

struct SubstrateBaseConfig {
    /// Secret key URI of the node's substrate account.
    suri: String,
    /// Password for the secret key.
    password: Option<String>,
    /// Substrate node url
    url: url::Url,
}

async fn state_call<A: Encode, R: Decode>(url: &str, func: &str, args: A) -> Result<R> {
    let client = WsClientBuilder::default().build(&url).await?;
    let params = rpc_params![func, Bytes(args.encode())];
    let bytes: Bytes = client.request("state_call", params).await?;
    Ok(R::decode(&mut bytes.as_ref())?)
}
