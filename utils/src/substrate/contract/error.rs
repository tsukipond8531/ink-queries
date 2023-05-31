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

use sp_runtime::DispatchError;
use std::fmt::{
    self,
    Debug,
    Display,
};

#[derive(serde::Serialize)]
pub enum ErrorVariant {
    #[serde(rename = "module_error")]
    Module(ModuleError),
    #[serde(rename = "generic_error")]
    Generic(GenericError),
}

impl From<subxt::Error> for ErrorVariant {
    fn from(error: subxt::Error) -> Self {
        match error {
            subxt::Error::Runtime(subxt::error::DispatchError::Module(module_err)) => {
                module_err
                    .details()
                    .map(|details| {
                        ErrorVariant::Module(ModuleError {
                            pallet: details.pallet().to_string(),
                            error: details.error().to_string(),
                            docs: details.docs().to_vec(),
                        })
                    })
                    .unwrap_or_else(|err| {
                        ErrorVariant::Generic(GenericError::from_message(format!(
                            "Error extracting subxt error details: {}",
                            err
                        )))
                    })
            }
            err => ErrorVariant::Generic(GenericError::from_message(err.to_string())),
        }
    }
}

impl From<anyhow::Error> for ErrorVariant {
    fn from(error: anyhow::Error) -> Self {
        Self::Generic(GenericError::from_message(format!("{error:?}")))
    }
}

impl From<&str> for ErrorVariant {
    fn from(err: &str) -> Self {
        Self::Generic(GenericError::from_message(err.to_owned()))
    }
}

#[derive(serde::Serialize)]
pub struct ModuleError {
    pub pallet: String,
    pub error: String,
    pub docs: Vec<String>,
}

#[derive(serde::Serialize)]
pub struct GenericError {
    error: String,
}

impl GenericError {
    pub fn from_message(error: String) -> Self {
        GenericError { error }
    }
}

impl ErrorVariant {
    pub fn from_dispatch_error(
        error: &DispatchError,
        metadata: &subxt::Metadata,
    ) -> anyhow::Result<ErrorVariant> {
        match error {
            DispatchError::Module(err) => {
                let details = metadata.error(err.index, err.error[0])?;
                Ok(ErrorVariant::Module(ModuleError {
                    pallet: details.pallet().to_owned(),
                    error: details.error().to_owned(),
                    docs: details.docs().to_owned(),
                }))
            }
            err => {
                Ok(ErrorVariant::Generic(GenericError::from_message(format!(
                    "DispatchError: {err:?}"
                ))))
            }
        }
    }
}

impl Debug for ErrorVariant {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        <Self as Display>::fmt(self, f)
    }
}

impl Display for ErrorVariant {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ErrorVariant::Module(err) => {
                f.write_fmt(format_args!(
                    "ModuleError: {}::{}: {:?}",
                    err.pallet, err.error, err.docs
                ))
            }
            ErrorVariant::Generic(err) => write!(f, "{}", err.error),
        }
    }
}
