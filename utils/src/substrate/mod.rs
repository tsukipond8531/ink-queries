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

mod contract;
mod metadata;

#[allow(dead_code)]
mod transcoder;


use anyhow::{
    Result,
};

use sp_core::{crypto::Pair, sr25519};

pub use subxt::{
    Config,
    OnlineClient,
    PolkadotConfig as DefaultConfig,
    tx,
};

use url::Url;

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
    url: Url,
}

impl SubstrateBaseConfig {
    /// Returns the signer for contract extrinsics.
    pub fn signer(&self) -> Result<sr25519::Pair> {
        Pair::from_string(&self.suri, self.password.as_ref().map(String::as_ref))
            .map_err(|_| anyhow::anyhow!("Secret string error"))
    }

    pub fn url_to_string(&self) -> String {
        let mut res = self.url.to_string();
        match (self.url.port(), self.url.port_or_known_default()) {
            (None, Some(port)) => {
                res.insert_str(res.len() - 1, &format!(":{port}"));
                res
            }
            _ => res,
        }
    }

    /// Create a new [`PairSigner`] from the given [`sr25519::Pair`].
    pub fn pair_signer(&self, pair: sr25519::Pair) -> PairSigner {
        PairSigner::new(pair)
    }
}




