use super::contract::ink;
use super::{contract::query::Query, PairSigner};
use crate::substrate::{KeyExtension, Nonce, PairExtension};
use anyhow::anyhow;
use anyhow::Result;
use phactory_api::prpc::phactory_api_client::PhactoryApiClient;

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
struct InkMessage(Vec<u8>);

#[derive(Debug, Encode, Decode)]
pub enum Response {
    Payload(Vec<u8>),
}

pub struct Ready;

pub struct InProcess {
    pr: PhactoryApiClient<RpcRequest>,
    req: contract::ContractQuery<InkMessage>,
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
                let query = contract::ContractQuery {
                    head,
                    data: InkMessage(message),
                };
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
