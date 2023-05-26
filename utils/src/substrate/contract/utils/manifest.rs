// Copyright 2018-2022 Parity Technologies (UK) Ltd.
// This file is part of cargo-contract.
//
// cargo-contract is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// cargo-contract is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with cargo-contract.  If not, see <http://www.gnu.org/licenses/>.

use anyhow::Result;


use std::{
    convert::TryFrom,
    path::{
        Path,
        PathBuf,
    },
};

const MANIFEST_FILE: &str = "Cargo.toml";
const LEGACY_METADATA_PACKAGE_PATH: &str = ".ink/abi_gen";
const METADATA_PACKAGE_PATH: &str = ".ink/metadata_gen";

/// Path to a `Cargo.toml` file
#[derive(Clone, Debug)]
pub struct ManifestPath {
    path: PathBuf,
}

impl ManifestPath {
    /// Create a new [`ManifestPath`], errors if not path to `Cargo.toml`
    pub fn new<P: AsRef<Path>>(path: P) -> Result<Self> {
        let manifest = path.as_ref();
        if let Some(file_name) = manifest.file_name() {
            if file_name != MANIFEST_FILE {
                anyhow::bail!("Manifest file must be a Cargo.toml")
            }
        }
        Ok(ManifestPath {
            path: manifest.into(),
        })
    }

    /// Create an arg `--manifest-path=` for `cargo` command
    pub fn cargo_arg(&self) -> Result<String> {
        let path = self.path.canonicalize().map_err(|err| {
            anyhow::anyhow!("Failed to canonicalize {:?}: {:?}", self.path, err)
        })?;
        Ok(format!("--manifest-path={}", path.to_string_lossy()))
    }

    /// The directory path of the manifest path.
    ///
    /// Returns `None` if the path is just the plain file name `Cargo.toml`
    pub fn directory(&self) -> Option<&Path> {
        let just_a_file_name =
            self.path.iter().collect::<Vec<_>>() == vec![Path::new(MANIFEST_FILE)];
        if !just_a_file_name {
            self.path.parent()
        } else {
            None
        }
    }

    /// Returns the absolute directory path of the manifest.
    pub fn absolute_directory(&self) -> Result<PathBuf, std::io::Error> {
        let directory = match self.directory() {
            Some(dir) => dir,
            None => Path::new("./"),
        };
        directory.canonicalize()
    }
}

impl<P> TryFrom<Option<P>> for ManifestPath
    where
        P: AsRef<Path>,
{
    type Error = anyhow::Error;

    fn try_from(value: Option<P>) -> Result<Self, Self::Error> {
        value.map_or(Ok(Default::default()), ManifestPath::new)
    }
}

impl Default for ManifestPath {
    fn default() -> ManifestPath {
        ManifestPath::new(MANIFEST_FILE).expect("it's a valid manifest file")
    }
}

impl AsRef<Path> for ManifestPath {
    fn as_ref(&self) -> &Path {
        self.path.as_ref()
    }
}

impl From<ManifestPath> for PathBuf {
    fn from(path: ManifestPath) -> Self {
        path.path
    }
}