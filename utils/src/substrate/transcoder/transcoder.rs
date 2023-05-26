// Copyright 2018-2020 Parity Technologies (UK) Ltd.
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

use super::{
    decode::Decoder,
    encode::Encoder,
    env_types::{
        self,
        CustomTypeDecoder,
        CustomTypeEncoder,
        EnvTypesTranscoder,
        PathKey,
        TypesByPath,
    },
    scon::Value,
    AccountId32,
};

use anyhow::Result;
use scale::Output;
use scale_info::{
    PortableRegistry,
    TypeInfo,
};
use std::{
    collections::HashMap,
    fmt::Debug,
};

/// Encode strings to SCALE encoded output.
/// Decode SCALE encoded input into `Value` objects.
pub struct Transcoder {
    env_types: EnvTypesTranscoder,
}

impl Transcoder {
    pub fn new(env_types: EnvTypesTranscoder) -> Self {
        Self { env_types }
    }

    pub fn encode<O>(
        &self,
        registry: &PortableRegistry,
        type_id: u32,
        value: &Value,
        output: &mut O,
    ) -> Result<()>
        where
            O: Output + Debug,
    {
        let encoder = Encoder::new(registry, &self.env_types);
        encoder.encode(type_id, value, output)
    }

    pub fn decode(
        &self,
        registry: &PortableRegistry,
        type_id: u32,
        input: &mut &[u8],
    ) -> Result<Value> {
        let decoder = Decoder::new(registry, &self.env_types);
        decoder.decode(type_id, input)
    }
}

/// Construct a [`Transcoder`], allows registering custom transcoders for certain types.
pub struct TranscoderBuilder {
    types_by_path: TypesByPath,
    encoders: HashMap<u32, Box<dyn CustomTypeEncoder>>,
    decoders: HashMap<u32, Box<dyn CustomTypeDecoder>>,
}

impl TranscoderBuilder {
    pub fn new(registry: &PortableRegistry) -> Self {
        let types_by_path = registry
            .types
            .iter()
            .map(|ty| (PathKey::from(&ty.ty.path), ty.id))
            .collect::<TypesByPath>();
        Self {
            types_by_path,
            encoders: HashMap::new(),
            decoders: HashMap::new(),
        }
    }

    pub fn with_default_custom_type_transcoders(self) -> Self {
        self.register_custom_type_transcoder::<AccountId32, _>(env_types::AccountId)
            .register_custom_type_decoder::<primitive_types::H256, _>(env_types::Hash)
    }

    pub fn register_custom_type_transcoder<T, U>(self, transcoder: U) -> Self
        where
            T: TypeInfo + 'static,
            U: CustomTypeEncoder + CustomTypeDecoder + Clone + 'static,
    {
        self.register_custom_type_encoder::<T, U>(transcoder.clone())
            .register_custom_type_decoder::<T, U>(transcoder)
    }

    pub fn register_custom_type_encoder<T, U>(self, encoder: U) -> Self
        where
            T: TypeInfo + 'static,
            U: CustomTypeEncoder + 'static,
    {
        let mut this = self;

        let path_key = PathKey::from_type::<T>();
        let type_id = this.types_by_path.get(&path_key);

        match type_id {
            Some(type_id) => {
                let existing = this.encoders.insert(*type_id, Box::new(encoder));

                if existing.is_some() {
                    panic!(
                        "Attempted to register encoder with existing type id {type_id:?}"
                    );
                }
            }
            None => {
                // if the type is not present in the registry, it just means it has not
                // been used.
            }
        }
        this
    }

    pub fn register_custom_type_decoder<T, U>(self, encoder: U) -> Self
        where
            T: TypeInfo + 'static,
            U: CustomTypeDecoder + 'static,
    {
        let mut this = self;

        let path_key = PathKey::from_type::<T>();
        let type_id = this.types_by_path.get(&path_key);

        match type_id {
            Some(type_id) => {
                let existing = this.decoders.insert(*type_id, Box::new(encoder));

                if existing.is_some() {
                    panic!(
                        "Attempted to register decoder with existing type id {type_id:?}"
                    );
                }
            }
            None => {
                // if the type is not present in the registry, it just means it has not
                // been used.
            }
        }
        this
    }

    pub fn done(self) -> Transcoder {
        let env_types_transcoder = EnvTypesTranscoder::new(self.encoders, self.decoders);
        Transcoder::new(env_types_transcoder)
    }
}

