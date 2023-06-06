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

use super::{ink::InkMeta, ContractInstance};
use crate::substrate;
use crate::substrate::{PairSigner, SubstrateBaseConfig};
use anyhow::Result;

pub struct NotInitialized;

pub struct Initialized {
    config: SubstrateBaseConfig,
}

pub struct Signed {
    signer: PairSigner,
}

pub struct ContractBuilder<T> {
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
    pub fn init_config(self, config: SubstrateBaseConfig) -> ContractBuilder<Initialized> {
        ContractBuilder {
            state: Initialized { config },
        }
    }
}

impl ContractBuilder<Initialized> {
    pub fn sign(self) -> Result<ContractBuilder<Signed>> {
        let pair = self.state.config.signer()?;

        Ok(ContractBuilder {
            state: Signed {
                signer: substrate::pair_signer(pair),
            },
        })
    }
}

impl ContractBuilder<Signed> {
    pub fn build(self) -> Result<ContractInstance> {
        let meta = InkMeta::from_config_file()?;
        Ok(ContractInstance::new(meta, self.state.signer))
    }
}
