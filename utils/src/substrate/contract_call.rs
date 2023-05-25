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
    anyhow,
    Context,
    Result,
};
use subxt::{Config, OnlineClient};
use url::Url;
use super::{
    DefaultConfig,
    Balance,
    PairSigner,
};
use super::runtime_api::api;
use pallet_contracts_primitives::ContractExecResult;
use sp_core::blake2_256;
use sp_core::crypto::Ss58Codec;
use sp_weights::Weight;
use subxt::utils::AccountId32;

pub struct ContractInstance {
    contract_address: <DefaultConfig as Config>::AccountId,
    signer: PairSigner,
    node_url: Url,
}


impl ContractInstance {
    pub fn new(contract_address: <DefaultConfig as Config>::AccountId, signer: PairSigner, node_url: Url) -> Self {
        ContractInstance {
            contract_address,
            signer,
            node_url,
        }
    }

    /// Returns the result of a contract call to a specific message, without a state transition
    /// Takes as input the name of the function
    async fn call_dry_run(&self, msg_name: &str) -> Result<ContractExecResult<Balance>> {
        //TODO
    }
}


pub struct Request {
    /// Address of the caller
    origin: <DefaultConfig as Config>::AccountId,
    /// Address of the destination contract
    dest: <DefaultConfig as Config>::AccountId,
    /// Amount of max gas -> must be None for a read call
    gas_limit: Option<Weight>,
    /// The maximum amount of balance that can be charged from the caller to pay for the
    /// storage. consumed.
    storage_deposit_limit: Option<Balance>,
    /// Encoded message to execute
    input_data: Vec<u8>,
}

