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
pub mod crate_metadata;
mod manifest;

use subxt::Config;
use crate::substrate::DefaultConfig;
use anyhow::Result;


/// The list of targets that ink! supports.
#[derive(
Eq,
PartialEq,
Copy,
Clone,
Debug,
Default,
serde::Serialize,
serde::Deserialize,
strum::EnumIter,
)]
pub enum Target {
    /// WebAssembly
    #[default]
    Wasm,
    /// RISC-V: Experimental
    RiscV,
}

impl Target {
    /// The target string to be passed to rustc in order to build for this target.
    pub fn llvm_target(&self) -> &'static str {
        match self {
            Self::Wasm => "wasm32-unknown-unknown",
            Self::RiscV => "riscv32i-unknown-none-elf",
        }
    }

    /// Target specific flags to be set to `CARGO_ENCODED_RUSTFLAGS` while building.
    pub fn rustflags(&self) -> &'static str {
        match self {
            Self::Wasm => "-C\x1flink-arg=-zstack-size=65536\x1f-C\x1flink-arg=--import-memory\x1f-Clinker-plugin-lto\x1f-C\x1ftarget-cpu=mvp",
            Self::RiscV => "-Clinker-plugin-lto",
        }
    }

    /// The file extension that is used by rustc when outputting the binary.
    pub fn source_extension(&self) -> &'static str {
        match self {
            Self::Wasm => "wasm",
            Self::RiscV => "",
        }
    }

    // The file extension that is used to store the post processed binary.
    pub fn dest_extension(&self) -> &'static str {
        match self {
            Self::Wasm => "wasm",
            Self::RiscV => "riscv",
        }
    }
}


fn blake2_hash(code: &[u8]) -> [u8; 32] {
    use blake2::digest::{
        consts::U32,
        Digest as _,
    };
    let mut blake2 = blake2::Blake2b::<U32>::new();
    blake2.update(code);
    let result = blake2.finalize();
    result.into()
}

/// Returns the blake2 hash of the code slice.
pub fn code_hash(code: &[u8]) -> [u8; 32] {
    blake2_hash(code)
}

/// Decode hex string with or without 0x prefix
pub fn decode_hex(input: &str) -> Result<Vec<u8>, hex::FromHexError> {
    hex::decode(input.trim_start_matches("0x"))
}

/// Parse a hex encoded 32 byte hash. Returns error if not exactly 32 bytes.
pub fn parse_code_hash(input: &str) -> Result<<DefaultConfig as Config>::Hash> {
    let bytes = decode_hex(input)?;
    if bytes.len() != 32 {
        anyhow::bail!("Code hash should be 32 bytes in length")
    }
    let mut arr = [0u8; 32];
    arr.copy_from_slice(&bytes);
    Ok(arr.into())
}
