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
    Hex,
    Tuple,
    Value,
};
use escape8259::unescape;
use nom::{
    branch::alt,
    bytes::complete::{
        tag,
        take_while1,
    },
    character::complete::{
        alphanumeric1,
        anychar,
        char,
        digit1,
        hex_digit1,
        multispace0,
    },
    multi::{
        many0,
        separated_list0,
    },
    sequence::{
        delimited,
        pair,
    },
    AsChar,
    IResult,
    Parser,
};
use nom_supreme::{
    error::ErrorTree,
    ParserExt,
};
use std::str::FromStr as _;

/// Attempt to parse a SCON value
pub fn parse_value(input: &str) -> anyhow::Result<Value> {
    let (_, value) = scon_value(input)
        .map_err(|err| anyhow::anyhow!("Error parsing Value: {}", err))?;
    Ok(value)
}

fn scon_value(input: &str) -> IResult<&str, Value, ErrorTree<&str>> {
    ws(alt((
        scon_unit,
        scon_hex,
        scon_seq,
        scon_string,
        scon_literal,
        scon_integer,
        scon_bool,
        scon_char,
        scon_unit_tuple,
    )))
        .context("Value")
        .parse(input)
}

fn scon_string(input: &str) -> IResult<&str, Value, ErrorTree<&str>> {
    #[derive(Debug)]
    struct UnescapeError(String);
    impl std::error::Error for UnescapeError {}

    impl std::fmt::Display for UnescapeError {
        fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
            write!(f, "Error unescaping string '{}'", self.0)
        }
    }

    // One or more unescaped text characters
    let nonescaped_string = take_while1(|c| {
        let cv = c as u32;
        // A character that is:
        // NOT a control character (0x00 - 0x1F)
        // NOT a quote character (0x22)
        // NOT a backslash character (0x5C)
        // Is within the unicode range (< 0x10FFFF) (this is already guaranteed by Rust
        // char)
        (cv >= 0x20) && (cv != 0x22) && (cv != 0x5C)
    });

    // There are only two types of escape allowed by RFC 8259.
    // - single-character escapes \" \\ \/ \b \f \n \r \t
    // - general-purpose \uXXXX
    // Note: we don't enforce that escape codes are valid here.
    // There must be a decoder later on.
    let escape_code = pair(
        tag("\\"),
        alt((
            tag("\""),
            tag("\\"),
            tag("/"),
            tag("b"),
            tag("f"),
            tag("n"),
            tag("r"),
            tag("t"),
            tag("u"),
        )),
    )
        .recognize();

    many0(alt((nonescaped_string, escape_code)))
        .recognize()
        .delimited_by(tag("\""))
        .map_res::<_, _, UnescapeError>(|s: &str| {
            let unescaped = unescape(s).map_err(|_| UnescapeError(s.to_string()))?;
            Ok(Value::String(unescaped))
        })
        .parse(input)
}

fn rust_ident(input: &str) -> IResult<&str, &str, ErrorTree<&str>> {
    let alpha_or_underscore = anychar.verify(|c: &char| c.is_alpha() || *c == '_');

    take_while1(|c: char| c.is_alphanumeric() || c == '_')
        .preceded_by(alpha_or_underscore.peek())
        .parse(input)
}

/// Parse a signed or unsigned integer literal, supports optional Rust style underscore
/// separators.
fn scon_integer(input: &str) -> IResult<&str, Value, ErrorTree<&str>> {
    let sign = alt((char('+'), char('-')));
    pair(sign.opt(), separated_list0(char('_'), digit1))
        .map_res(|(sign, parts)| {
            let digits = parts.join("");
            if let Some(sign) = sign {
                let s = format!("{sign}{digits}");
                s.parse::<i128>().map(Value::Int)
            } else {
                digits.parse::<u128>().map(Value::UInt)
            }
        })
        .parse(input)
}

fn scon_unit(input: &str) -> IResult<&str, Value, ErrorTree<&str>> {
    let (i, _) = tag("()").parse(input)?;
    Ok((i, Value::Unit))
}

fn scon_bool(input: &str) -> IResult<&str, Value, ErrorTree<&str>> {
    alt((
        tag("false").value(Value::Bool(false)),
        tag("true").value(Value::Bool(true)),
    ))
        .parse(input)
}

fn scon_char(input: &str) -> IResult<&str, Value, ErrorTree<&str>> {
    anychar
        .delimited_by(char('\''))
        .map(Value::Char)
        .parse(input)
}

fn scon_seq(input: &str) -> IResult<&str, Value, ErrorTree<&str>> {
    separated_list0(ws(char(',')), scon_value)
        .preceded_by(ws(char('[')))
        .terminated(pair(ws(char(',')).opt(), ws(char(']'))))
        .map(|seq| Value::Seq(seq.into()))
        .parse(input)
}


/// Parse a rust ident on its own which could represent a struct with no fields or a enum
/// unit variant e.g. "None"
fn scon_unit_tuple(input: &str) -> IResult<&str, Value, ErrorTree<&str>> {
    rust_ident
        .map(|ident| Value::Tuple(Tuple::new(Some(ident), Vec::new())))
        .parse(input)
}


fn scon_hex(input: &str) -> IResult<&str, Value, ErrorTree<&str>> {
    tag("0x")
        .precedes(hex_digit1)
        .map_res::<_, _, hex::FromHexError>(|byte_str| {
            let hex = Hex::from_str(byte_str)?;
            Ok(Value::Hex(hex))
        })
        .parse(input)
}

/// Parse any alphanumeric literal with more than 39 characters (the length of
/// `u128::MAX`)
///
/// This is suitable for capturing e.g. Base58 encoded literals for Substrate addresses
fn scon_literal(input: &str) -> IResult<&str, Value, ErrorTree<&str>> {
    const MAX_UINT_LEN: usize = 39;
    alphanumeric1
        .verify(|s: &&str| s.len() > MAX_UINT_LEN)
        .recognize()
        .map(|literal: &str| Value::Literal(literal.to_string()))
        .parse(input)
}

fn ws<F, I, O, E>(f: F) -> impl FnMut(I) -> IResult<I, O, E>
    where
        F: FnMut(I) -> IResult<I, O, E>,
        I: nom::InputTakeAtPosition,
        <I as nom::InputTakeAtPosition>::Item: AsChar + Clone,
        E: nom::error::ParseError<I>,
{
    delimited(multispace0, f, multispace0)
}
