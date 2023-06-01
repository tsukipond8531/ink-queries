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

use anyhow::Result;
use jsonrpsee::core::client::ClientT;
use jsonrpsee::rpc_params;
use jsonrpsee::ws_client::WsClientBuilder;
use pallet_contracts_primitives::ContractExecResult;
use phala_types::contract::ContractId;
use scale::{Decode, Encode};
use sp_core::Bytes;
use subxt::Config;
use crate::substrate::{Balance, Client, DefaultConfig, PairSigner};
use crate::substrate::contract::{CallRequest, CallResult};
use crate::substrate::contract::error::ErrorVariant;

pub enum Query {
    InkQuery(Vec<u8>, <DefaultConfig as Config>::AccountId),
    PhalaQuery(Vec<u8>, ContractId),
}


impl Query {
    pub fn query(&self, url: String, signer: &PairSigner) -> Result<CallResult, ErrorVariant> {
        match self {
            Query::InkQuery(message, id) => {
                async_std::task::block_on(async {
                    let client = Client::from_url(url.clone()).await?;

                    let result = self
                        .call_dry_run(url, signer, id.clone(), message.clone())
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
            Query::PhalaQuery(message, id) => {
                //TODO
            }
        }
    }

    async fn call_dry_run(
        &self,
        url: String,
        signer: &PairSigner,
        dest: <DefaultConfig as Config>::AccountId,
        input_data: Vec<u8>,
    ) -> Result<ContractExecResult<Balance>> {
        let call_request = CallRequest {
            origin: signer.account_id().clone(),
            dest,
            value: 0,
            gas_limit: None,
            storage_deposit_limit: None,
            input_data,
        };
        self.state_call(url.as_str(), "ContractsApi_call", call_request).await
    }


    async fn state_call<A: Encode, R: Decode>(&self, url: &str, func: &str, args: A) -> Result<R> {
        let client = WsClientBuilder::default().build(&url).await?;
        let params = rpc_params![func, Bytes(args.encode())];
        let bytes: Bytes = client.request("state_call", params).await?;
        Ok(R::decode(&mut bytes.as_ref())?)
    }
}
