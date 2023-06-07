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

use crate::substrate::{phala, Balance, Client, ContractId, DefaultConfig, Nonce, PairSigner};
use anyhow::{anyhow, Context, Result};
use contract_transcode::ContractMessageTranscoder;
use contract_transcode::Value;
use jsonrpsee::core::client::ClientT;
use jsonrpsee::rpc_params;
use jsonrpsee::ws_client::WsClientBuilder;
use pallet_contracts_primitives::ContractExecResult;
use scale::{Decode, Encode};
use sp_core::Bytes;
use sp_weights::Weight;
use subxt::Config;

use super::error::ErrorVariant;

pub struct ContractQuery {
    msg_name: String,
    transcoder: ContractMessageTranscoder,
    query: Query,
}

impl ContractQuery {
    pub fn call(&self, url: String, signer: &PairSigner) -> Result<Value, ErrorVariant> {
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

#[derive(Debug, Clone)]
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
    ) -> Result<Value, ErrorVariant> {
        match self {
            Query::InkQuery(message, id) => async_std::task::block_on(self.ink_query(
                url,
                signer,
                transcoder,
                msg_name,
                id.clone(),
                message.clone(),
            )),

            Query::PhalaQuery(message, id, nonce) => {
                let value = async_std::task::block_on(self.pink_query(
                    url,
                    signer,
                    transcoder,
                    msg_name,
                    id.clone(),
                    message.clone(),
                    nonce.clone(),
                ));

                match value {
                    Ok(res) => Ok(res),
                    Err(err) => {
                        let error = ErrorVariant::from(err);
                        Err(error)
                    }
                }
            }
        }
    }

    async fn pink_query(
        &self,
        url: String,
        signer: &PairSigner,
        transcoder: &ContractMessageTranscoder,
        msg_name: &str,
        id: ContractId,
        message: Vec<u8>,
        nonce: Nonce,
    ) -> Result<Value> {
        let payload = phala::pink_query_raw(&url, id, message, signer.signer(), nonce).await??;

        let ref output =
            pallet_contracts_primitives::ContractExecResult::<u128>::decode(&mut &payload[..])?
                .result
                .map_err(|err| anyhow::anyhow!("DispatchError({err:?})"))?;

        if output.did_revert() {
            return Err(anyhow!("Contract execution reverted"));
        }

        let value = transcoder
            .decode_return(msg_name, &mut &output.data[..])
            .context(format!("Failed to decode return value {:?}", &output))?;

        Ok(value)
    }

    async fn ink_query(
        &self,
        url: String,
        signer: &PairSigner,
        transcoder: &ContractMessageTranscoder,
        msg_name: &str,
        id: <DefaultConfig as Config>::AccountId,
        message: Vec<u8>,
    ) -> Result<Value, ErrorVariant> {
        let client = Client::from_url(url.clone()).await?;

        let result = self.call_dry_run(url, signer, id, message).await?;

        match result.result {
            Ok(ref ret_val) => {
                let value = transcoder
                    .decode_return(msg_name, &mut &ret_val.data[..])
                    .context(format!("Failed to decode return value {:?}", &ret_val))?;

                Ok(value)
            }
            Err(ref err) => {
                let metadata = client.metadata();
                let error = ErrorVariant::from_dispatch_error(err, &metadata)?;
                Err(error)
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
