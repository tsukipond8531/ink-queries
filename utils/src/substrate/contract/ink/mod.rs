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

use anyhow::{
    anyhow,
    Context,
    Ok,
    Result,
};

use std::{fs, path::PathBuf};

use subxt::Config;


use std::{
    option::Option,
    path::Path,
};

use std::str::FromStr;

pub use subxt::PolkadotConfig as DefaultConfig;
use toml::Value;

use contract_build::CrateMetadata;
use contract_metadata::ContractMetadata;
use contract_transcode::ContractMessageTranscoder;


const CONFIG_PATH: &'static str = "utils/src/substrate/contract/ink/config/config.toml";


/// Arguments required for creating and sending an extrinsic to a substrate node.
pub struct InkMeta {
    /// Path to a contract build artifact file: a raw `.wasm` file, a `.contract` bundle,
    /// or a `.json` metadata file.
    file: Option<PathBuf>,
    /// Node Url
    pub url: String,
    /// Address of the deployed contract
    pub contract_address: <DefaultConfig as Config>::AccountId,
}

impl InkMeta {
    pub fn from_config_file() -> Result<InkMeta> {
        let config_content = fs::read_to_string(CONFIG_PATH)?;
        let config: Value = toml::from_str(&config_content)?;

        macro_rules! extract {
        ($config:expr, $field:expr) => {
            $config[$field]
                .as_str()
                .ok_or_else(|| anyhow::anyhow!("Missing or invalid '{}'", $field))?
        };
        }

        let contract_address = <DefaultConfig as Config>::AccountId::from_str(extract!(config, "contract_address"))?;

        let ink_meta = InkMeta {
            file: Some(PathBuf::from(extract!(config, "contract_path"))),
            url: extract!(config, "url").to_owned(),
            contract_address,
        };


        Ok(ink_meta)
    }

    /// Load contract artifacts.
    pub fn contract_artifacts(&self) -> Result<ContractArtifacts> {
        ContractArtifacts::from_manifest_or_file(
            None,
            self.file.as_ref(),
        )
    }
}

/// Contract artifacts for use with extrinsic commands.
#[derive(Debug)]
pub struct ContractArtifacts {
    /// The original artifact path
    artifacts_path: PathBuf,
    /// The expected path of the file containing the contract metadata.
    metadata_path: PathBuf,
    /// The deserialized contract metadata if the expected metadata file exists.
    metadata: Option<ContractMetadata>,
    /// The Wasm code of the contract if available.
    pub code: Option<WasmCode>,
}

impl ContractArtifacts {
    /// Load contract artifacts.
    pub fn from_manifest_or_file(
        manifest_path: Option<&PathBuf>,
        file: Option<&PathBuf>,
    ) -> Result<ContractArtifacts> {
        let artifact_path = match (manifest_path, file) {
            (manifest_path, None) => {
                let crate_metadata = CrateMetadata::from_manifest_path(
                    manifest_path,
                    contract_build::Target::Wasm,
                )?;

                if crate_metadata.contract_bundle_path().exists() {
                    crate_metadata.contract_bundle_path()
                } else if crate_metadata.metadata_path().exists() {
                    crate_metadata.metadata_path()
                } else {
                    anyhow::bail!(
                        "Failed to find any contract artifacts in target directory. \n\
                        Run `cargo contract build --release` to generate the artifacts."
                    )
                }
            }
            (None, Some(artifact_file)) => artifact_file.clone(),
            (Some(_), Some(_)) => {
                anyhow::bail!("conflicting options: --manifest-path and --file")
            }
        };
        Self::from_artifact_path(artifact_path.as_path())
    }
    /// Given a contract artifact path, load the contract code and metadata where
    /// possible.
    fn from_artifact_path(path: &Path) -> Result<Self> {
        let (metadata_path, metadata, code) =
            match path.extension().and_then(|ext| ext.to_str()) {
                Some("contract") | Some("json") => {
                    let metadata = ContractMetadata::load(path)?;
                    let code = metadata.clone().source.wasm.map(|wasm| WasmCode(wasm.0));
                    (PathBuf::from(path), Some(metadata), code)
                }
                Some("wasm") => {
                    let file_name = path.file_stem()
                        .context("WASM bundle file has unreadable name")?
                        .to_str()
                        .context("Error parsing filename string")?;
                    let code = Some(WasmCode(fs::read(path)?));
                    let dir = path.parent().map_or_else(PathBuf::new, PathBuf::from);
                    let metadata_path = dir.join(format!("{file_name}.json"));
                    if !metadata_path.exists() {
                        (metadata_path, None, code)
                    } else {
                        let metadata = ContractMetadata::load(&metadata_path)?;
                        (metadata_path, Some(metadata), code)
                    }
                }
                Some(ext) => anyhow::bail!(
                    "Invalid artifact extension {ext}, expected `.contract`, `.json` or `.wasm`"
                ),
                None => {
                    anyhow::bail!(
                        "Artifact path has no extension, expected `.contract`, `.json`, or `.wasm`"
                    )
                }
            };
        Ok(Self {
            artifacts_path: path.into(),
            metadata_path,
            metadata,
            code,
        })
    }

    /// Get the path of the artifact file used to load the artifacts.
    pub fn artifact_path(&self) -> &Path {
        self.artifacts_path.as_path()
    }

    /// Get contract metadata, if available.
    ///
    /// ## Errors
    /// - No contract metadata could be found.
    /// - Invalid contract metadata.
    pub fn metadata(&self) -> Result<ContractMetadata> {
        self.metadata.clone().ok_or_else(|| {
            anyhow!(
                "No contract metadata found. Expected file {}",
                self.metadata_path.as_path().display()
            )
        })
    }


    /// Construct a [`ContractMessageTranscoder`] from contract metadata.
    pub fn contract_transcoder(&self) -> Result<ContractMessageTranscoder> {
        let metadata = self.metadata()?;
        ContractMessageTranscoder::try_from(metadata)
            .context("Failed to deserialize ink project metadata from contract metadata")
    }
}

/// The Wasm code of a contract.
#[derive(Debug)]
pub struct WasmCode(Vec<u8>);

impl WasmCode {
    /// The hash of the contract code: uniquely identifies the contract code on-chain.
    pub fn code_hash(&self) -> [u8; 32] {
        contract_build::code_hash(&self.0)
    }
}

