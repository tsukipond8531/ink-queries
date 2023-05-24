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

use std::path::PathBuf;

/// Arguments required for creating and sending an extrinsic to a substrate node.
#[derive(Clone, Debug)]
pub struct ExtrinsicOpts {
    /// Path to a contract build artifact file: a raw `.wasm` file, a `.contract` bundle,
    /// or a `.json` metadata file.
    file: Option<PathBuf>,
    /// Path to the `Cargo.toml` of the contract.
    manifest_path: Option<PathBuf>,
    /// Websockets url of a substrate node.
    url: url::Url,
    /// Secret key URI for the account deploying the contract.
    suri: String,
    /// Password for the secret key.
    password: Option<String>,
    /// Submit the extrinsic for on-chain execution.
    execute: bool,
    /// The maximum amount of balance that can be charged from the caller to pay for the
    /// storage. consumed.
    // storage_deposit_limit: Option<BalanceVariant>,
    /// Before submitting a transaction, do not dry-run it via RPC first.
    skip_dry_run: bool,
}