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

use crate::substrate::contract::error::ErrorVariant;
use crate::substrate::phala::Nonce;
use crate::substrate::{Balance, Client, DefaultConfig, PairSigner};
use anyhow::{Context, Result};
use contract_transcode::ContractMessageTranscoder;
use contract_transcode::Value;
use jsonrpsee::core::client::ClientT;
use jsonrpsee::rpc_params;
use jsonrpsee::ws_client::WsClientBuilder;
use pallet_contracts_primitives::ContractExecResult;
use phala_types::contract::ContractId;
use scale::{Decode, Encode};
use sp_core::Bytes;
use sp_weights::Weight;
use subxt::Config;

pub struct ContractQuery {
    msg_name: String,
    transcoder: ContractMessageTranscoder,
    query: Query,
}

impl ContractQuery {
    pub fn call(&self, url: String, signer: &PairSigner) -> Result<CallResult, ErrorVariant> {
        self.query
            .query(url, signer, &self.transcoder, self.msg_name.as_str())
    }
}

pub struct QueryBuilder {
    msg_name: String,
    transcoder: ContractMessageTranscoder,
    query: Option<Query>,
}

impl QueryBuilder {
    pub fn new(msg_name: String, transcoder: ContractMessageTranscoder) -> Self {
        Self {
            msg_name,
            transcoder,
            query: None,
        }
    }

    pub fn query(mut self, query: Query) -> Self {
        self.query = Some(query);
        self
    }

    pub fn build(self) -> ContractQuery {
        ContractQuery {
            msg_name: self.msg_name,
            transcoder: self.transcoder,
            query: self.query.expect("Query is not set"),
        }
    }
}

pub enum Query {
    InkQuery(Vec<u8>, <DefaultConfig as Config>::AccountId),
    PhalaQuery(Vec<u8>, ContractId, Nonce),
}

impl Query {
    pub fn query(
        &self,
        url: String,
        signer: &PairSigner,
        transcoder: &ContractMessageTranscoder,
        msg_name: &str,
    ) -> Result<CallResult, ErrorVariant> {
        match self {
            Query::InkQuery(message, id) => async_std::task::block_on(async {
                let client = Client::from_url(url.clone()).await?;

                let result = self
                    .call_dry_run(url, signer, id.clone(), message.clone())
                    .await?;

                match result.result {
                    Ok(ref ret_val) => {
                        let value = transcoder
                            .decode_return(msg_name, &mut &ret_val.data[..])
                            .context(format!("Failed to decode return value {:?}", &ret_val))?;

                        Ok(CallResult {
                            is_success: true,
                            reverted: ret_val.did_revert(),
                            data: value,
                        })
                    }
                    Err(ref err) => {
                        let metadata = client.metadata();
                        let error = ErrorVariant::from_dispatch_error(err, &metadata)?;
                        Err(error)
                    }
                }
            }),
            Query::PhalaQuery(_message, _id) => {
                Err(ErrorVariant::PhalaError("Not implemented".to_string()))
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
        self.state_call(url.as_str(), "ContractsApi_call", call_request)
            .await
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
