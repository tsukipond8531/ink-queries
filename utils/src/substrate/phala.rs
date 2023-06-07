use crate::substrate::{contract::ink::try_decode_hex, ContractId, KeyExtension, Nonce};
use anyhow::anyhow;
use anyhow::Result;
use phactory_api::prpc::phactory_api_client::PhactoryApiClient;
use phactory_api::pruntime_client::RpcRequest;
use phactory_api::{
    crypto::{CertificateBody, EncryptedData},
    prpc,
};
use phala_crypto::aead;
use phala_crypto::ecdh::EcdhPublicKey;
use phala_types::contract;
use scale::{Decode, Encode};
use sp_core::Pair;
use std::convert::TryFrom as _;

const DEPOSIT: u128 = 0;
const TRANSFER: u128 = 0;

struct Worker {
    pubkey: EcdhPublicKey,
}

struct PRuntime {
    pr: PhactoryApiClient<RpcRequest>,
}

impl PRuntime {
    fn new(url: &str) -> Self {
        Self {
            pr: phactory_api::pruntime_client::new_pruntime_client(url.to_string()),
        }
    }

    async fn retrieve_worker(&self) -> Result<Worker> {
        let info = self.pr.get_info(()).await?;
        let pubkey = info
            .system
            .ok_or_else(|| anyhow!("Worker not initialized"))?
            .ecdh_public_key;
        let pubkey = try_decode_hex(&pubkey)?;
        let pubkey = EcdhPublicKey::try_from(&pubkey[..])?;

        Ok(Worker { pubkey })
    }
}

// Copied from phat-poller crate for phat contract queries

pub async fn pink_query_raw(
    url: &str,
    id: ContractId,
    call_data: Vec<u8>,
    key: &sp_core::sr25519::Pair,
    nonce: Nonce,
) -> Result<Result<Vec<u8>, QueryError>> {
    let query = PinkQuery::InkMessage {
        payload: call_data,
        deposit: DEPOSIT,
        transfer: TRANSFER,
        estimating: false,
    };
    let result: Result<Response, QueryError> = contract_query(url, id, query, key, nonce).await?;
    Ok(result.map(|r| {
        let Response::Payload(payload) = r;
        payload
    }))
}

pub async fn contract_query<Request: Encode, Response: Decode>(
    url: &str,
    id: ContractId,
    data: Request,
    key: &sp_core::sr25519::Pair,
    nonce: Nonce,
) -> Result<Response> {
    // 2. Make ContractQuery
    let head = contract::ContractQueryHead { id, nonce };
    let query = contract::ContractQuery { head, data };

    let p_runtime = PRuntime::new(url);

    let worker = p_runtime.retrieve_worker().await?;

    // 3. Encrypt the ContractQuery.

    let ecdh_key = sp_core::sr25519::Pair::generate()
        .0
        .derive_ecdh_key()
        .map_err(|_| anyhow!("Derive ecdh key failed"))?;

    let iv = aead::generate_iv(&nonce);
    let encrypted_data = EncryptedData::encrypt(&ecdh_key, &worker.pubkey, iv, &query.encode())
        .map_err(|_| anyhow!("Encrypt data failed"))?;

    let data_cert_body = CertificateBody {
        pubkey: key.public().to_vec(),
        ttl: u32::MAX,
        config_bits: 0,
    };
    let data_cert = prpc::Certificate::new(data_cert_body, None);
    let data_signature = prpc::Signature {
        signed_by: Some(Box::new(data_cert)),
        signature_type: prpc::SignatureType::Sr25519 as _,
        signature: key.sign(&encrypted_data.encode()).0.to_vec(),
    };

    let request = prpc::ContractQueryRequest::new(encrypted_data, Some(data_signature));

    // 5. Do the RPC call.
    let response = p_runtime.pr.contract_query(request).await?;

    // 6. Decrypt the response.
    let encrypted_data = response.decode_encrypted_data()?;
    let data = encrypted_data
        .decrypt(&ecdh_key)
        .map_err(|_| anyhow!("Decrypt data failed"))?;

    // 7. Decode the response.
    let response: contract::ContractQueryResponse<Response> = Decode::decode(&mut &data[..])?;

    // 8. check the nonce is match the one we sent.
    if response.nonce != nonce {
        return Err(anyhow!("nonce mismatch"));
    }

    Ok(response.result)
}

#[derive(Debug, Encode, Decode)]
pub enum Response {
    Payload(Vec<u8>),
}

// Copied from phat-poller query module in phala-blockchain/standalone

#[derive(Debug, Encode, Decode)]
pub enum PinkQuery {
    InkMessage {
        payload: Vec<u8>,
        /// Amount of tokens deposit to the caller.
        deposit: u128,
        /// Amount of tokens transfer from the caller to the target contract.
        transfer: u128,
        /// Whether to use the gas estimation mode.
        estimating: bool,
    },
    SidevmQuery(Vec<u8>),
}

#[derive(Debug, Encode, Decode)]
pub enum QueryError {
    BadOrigin,
    RuntimeError(String),
    SidevmNotFound,
}

impl std::fmt::Display for QueryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            QueryError::BadOrigin => write!(f, "Bad origin"),
            QueryError::RuntimeError(msg) => write!(f, "Runtime error: {}", msg),
            QueryError::SidevmNotFound => write!(f, "Sidevm not found"),
        }
    }
}

impl std::error::Error for QueryError {}
