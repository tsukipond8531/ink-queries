use super::contract::ink;
use super::{contract::query::Query, PairSigner};
use crate::substrate::{ContractId, KeyExtension, Nonce, PairExtension};
use anyhow::anyhow;
use anyhow::Result;
use phactory_api::prpc::phactory_api_client::PhactoryApiClient;

use crate::substrate::contract::ink::{decode_hex, try_decode_hex};
use phactory_api::pruntime_client::RpcRequest;
use phactory_api::{
    crypto::{CertificateBody, EncryptedData},
    prpc,
};
use phala_crypto::aead;
use phala_crypto::ecdh::{EcdhKey, EcdhPublicKey};
use phala_types::contract;
use scale::{Decode, Encode};
use sp_core::Pair;
use std::convert::TryFrom as _;

#[derive(Debug, Encode, Decode)]
pub enum Response {
    Payload(Vec<u8>),
}

pub struct Ready;

pub struct InProcess {
    pr: PhactoryApiClient<RpcRequest>,
    req: contract::ContractQuery<PinkQuery>,
}

pub struct Encrypted {
    pr: PhactoryApiClient<RpcRequest>,
    ecdh_key: EcdhKey,
    req: prpc::ContractQueryRequest,
}

pub struct Computed<Result: Decode> {
    result: Result,
}

pub struct PhalaQuery<T> {
    nonce: Option<Nonce>,
    /// Public key used for the key agreement
    remote_pubkey: Option<EcdhPublicKey>,
    /// Signer of the request
    signer: Option<PairSigner>,
    /// state of the request to be encrypted
    state: T,
}

impl Default for PhalaQuery<Ready> {
    fn default() -> Self {
        Self {
            nonce: None,
            remote_pubkey: None,
            signer: None,
            state: Ready,
        }
    }
}

/// 1. Make the Contract Query
impl PhalaQuery<Ready> {
    pub async fn new(
        url: String,
        query: Query,
        signer: &PairSigner,
    ) -> Result<PhalaQuery<InProcess>> {
        match query {
            Query::PhalaQuery(message, id, nonce) => {
                let nonce_query = nonce.clone();

                let head = contract::ContractQueryHead { id, nonce };
                let data = PinkQuery::InkMessage {
                    payload: message,
                    deposit: 0,
                    transfer: 0,
                    estimating: false,
                };

                let query = contract::ContractQuery { head, data };

                let pr = phactory_api::pruntime_client::new_pruntime_client(url);

                let info = pr.get_info(()).await?;
                let remote_pubkey = info
                    .system
                    .ok_or_else(|| anyhow!("Worker not initialized"))?
                    .ecdh_public_key;

                let remote_pubkey = ink::try_decode_hex(&remote_pubkey)?;
                let remote_pubkey = EcdhPublicKey::try_from(&remote_pubkey[..])?;

                Ok(PhalaQuery {
                    nonce: Some(nonce_query),
                    remote_pubkey: Some(remote_pubkey),
                    signer: Some(signer.consume_ref()),
                    state: InProcess { pr, req: query },
                })
            }

            Query::InkQuery(_msg, _id) => anyhow::bail!("Only Phala queries are allowed"),
        }
    }
}

/// 2. Encrypt the Contract Query
impl PhalaQuery<InProcess> {
    /// Encrypt the Contract Query
    pub fn encrypt_and_sign(self) -> Result<PhalaQuery<Encrypted>> {
        let ecdh_key = sp_core::sr25519::Pair::generate()
            .0
            .derive_ecdh_key()
            .map_err(|_| anyhow!("Derive ecdh key failed"))?;

        let iv = aead::generate_iv(&self.nonce.unwrap());

        let encrypted_data = EncryptedData::encrypt(
            &ecdh_key,
            &self.remote_pubkey.unwrap(),
            iv,
            &self.state.req.encode(),
        )
        .map_err(|_| anyhow!("Encrypt data failed"))?;

        let signer = self.signer.as_ref().unwrap().signer();

        let data_cert_body = CertificateBody {
            pubkey: signer.public().to_vec(),
            ttl: u32::MAX,
            config_bits: 0,
        };
        let data_cert = prpc::Certificate::new(data_cert_body, None);
        let data_signature = prpc::Signature {
            signed_by: Some(Box::new(data_cert)),
            signature_type: prpc::SignatureType::Sr25519 as _,
            signature: signer.sign(&encrypted_data.encode()).0.to_vec(),
        };

        let req = prpc::ContractQueryRequest::new(encrypted_data, Some(data_signature));

        Ok(PhalaQuery {
            nonce: self.nonce,
            remote_pubkey: self.remote_pubkey,
            signer: self.signer,
            state: Encrypted {
                pr: self.state.pr,
                ecdh_key,
                req,
            },
        })
    }
}

/// 3. Make the rpc call and retrieve result
impl PhalaQuery<Encrypted> {
    pub async fn query<Response: Decode>(self) -> Result<PhalaQuery<Computed<Response>>> {
        let ecdh_key = self.state.ecdh_key;

        let request = self.state.req;

        let response = self.state.pr.contract_query(request).await?;

        // Decrypt the response
        let encrypted_data = response.decode_encrypted_data()?;

        let data = encrypted_data
            .decrypt(&ecdh_key)
            .map_err(|_| anyhow!("Decrypt data failed"))?;

        // Decode the response.
        let response: contract::ContractQueryResponse<Response> = Decode::decode(&mut &data[..])?;

        // check the nonce is match the one we sent.
        if response.nonce != self.nonce.unwrap() {
            return Err(anyhow!("nonce mismatch"));
        }

        Ok(PhalaQuery {
            nonce: None,
            remote_pubkey: None,
            signer: None,
            state: Computed {
                result: response.result,
            },
        })
    }
}

impl PhalaQuery<Computed<Response>> {
    pub fn result(self) -> Vec<u8> {
        let Response::Payload(payload) = self.state.result;
        payload
    }
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
    _SidevmQuery(Vec<u8>),
}

pub async fn query_test<Request: Encode>(
    url: String,
    id: ContractId,
    data: Request,
) -> Result<Vec<u8>> {
    // 2. Make ContractQuery
    let nonce = [1; 32];
    let head = contract::ContractQueryHead { id, nonce };
    let query = contract::ContractQuery { head, data };

    let pr = phactory_api::pruntime_client::new_pruntime_client(url);

    let info = pr.get_info(()).await?;
    let remote_pubkey = info
        .system
        .ok_or_else(|| anyhow!("Worker not initialized"))?
        .ecdh_public_key;
    let remote_pubkey = try_decode_hex(&remote_pubkey)?;
    let remote_pubkey = EcdhPublicKey::try_from(&remote_pubkey[..])?;

    // 3. Encrypt the ContractQuery.

    let ecdh_key = sp_core::sr25519::Pair::generate()
        .0
        .derive_ecdh_key()
        .map_err(|_| anyhow!("Derive ecdh key failed"))?;

    let iv = [1; 12];
    let encrypted_data = EncryptedData::encrypt(&ecdh_key, &remote_pubkey, iv, &query.encode())
        .map_err(|_| anyhow!("Encrypt data failed"))?;

    // 4. Sign the encrypted data.
    // 4.1 Make the root certificate.
    let (root_key, _) = sp_core::sr25519::Pair::generate();
    let root_cert_body = CertificateBody {
        pubkey: root_key.public().to_vec(),
        ttl: u32::MAX,
        config_bits: 0,
    };
    let root_cert = prpc::Certificate::new(root_cert_body, None);

    // 4.2 Generate a temporary key pair and sign it with root key.
    let (key_g, _) = sp_core::sr25519::Pair::generate();

    let data_cert_body = CertificateBody {
        pubkey: key_g.public().to_vec(),
        ttl: u32::MAX,
        config_bits: 0,
    };
    let cert_signature = prpc::Signature {
        signed_by: Some(Box::new(root_cert)),
        signature_type: prpc::SignatureType::Sr25519 as _,
        signature: root_key.sign(&data_cert_body.encode()).0.to_vec(),
    };
    let data_cert = prpc::Certificate::new(data_cert_body, Some(Box::new(cert_signature)));
    let data_signature = prpc::Signature {
        signed_by: Some(Box::new(data_cert)),
        signature_type: prpc::SignatureType::Sr25519 as _,
        signature: key_g.sign(&encrypted_data.encode()).0.to_vec(),
    };

    let request = prpc::ContractQueryRequest::new(encrypted_data, Some(data_signature));

    // 5. Do the RPC call.
    let response = pr.contract_query(request).await?;

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
    let Response::Payload(payload) = response.result;
    Ok(payload)
}

pub async fn pink_query_raw(
    worker_pubkey: &[u8],
    url: &str,
    id: ContractId,
    call_data: Vec<u8>,
    key: &sp_core::sr25519::Pair,
) -> Result<Result<Vec<u8>, QueryError>> {
    let query = PinkQuery::InkMessage {
        payload: call_data,
        deposit: 0,
        transfer: 0,
        estimating: false,
    };
    let result: Result<Response, QueryError> =
        contract_query(worker_pubkey, url, id, query, key).await?;
    Ok(result.map(|r| {
        let Response::Payload(payload) = r;
        payload
    }))
}

pub async fn contract_query<Request: Encode, Response: Decode>(
    worker_pubkey: &[u8],
    url: &str,
    id: ContractId,
    data: Request,
    key: &sp_core::sr25519::Pair,
) -> Result<Response> {
    // 2. Make ContractQuery
    let nonce = [1; 32];
    let head = contract::ContractQueryHead { id, nonce };
    let query = contract::ContractQuery { head, data };

    let pr = phactory_api::pruntime_client::new_pruntime_client_no_log(url.into());

    let remote_pubkey = EcdhPublicKey::try_from(worker_pubkey)?;

    // 3. Encrypt the ContractQuery.

    let ecdh_key = sp_core::sr25519::Pair::generate()
        .0
        .derive_ecdh_key()
        .map_err(|_| anyhow!("Derive ecdh key failed"))?;

    let iv = aead::generate_iv(&nonce);
    let encrypted_data = EncryptedData::encrypt(&ecdh_key, &remote_pubkey, iv, &query.encode())
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
    let response = pr.contract_query(request).await?;

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
