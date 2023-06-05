use crate::substrate::contract::pair_signer;
use anyhow::Result;
use phala_crypto::ecdh::EcdhPublicKey;
use phala_types::contract;
use phala_types::contract::ContractId;
use scale::{Decode, Encode};
use std::convert::TryFrom as _;

use super::{
    contract::{ink, query::Query},
    PairSigner,
};

pub struct Nonce([u8; 32]);

#[derive(Debug, Encode, Decode)]
struct InkMessage(Vec<u8>);

struct PhalaQuery {
    /// Contract query request parameters, to be encrypted.
    query: contract::ContractQuery<InkMessage>,
    nonce: Nonce,
    /// Public key used for the key agreement
    remote_pubkey: EcdhPublicKey,
    /// Signer of the request
    signer: PairSigner,
}

impl PhalaQuery {
    pub fn new(_id: ContractId, _url: String, query: Query, signer: &PairSigner) -> Result<Self> {
        match query {
            Query::PhalaQuery(message, id, nonce) => {
                let nonce = nonce.0;
                let head = contract::ContractQueryHead { id, nonce };
                let query = contract::ContractQuery {
                    head,
                    data: InkMessage(message),
                };

                let remote_pubkey = "test";
                let remote_pubkey = ink::try_decode_hex(&remote_pubkey)?;
                let remote_pubkey = EcdhPublicKey::try_from(&remote_pubkey[..])?;

                Ok(Self {
                    query,
                    nonce: Nonce(nonce),
                    remote_pubkey,
                    signer: pair_signer(signer.signer().clone()),
                })
            }
            Query::InkQuery(_msg, _id) => anyhow::bail!("Only Phala queries are allowed"),
        }
    }
}
