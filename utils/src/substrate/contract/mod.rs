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

pub mod builder;
mod error;
mod ink;
mod query;

use crate::substrate::contract::error::ErrorVariant;
use crate::substrate::contract::ink::InkMeta;
use crate::substrate::contract::query::{InkQuery, Query};
use crate::substrate::{Balance, Client, DefaultConfig, PairSigner};
use anyhow::{Context, Result};
use contract_transcode::Value;
use pallet_contracts_primitives::ContractExecResult;
use phala_types::contract::ContractId;
use sp_core::{crypto::Pair, sr25519};
use sp_weights::Weight;
use subxt::Config;

pub struct SubstrateBaseConfig {
    /// Secret key URI of the node's substrate account.
    suri: String,
    /// Password for the secret key.
    password: Option<String>,
}

impl SubstrateBaseConfig {
    pub fn new(suri: String, password: Option<String>) -> Self {
        Self { suri, password }
    }

    /// Returns the signer for contract extrinsics.
    pub fn signer(&self) -> Result<sr25519::Pair> {
        Pair::from_string(&self.suri, self.password.as_ref().map(String::as_ref))
            .map_err(|_| anyhow::anyhow!("Secret string error"))
    }
}

/// Create a new [`PairSigner`] from the given [`sr25519::Pair`].
pub fn pair_signer(pair: sr25519::Pair) -> PairSigner {
    PairSigner::new(pair)
}

pub struct ContractInstance {
    pub signer: PairSigner,
    meta: InkMeta,
}

impl ContractInstance {
    pub fn new(meta: InkMeta, signer: PairSigner) -> Self {
        Self { meta, signer }
    }

    pub fn call_msg(&self, msg_name: &str, args: Vec<&str>) -> Result<CallResult, ErrorVariant> {
        let call_data = self.get_encoded_msg(msg_name, args);

        let query = match (
            self.meta.ink_contract_id.clone(),
            self.meta.phala_contract_id,
        ) {
            (Some(ink_id), None) => Query::InkQuery(call_data, ink_id),
            (None, Some(phala_id)) => Query::PhalaQuery(call_data, phala_id),
            _ => anyhow::bail!("Contract Id Error: must provide only one contract address"),
        };

        query.query(self.meta.url.clone(), &self.signer)
    }

    fn get_encoded_msg(&self, msg_name: &str, args: Vec<&str>) -> Vec<u8> {
        let artifacts = self.meta.contract_artifacts()?;
        let transcoder = artifacts.contract_transcoder()?;

        transcoder.encode(msg_name, &args)?;
    }
}

/// A struct that encodes RPC parameters required for a call to a smart contract.
///
/// Copied from `pallet-contracts-rpc-runtime-api`.
#[derive(Encode)]
pub struct CallRequest {
    origin: <DefaultConfig as Config>::AccountId,
    dest: <DefaultConfig as Config>::AccountId,
    value: Balance,
    gas_limit: Option<Weight>,
    storage_deposit_limit: Option<Balance>,
    input_data: Vec<u8>,
}

/// Result of the contract call
#[derive(serde::Serialize)]
pub struct CallResult {
    /// Result of a dry run
    pub is_success: bool,
    /// Was the operation reverted
    pub reverted: bool,
    pub data: Value,
}
