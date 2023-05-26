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

mod byte_str;

use anyhow::{
    Context,
    Result,
};
use semver::Version;
use serde::{
    de,
    Deserialize,
    Serialize,
    Serializer,
};
use serde_json::{
    Map,
    Value,
};
use std::{
    fmt::{
        Display,
        Formatter,
        Result as DisplayResult,
    },
    fs::File,
    path::Path,
    str::FromStr,
};
use url::Url;

/// Smart contract metadata.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ContractMetadata {
    /// Information about the contract's Wasm code.
    pub source: Source,
    /// Metadata about the contract.
    pub contract: Contract,
    /// Additional user-defined metadata.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user: Option<User>,
    /// Raw JSON of the contract's abi metadata, generated during contract compilation.
    #[serde(flatten)]
    pub abi: Map<String, Value>,
}

impl ContractMetadata {
    /// Construct new contract metadata.
    pub fn new(
        source: Source,
        contract: Contract,
        user: Option<User>,
        abi: Map<String, Value>,
    ) -> Self {
        Self {
            source,
            contract,
            user,
            abi,
        }
    }

    pub fn remove_source_wasm_attribute(&mut self) {
        self.source.wasm = None;
    }

    /// Reads the file and tries to parse it as instance of `ContractMetadata`.
    pub fn load<P>(metadata_path: P) -> Result<Self>
        where
            P: AsRef<Path>,
    {
        let path = metadata_path.as_ref();
        let file = File::open(path)
            .context(format!("Failed to open metadata file {}", path.display()))?;
        serde_json::from_reader(file).context(format!(
            "Failed to deserialize metadata file {}",
            path.display()
        ))
    }
}

/// Representation of the Wasm code hash.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Deserialize, Serialize)]
pub struct CodeHash(
    #[serde(
    serialize_with = "byte_str::serialize_as_byte_str",
    deserialize_with = "byte_str::deserialize_from_byte_str_array"
    )]
    /// The raw bytes of the hash.
    pub [u8; 32],
);

impl From<[u8; 32]> for CodeHash {
    fn from(value: [u8; 32]) -> Self {
        CodeHash(value)
    }
}

/// Information about the contract's Wasm code.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Source {
    /// The hash of the contract's Wasm code.
    pub hash: CodeHash,
    /// The language used to write the contract.
    pub language: SourceLanguage,
    /// The compiler used to compile the contract.
    pub compiler: SourceCompiler,
    /// The actual Wasm code of the contract, for optionally bundling the code
    /// with the metadata.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub wasm: Option<SourceWasm>,
    /// Extra information about the environment in which the contract was built.
    ///
    /// Useful for producing deterministic builds.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub build_info: Option<Map<String, Value>>,
}

impl Source {
    /// Constructs a new InkProjectSource.
    pub fn new(
        wasm: Option<SourceWasm>,
        hash: CodeHash,
        language: SourceLanguage,
        compiler: SourceCompiler,
        build_info: Option<Map<String, Value>>,
    ) -> Self {
        Source {
            hash,
            language,
            compiler,
            wasm,
            build_info,
        }
    }
}

/// The bytes of the compiled Wasm smart contract.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct SourceWasm(
    #[serde(
    serialize_with = "byte_str::serialize_as_byte_str",
    deserialize_with = "byte_str::deserialize_from_byte_str"
    )]
    /// The raw bytes of the Wasm code.
    pub Vec<u8>,
);

impl SourceWasm {
    /// Constructs a new `SourceWasm`.
    pub fn new(wasm: Vec<u8>) -> Self {
        SourceWasm(wasm)
    }
}

impl Display for SourceWasm {
    fn fmt(&self, f: &mut Formatter<'_>) -> DisplayResult {
        write!(f, "0x").expect("failed writing to string");
        for byte in &self.0 {
            write!(f, "{byte:02x}").expect("failed writing to string");
        }
        write!(f, "")
    }
}

/// The language and version in which a smart contract is written.
#[derive(Clone, Debug)]
pub struct SourceLanguage {
    /// The language used to write the contract.
    pub language: Language,
    /// The version of the language used to write the contract.
    pub version: Version,
}

impl SourceLanguage {
    /// Constructs a new SourceLanguage.
    pub fn new(language: Language, version: Version) -> Self {
        SourceLanguage { language, version }
    }
}

impl Serialize for SourceLanguage {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where
            S: Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for SourceLanguage {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
        where
            D: de::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        FromStr::from_str(&s).map_err(de::Error::custom)
    }
}

impl Display for SourceLanguage {
    fn fmt(&self, f: &mut Formatter<'_>) -> DisplayResult {
        write!(f, "{} {}", self.language, self.version)
    }
}

impl FromStr for SourceLanguage {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut parts = s.split_whitespace();

        let language = parts
            .next()
            .ok_or_else(|| {
                format!(
                    "SourceLanguage: Expected format '<language> <version>', got '{s}'"
                )
            })
            .and_then(FromStr::from_str)?;

        let version = parts
            .next()
            .ok_or_else(|| {
                format!(
                    "SourceLanguage: Expected format '<language> <version>', got '{s}'"
                )
            })
            .and_then(|v| {
                <Version as FromStr>::from_str(v)
                    .map_err(|e| format!("Error parsing version {e}"))
            })?;

        Ok(Self { language, version })
    }
}

/// The language in which the smart contract is written.
#[derive(Clone, Debug)]
pub enum Language {
    Ink,
    Solidity,
    AssemblyScript,
}

impl Display for Language {
    fn fmt(&self, f: &mut Formatter<'_>) -> DisplayResult {
        match self {
            Self::Ink => write!(f, "ink!"),
            Self::Solidity => write!(f, "Solidity"),
            Self::AssemblyScript => write!(f, "AssemblyScript"),
        }
    }
}

impl FromStr for Language {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "ink!" => Ok(Self::Ink),
            "Solidity" => Ok(Self::Solidity),
            "AssemblyScript" => Ok(Self::AssemblyScript),
            _ => Err(format!("Invalid language '{s}'")),
        }
    }
}

/// A compiler used to compile a smart contract.
#[derive(Clone, Debug)]
pub struct SourceCompiler {
    /// The compiler used to compile the smart contract.
    pub compiler: Compiler,
    /// The version of the compiler used to compile the smart contract.
    pub version: Version,
}

impl Display for SourceCompiler {
    fn fmt(&self, f: &mut Formatter<'_>) -> DisplayResult {
        write!(f, "{} {}", self.compiler, self.version)
    }
}

impl FromStr for SourceCompiler {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut parts = s.split_whitespace();

        let compiler = parts
            .next()
            .ok_or_else(|| {
                format!(
                    "SourceCompiler: Expected format '<compiler> <version>', got '{s}'"
                )
            })
            .and_then(FromStr::from_str)?;

        let version = parts
            .next()
            .ok_or_else(|| {
                format!(
                    "SourceCompiler: Expected format '<compiler> <version>', got '{s}'"
                )
            })
            .and_then(|v| {
                <Version as FromStr>::from_str(v)
                    .map_err(|e| format!("Error parsing version {e}"))
            })?;

        Ok(Self { compiler, version })
    }
}

impl Serialize for SourceCompiler {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where
            S: Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for SourceCompiler {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
        where
            D: de::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        FromStr::from_str(&s).map_err(de::Error::custom)
    }
}

impl SourceCompiler {
    pub fn new(compiler: Compiler, version: Version) -> Self {
        SourceCompiler { compiler, version }
    }
}

/// Compilers used to compile a smart contract.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub enum Compiler {
    /// The rust compiler.
    RustC,
    /// The solang compiler.
    Solang,
}

impl Display for Compiler {
    fn fmt(&self, f: &mut Formatter<'_>) -> DisplayResult {
        match self {
            Self::RustC => write!(f, "rustc"),
            Self::Solang => write!(f, "solang"),
        }
    }
}

impl FromStr for Compiler {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "rustc" => Ok(Self::RustC),
            "solang" => Ok(Self::Solang),
            _ => Err(format!("Invalid compiler '{s}'")),
        }
    }
}

/// Metadata about a smart contract.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Contract {
    /// The name of the smart contract.
    pub name: String,
    /// The version of the smart contract.
    pub version: Version,
    /// The authors of the smart contract.
    pub authors: Vec<String>,
    /// The description of the smart contract.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Link to the documentation of the smart contract.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub documentation: Option<Url>,
    /// Link to the code repository of the smart contract.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub repository: Option<Url>,
    /// Link to the homepage of the smart contract.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub homepage: Option<Url>,
    /// The license of the smart contract.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub license: Option<String>,
}

impl Contract {
    pub fn builder() -> ContractBuilder {
        ContractBuilder::default()
    }
}

/// Additional user defined metadata, can be any valid json.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct User {
    /// Raw json of user defined metadata.
    #[serde(flatten)]
    pub json: Map<String, Value>,
}

impl User {
    /// Constructs new user metadata.
    pub fn new(json: Map<String, Value>) -> Self {
        User { json }
    }
}

/// Builder for contract metadata
#[derive(Default)]
pub struct ContractBuilder {
    name: Option<String>,
    version: Option<Version>,
    authors: Option<Vec<String>>,
    description: Option<String>,
    documentation: Option<Url>,
    repository: Option<Url>,
    homepage: Option<Url>,
    license: Option<String>,
}

impl ContractBuilder {
    /// Set the contract name (required)
    pub fn name<S>(&mut self, name: S) -> &mut Self
        where
            S: AsRef<str>,
    {
        if self.name.is_some() {
            panic!("name has already been set")
        }
        self.name = Some(name.as_ref().to_string());
        self
    }

    /// Set the contract version (required)
    pub fn version(&mut self, version: Version) -> &mut Self {
        if self.version.is_some() {
            panic!("version has already been set")
        }
        self.version = Some(version);
        self
    }

    /// Set the contract version (required)
    pub fn authors<I, S>(&mut self, authors: I) -> &mut Self
        where
            I: IntoIterator<Item=S>,
            S: AsRef<str>,
    {
        if self.authors.is_some() {
            panic!("authors has already been set")
        }

        let authors = authors
            .into_iter()
            .map(|s| s.as_ref().to_string())
            .collect::<Vec<_>>();

        if authors.is_empty() {
            panic!("must have at least one author")
        }

        self.authors = Some(authors);
        self
    }

    /// Set the contract description (optional)
    pub fn description<S>(&mut self, description: S) -> &mut Self
        where
            S: AsRef<str>,
    {
        if self.description.is_some() {
            panic!("description has already been set")
        }
        self.description = Some(description.as_ref().to_string());
        self
    }

    /// Set the contract documentation url (optional)
    pub fn documentation(&mut self, documentation: Url) -> &mut Self {
        if self.documentation.is_some() {
            panic!("documentation is already set")
        }
        self.documentation = Some(documentation);
        self
    }

    /// Set the contract repository url (optional)
    pub fn repository(&mut self, repository: Url) -> &mut Self {
        if self.repository.is_some() {
            panic!("repository is already set")
        }
        self.repository = Some(repository);
        self
    }

    /// Set the contract homepage url (optional)
    pub fn homepage(&mut self, homepage: Url) -> &mut Self {
        if self.homepage.is_some() {
            panic!("homepage is already set")
        }
        self.homepage = Some(homepage);
        self
    }

    /// Set the contract license (optional)
    pub fn license<S>(&mut self, license: S) -> &mut Self
        where
            S: AsRef<str>,
    {
        if self.license.is_some() {
            panic!("license has already been set")
        }
        self.license = Some(license.as_ref().to_string());
        self
    }

    /// Finalize construction of the [`ContractMetadata`].
    ///
    /// Returns an `Err` if any required fields missing.
    pub fn build(&self) -> Result<Contract, String> {
        let mut required = Vec::new();

        if let (Some(name), Some(version), Some(authors)) =
            (&self.name, &self.version, &self.authors)
        {
            Ok(Contract {
                name: name.to_string(),
                version: version.clone(),
                authors: authors.to_vec(),
                description: self.description.clone(),
                documentation: self.documentation.clone(),
                repository: self.repository.clone(),
                homepage: self.homepage.clone(),
                license: self.license.clone(),
            })
        } else {
            if self.name.is_none() {
                required.push("name");
            }
            if self.version.is_none() {
                required.push("version")
            }
            if self.authors.is_none() {
                required.push("authors")
            }
            Err(format!(
                "Missing required non-default fields: {}",
                required.join(", ")
            ))
        }
    }
}

