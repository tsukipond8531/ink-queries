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

mod contract;
mod phala;
use anyhow::Result;
use phala_crypto::ecdh::EcdhKey;
use phala_crypto::CryptoError;

use sp_core::{sr25519, Pair, H256};

pub use subxt::{tx, Config, OnlineClient, PolkadotConfig as DefaultConfig};

use contract::{builder::ContractBuilder, ContractInstance};

type Client = OnlineClient<DefaultConfig>;
type Balance = u128;
type PairSigner = tx::PairSigner<DefaultConfig, sr25519::Pair>;
type ContractId = H256;
type Nonce = [u8; 32];

pub trait KeyExtension {
    fn derive_ecdh_key(&self) -> Result<EcdhKey, CryptoError>;
}

impl KeyExtension for PairSigner {
    fn derive_ecdh_key(&self) -> Result<EcdhKey, CryptoError> {
        EcdhKey::from_secret(&self.signer().as_ref().secret.to_bytes())
    }
}

impl KeyExtension for sr25519::Pair {
    fn derive_ecdh_key(&self) -> Result<EcdhKey, CryptoError> {
        EcdhKey::from_secret(&self.as_ref().secret.to_bytes())
    }
}

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

pub trait PairExtension {
    fn consume_ref(&self) -> PairSigner;
}

impl PairExtension for PairSigner {
    fn consume_ref(&self) -> PairSigner {
        pair_signer(self.signer().clone())
    }
}

pub struct SubstrateContract {
    pub instance: ContractInstance,
}

impl SubstrateContract {
    pub fn from_account(suri: String, password: Option<String>) -> Result<Self> {
        let config: SubstrateBaseConfig = SubstrateBaseConfig::new(suri, password);

        let instance = ContractBuilder::default()
            .init_config(config)
            .sign()?
            .build()?;

        Ok(Self { instance })
    }

    pub fn get_pair_signer(&self) -> PairSigner {
        self.instance.signer.consume_ref()
    }
}

pub fn pair_signer(pair: sp_core::sr25519::Pair) -> PairSigner {
    PairSigner::new(pair)
}
