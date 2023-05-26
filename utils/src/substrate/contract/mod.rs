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

mod builder;
mod ink;
pub mod utils;
mod error;

use anyhow::{Context, Result};
use crate::substrate::transcoder::Value;

use jsonrpsee::{
    core::client::ClientT,
    rpc_params,
    ws_client::WsClientBuilder,
};

use pallet_contracts_primitives::ContractExecResult;

use scale::{Decode, Encode};

use sp_core::Bytes;

use sp_weights::Weight;

use subxt::{Config, OnlineClient};

use crate::substrate::{
    Balance,
    DefaultConfig,
    PairSigner,
};
use crate::substrate::contract::error::ErrorVariant;
use crate::substrate::contract::ink::InkMeta;


pub struct ContractInstance {
    signer: PairSigner,
    meta: InkMeta,
}


impl ContractInstance {
    pub fn new(meta: InkMeta, signer: PairSigner) -> Self {
        Self {
            meta,
            signer,
        }
    }

    pub fn call_message(&self, msg_name: String, args: Vec<String>) -> Result<CallResult, ErrorVariant> {
        let artifacts = self.meta.contract_artifacts()?;
        let transcoder = artifacts.contract_transcoder()?;

        let call_data = transcoder.encode(&msg_name, &args)?;

        async_std::task::block_on(async {
            let client = OnlineClient::<DefaultConfig>::from_url(self.meta.url.clone()).await?;

            let result = self
                .call_dry_run(call_data.clone())
                .await?;
            match result.result {
                Ok(ref ret_val) => {
                    let value = transcoder
                        .decode_return(&msg_name, &mut &ret_val.data[..])
                        .context(format!(
                            "Failed to decode return value {:?}",
                            &ret_val
                        ))?;

                    Ok(CallResult {
                        is_success: true,
                        reverted: ret_val.did_revert(),
                        data: value,
                    })
                }
                Err(ref err) => {
                    let metadata = client.metadata();
                    let object = ErrorVariant::from_dispatch_error(err, &metadata)?;
                    Err(object)
                }
            }
        })
    }


    async fn call_dry_run(
        &self,
        input_data: Vec<u8>,
    ) -> Result<ContractExecResult<Balance>> {
        let call_request = CallRequest {
            origin: self.signer.account_id().clone(),
            dest: self.meta.contract_address.clone(),
            value: 0,
            gas_limit: None,
            storage_deposit_limit: None,
            input_data,
        };
        self.state_call(self.meta.url.as_str(), "ContractsApi_call", call_request).await
    }


    async fn state_call<A: Encode, R: Decode>(&self, url: &str, func: &str, args: A) -> Result<R> {
        let client = WsClientBuilder::default().build(&url).await?;
        let params = rpc_params![func, Bytes(args.encode())];
        let bytes: Bytes = client.request("state_call", params).await?;
        Ok(R::decode(&mut bytes.as_ref())?)
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

