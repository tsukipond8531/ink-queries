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

use anyhow::{
    Result,
};
use subxt::Config;
use crate::substrate::{DefaultConfig, PairSigner, SubstrateBaseConfig};
use crate::substrate::contract::ink::InkMeta;
use super::ContractInstance;


struct NotInitialized;

struct Initialized {
    config: SubstrateBaseConfig,
    contract_address: <DefaultConfig as Config>::AccountId,
}

struct Signed {
    contract_address: <DefaultConfig as Config>::AccountId,
    node_url: String,
    signer: PairSigner,
}

struct ContractBuilder<T> {
    state: T,
}

impl Default for ContractBuilder<NotInitialized> {
    fn default() -> Self {
        Self {
            state: NotInitialized,
        }
    }
}


impl ContractBuilder<NotInitialized> {
    fn init_config(self, config: SubstrateBaseConfig, contract_address: <DefaultConfig as Config>::AccountId) -> ContractBuilder<Initialized> {
        ContractBuilder {
            state: Initialized {
                config,
                contract_address,
            }
        }
    }
}

impl ContractBuilder<Initialized> {
    fn sign(self) -> Result<ContractBuilder<Signed>> {
        let pair = self.state.config.signer()?;

        Ok(ContractBuilder {
            state: Signed {
                contract_address: self.state.contract_address,
                node_url: self.state.config.url_to_string(),
                signer: self.state.config.pair_signer(pair),
            }
        })
    }
}


impl ContractBuilder<Signed> {
    fn build(self) -> Result<ContractInstance> {
        let meta = InkMeta::from_config_file()?;
        Ok(ContractInstance::new(meta, self.state.signer))
    }
}