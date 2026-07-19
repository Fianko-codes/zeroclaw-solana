//! Deterministic JSON-RPC request builders and strict response parsers.
//!
//! This module deliberately contains no transport. Callers decide how bytes
//! reach an operator-configured endpoint and enforce the transport boundary.

use std::{fmt, str::FromStr};

use base64::{engine::general_purpose::STANDARD, Engine as _};
use serde_json::{json, Map, Value};

use crate::{pubkey::Pubkey, shape::single_line};

pub const MAX_RPC_RESPONSE_BYTES: usize = 64 * 1024;
const MAX_SIMULATION_REASON_CHARS: usize = 96;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LatestBlockhash {
    pub blockhash: [u8; 32],
    pub last_valid_block_height: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RpcAccount {
    pub owner: Pubkey,
    pub executable: bool,
    pub data: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SimulationResult {
    pub error: Option<String>,
    /// True when the RPC reported a `replacementBlockhash`, i.e. it replaced the
    /// transaction's blockhash. Durable-nonce callers request no replacement and
    /// must reject a transaction whose blockhash was replaced anyway, since that
    /// would simulate a different lifetime mechanism than the returned bytes.
    pub replaced_blockhash: bool,
}

impl SimulationResult {
    pub fn is_success(&self) -> bool {
        self.error.is_none()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RpcError {
    ResponseTooLarge(usize),
    InvalidJson,
    InvalidEnvelope(&'static str),
    MismatchedId {
        expected: u64,
        received: Option<u64>,
    },
    Remote {
        code: i64,
    },
    MissingResult,
    AccountNotFound,
    InvalidOwner,
    InvalidAccountDataEncoding,
    InvalidAccountData,
    InvalidBlockhash,
    InvalidBlockHeight,
    InvalidSimulationResult,
}

impl fmt::Display for RpcError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ResponseTooLarge(size) => write!(
                formatter,
                "RPC response is {size} bytes; maximum is {MAX_RPC_RESPONSE_BYTES}"
            ),
            Self::InvalidJson => formatter.write_str("RPC returned malformed JSON"),
            Self::InvalidEnvelope(reason) => {
                write!(formatter, "invalid JSON-RPC envelope: {reason}")
            }
            Self::MismatchedId { expected, received } => match received {
                Some(received) => write!(
                    formatter,
                    "JSON-RPC response id {received} does not match request id {expected}"
                ),
                None => formatter.write_str("JSON-RPC response id is not an unsigned integer"),
            },
            Self::Remote { code } => write!(formatter, "RPC returned JSON-RPC error code {code}"),
            Self::MissingResult => formatter.write_str("JSON-RPC response is missing result"),
            Self::AccountNotFound => formatter.write_str("mint account was not found"),
            Self::InvalidOwner => formatter.write_str("RPC account owner is not a public key"),
            Self::InvalidAccountDataEncoding => {
                formatter.write_str("RPC account data must use base64 encoding")
            }
            Self::InvalidAccountData => formatter.write_str("RPC account data is invalid"),
            Self::InvalidBlockhash => formatter.write_str("RPC returned an invalid blockhash"),
            Self::InvalidBlockHeight => {
                formatter.write_str("RPC returned an invalid last valid block height")
            }
            Self::InvalidSimulationResult => {
                formatter.write_str("RPC returned a malformed simulation result")
            }
        }
    }
}

impl std::error::Error for RpcError {}

pub fn get_latest_blockhash_request(id: u64) -> String {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": "getLatestBlockhash",
        "params": [{"commitment": "confirmed"}]
    })
    .to_string()
}

pub fn get_account_info_request(id: u64, address: &Pubkey) -> String {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": "getAccountInfo",
        "params": [address.to_string(), {"commitment": "confirmed", "encoding": "base64"}]
    })
    .to_string()
}

pub fn simulate_transaction_request(id: u64, transaction_base64: &str) -> String {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": "simulateTransaction",
        "params": [transaction_base64, {
            "commitment": "confirmed",
            "encoding": "base64",
            "replaceRecentBlockhash": true,
            "sigVerify": false
        }]
    })
    .to_string()
}

/// Durable-nonce simulation request. The stored nonce value must remain in the
/// message `recent_blockhash` field, so `replaceRecentBlockhash` MUST be false;
/// setting it true would swap in a fresh cluster blockhash and bypass the
/// durable-nonce validation path (the two options are mutually exclusive with
/// `sigVerify`, which stays false for an unsigned transaction).
pub fn simulate_durable_transaction_request(id: u64, transaction_base64: &str) -> String {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": "simulateTransaction",
        "params": [transaction_base64, {
            "commitment": "confirmed",
            "encoding": "base64",
            "replaceRecentBlockhash": false,
            "sigVerify": false
        }]
    })
    .to_string()
}

pub fn parse_latest_blockhash_response(
    body: &str,
    expected_id: u64,
) -> Result<LatestBlockhash, RpcError> {
    let result = parse_result(body, expected_id)?;
    let value = result
        .get("value")
        .and_then(Value::as_object)
        .ok_or(RpcError::MissingResult)?;
    let blockhash_text = value
        .get("blockhash")
        .and_then(Value::as_str)
        .ok_or(RpcError::InvalidBlockhash)?;
    let blockhash = Pubkey::from_str(blockhash_text)
        .map_err(|_| RpcError::InvalidBlockhash)?
        .to_bytes();
    let last_valid_block_height = value
        .get("lastValidBlockHeight")
        .and_then(Value::as_u64)
        .ok_or(RpcError::InvalidBlockHeight)?;
    Ok(LatestBlockhash {
        blockhash,
        last_valid_block_height,
    })
}

pub fn parse_account_info_response(body: &str, expected_id: u64) -> Result<RpcAccount, RpcError> {
    let result = parse_result(body, expected_id)?;
    let value = result.get("value").ok_or(RpcError::MissingResult)?;
    if value.is_null() {
        return Err(RpcError::AccountNotFound);
    }
    let account = value.as_object().ok_or(RpcError::InvalidAccountData)?;
    let owner = account
        .get("owner")
        .and_then(Value::as_str)
        .ok_or(RpcError::InvalidOwner)?
        .parse()
        .map_err(|_| RpcError::InvalidOwner)?;
    let executable = account
        .get("executable")
        .and_then(Value::as_bool)
        .ok_or(RpcError::InvalidAccountData)?;
    let encoded = account
        .get("data")
        .and_then(Value::as_array)
        .ok_or(RpcError::InvalidAccountDataEncoding)?;
    if encoded.len() != 2 || encoded.get(1).and_then(Value::as_str) != Some("base64") {
        return Err(RpcError::InvalidAccountDataEncoding);
    }
    let data = STANDARD
        .decode(
            encoded
                .first()
                .and_then(Value::as_str)
                .ok_or(RpcError::InvalidAccountDataEncoding)?,
        )
        .map_err(|_| RpcError::InvalidAccountData)?;
    if let Some(space) = account.get("space") {
        let space = space.as_u64().ok_or(RpcError::InvalidAccountData)?;
        if usize::try_from(space).ok() != Some(data.len()) {
            return Err(RpcError::InvalidAccountData);
        }
    }
    Ok(RpcAccount {
        owner,
        executable,
        data,
    })
}

pub fn parse_simulation_response(
    body: &str,
    expected_id: u64,
) -> Result<SimulationResult, RpcError> {
    let result = parse_result(body, expected_id)?;
    let value = result
        .get("value")
        .and_then(Value::as_object)
        .ok_or(RpcError::InvalidSimulationResult)?;
    let error = value.get("err").ok_or(RpcError::InvalidSimulationResult)?;
    let replaced_blockhash = value
        .get("replacementBlockhash")
        .is_some_and(|value| !value.is_null());
    Ok(SimulationResult {
        error: (!error.is_null()).then(|| simulation_reason(error)),
        replaced_blockhash,
    })
}

fn parse_result(body: &str, expected_id: u64) -> Result<Map<String, Value>, RpcError> {
    if body.len() > MAX_RPC_RESPONSE_BYTES {
        return Err(RpcError::ResponseTooLarge(body.len()));
    }
    let value: Value = serde_json::from_str(body).map_err(|_| RpcError::InvalidJson)?;
    let envelope = value
        .as_object()
        .ok_or(RpcError::InvalidEnvelope("root must be an object"))?;
    if envelope.get("jsonrpc").and_then(Value::as_str) != Some("2.0") {
        return Err(RpcError::InvalidEnvelope("jsonrpc must equal 2.0"));
    }
    let received_id = envelope.get("id").and_then(Value::as_u64);
    if received_id != Some(expected_id) {
        return Err(RpcError::MismatchedId {
            expected: expected_id,
            received: received_id,
        });
    }

    let result = envelope.get("result");
    let remote_error = envelope.get("error");
    match (result, remote_error) {
        (Some(_), Some(_)) => Err(RpcError::InvalidEnvelope(
            "response must not contain both result and error",
        )),
        (None, None) => Err(RpcError::MissingResult),
        (None, Some(error)) => {
            let error = error
                .as_object()
                .ok_or(RpcError::InvalidEnvelope("error must be an object"))?;
            let code = error
                .get("code")
                .and_then(Value::as_i64)
                .ok_or(RpcError::InvalidEnvelope("error code must be an integer"))?;
            if error.get("message").and_then(Value::as_str).is_none() {
                return Err(RpcError::InvalidEnvelope("error message must be a string"));
            }
            Err(RpcError::Remote { code })
        }
        (Some(result), None) => result
            .as_object()
            .cloned()
            .ok_or(RpcError::InvalidEnvelope("result must be an object")),
    }
}

fn simulation_reason(error: &Value) -> String {
    let raw = match error {
        Value::String(value) => value.as_str(),
        Value::Object(object) if object.len() == 1 => object
            .keys()
            .next()
            .map(String::as_str)
            .unwrap_or("transaction error"),
        _ => "transaction error",
    };
    let bounded = single_line(raw, MAX_SIMULATION_REASON_CHARS);
    if bounded.is_empty() {
        "transaction error".to_string()
    } else {
        bounded
    }
}
