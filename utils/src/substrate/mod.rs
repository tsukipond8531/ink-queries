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


use anyhow::Result;

use sp_core::sr25519;


pub use subxt::{
    Config,
    OnlineClient,
    PolkadotConfig as DefaultConfig,
    tx,
};

use contract::{SubstrateBaseConfig, ContractInstance, builder::ContractBuilder};
pub use contract::Execution;

type Client = OnlineClient<DefaultConfig>;
type Balance = u128;
type PairSigner = tx::PairSigner<DefaultConfig, sr25519::Pair>;


pub struct SubstrateContract {
    pub instance: ContractInstance,
}

impl SubstrateContract {
    pub fn new(suri: String, password: Option<String>) -> Result<Self> {
        let config = SubstrateBaseConfig::new(suri, password);

        let instance = ContractBuilder::default().
            init_config(config).
            sign()?
            .build()?;

        Ok(Self { instance })
    }

    pub fn get_pair_signer(&self) -> PairSigner {
        let signer = self.instance.signer.signer().clone();
        PairSigner::new(signer)
    }
}





