//! Deterministic JSON-RPC request builders and strict response parsers.
//!
//! This module deliberately contains no transport. Callers decide how bytes
//! reach an operator-configured endpoint and enforce the transport boundary.

use std::{fmt, str::FromStr};

use base64::{engine::general_purpose::STANDARD, Engine as _};
use serde_json::{json, Map, Value};

use crate::{pubkey::Pubkey, shape::single_line, signature::Signature};

pub const MAX_RPC_RESPONSE_BYTES: usize = 64 * 1024;
/// `getTransaction` returns the full transaction plus its metadata, including
/// program logs, so it needs a larger ceiling than the other supported methods.
/// It is still a hard bound: a larger response is refused, never truncated.
pub const MAX_TRANSACTION_RESPONSE_BYTES: usize = 256 * 1024;
/// Upper bound on `getSignaturesForAddress` results, matching the RPC maximum.
pub const MAX_SIGNATURE_RESULTS: usize = 1_000;
/// Upper bound on pre/post token-balance entries accepted from one transaction.
pub const MAX_TOKEN_BALANCES: usize = 256;
const MAX_SIMULATION_REASON_CHARS: usize = 96;

/// Cluster commitment, ordered from weakest to strongest. `Ord` is derived from
/// that declaration order, so `status >= minimum` is the "meets commitment" test.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum CommitmentLevel {
    Processed,
    Confirmed,
    Finalized,
}

impl CommitmentLevel {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Processed => "processed",
            Self::Confirmed => "confirmed",
            Self::Finalized => "finalized",
        }
    }

    /// Parse exactly the three canonical spellings. Anything else is `None`, so
    /// an unknown status from an endpoint can never be treated as a known one.
    pub fn parse_exact(value: &str) -> Option<Self> {
        match value {
            "processed" => Some(Self::Processed),
            "confirmed" => Some(Self::Confirmed),
            "finalized" => Some(Self::Finalized),
            _ => None,
        }
    }

    pub fn satisfies(self, minimum: Self) -> bool {
        self >= minimum
    }
}

impl fmt::Display for CommitmentLevel {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

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

/// One entry of a `getSignaturesForAddress` result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignatureRecord {
    pub signature: Signature,
    pub slot: u64,
    /// True when the entry reports a transaction-level error.
    pub failed: bool,
    /// `None` when the endpoint omitted `confirmationStatus`. Callers that
    /// enforce a minimum commitment must treat `None` as "not established"
    /// rather than assuming a level.
    pub confirmation_status: Option<CommitmentLevel>,
}

/// One `preTokenBalances` / `postTokenBalances` entry.
///
/// `account_index` indexes the transaction's account list. Address-table
/// lookups would extend that list beyond the message's static keys, so callers
/// must reject messages carrying lookups before trusting this index.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TokenBalance {
    pub account_index: usize,
    pub mint: Pubkey,
    pub owner: Option<Pubkey>,
    pub program_id: Option<Pubkey>,
    pub raw_amount: u64,
    pub decimals: u8,
}

/// A settled transaction as returned by `getTransaction` with base64 encoding.
///
/// Only the fields required to verify a payment from raw bytes are kept; logs,
/// inner instructions, rewards, and fee details are deliberately discarded so
/// no endpoint prose can reach a caller's output.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransactionRecord {
    pub slot: u64,
    /// The base64-decoded signed transaction wire bytes.
    pub transaction: Vec<u8>,
    /// True when `meta.err` is not null.
    pub failed: bool,
    pub pre_token_balances: Vec<TokenBalance>,
    pub post_token_balances: Vec<TokenBalance>,
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
    InvalidSignatureList,
    InvalidSignatureRecord(&'static str),
    TransactionNotFound,
    InvalidTransactionRecord(&'static str),
    InvalidTokenBalance(&'static str),
}

impl fmt::Display for RpcError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            // The ceiling depends on the method: MAX_RPC_RESPONSE_BYTES for
            // account, blockhash, simulation, and signature-list responses, and
            // MAX_TRANSACTION_RESPONSE_BYTES for a settled transaction.
            Self::ResponseTooLarge(size) => write!(
                formatter,
                "RPC response is {size} bytes, above the accepted maximum for this method"
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
            Self::InvalidSignatureList => {
                formatter.write_str("RPC returned a malformed signature list")
            }
            Self::InvalidSignatureRecord(reason) => {
                write!(
                    formatter,
                    "RPC returned an invalid signature entry: {reason}"
                )
            }
            Self::TransactionNotFound => {
                formatter.write_str("transaction was not found at the requested commitment")
            }
            Self::InvalidTransactionRecord(reason) => {
                write!(formatter, "RPC returned an invalid transaction: {reason}")
            }
            Self::InvalidTokenBalance(reason) => {
                write!(formatter, "RPC returned an invalid token balance: {reason}")
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

/// Build a `getSignaturesForAddress` request for a payment reference.
///
/// `limit` is clamped to the RPC's documented `1..=1000` range so a caller
/// misconfiguration cannot produce a request the endpoint rejects outright.
pub fn get_signatures_for_address_request(
    id: u64,
    address: &Pubkey,
    limit: u16,
    commitment: CommitmentLevel,
) -> String {
    let limit = limit.clamp(1, 1_000);
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": "getSignaturesForAddress",
        "params": [address.to_string(), {
            "commitment": commitment.as_str(),
            "limit": limit
        }]
    })
    .to_string()
}

/// Build a `getTransaction` request that returns raw transaction bytes.
///
/// `base64` is required rather than `jsonParsed`: the endpoint's interpretation
/// of a transaction is exactly what a verifier must not depend on.
pub fn get_transaction_request(
    id: u64,
    signature: &Signature,
    commitment: CommitmentLevel,
) -> String {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": "getTransaction",
        "params": [signature.to_string(), {
            "commitment": commitment.as_str(),
            "encoding": "base64",
            "maxSupportedTransactionVersion": 0
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

pub fn parse_signatures_for_address_response(
    body: &str,
    expected_id: u64,
) -> Result<Vec<SignatureRecord>, RpcError> {
    let result = parse_result_value(body, expected_id, MAX_RPC_RESPONSE_BYTES)?;
    let entries = result.as_array().ok_or(RpcError::InvalidSignatureList)?;
    if entries.len() > MAX_SIGNATURE_RESULTS {
        return Err(RpcError::InvalidSignatureList);
    }
    entries.iter().map(parse_signature_record).collect()
}

pub fn parse_transaction_response(
    body: &str,
    expected_id: u64,
) -> Result<TransactionRecord, RpcError> {
    let result = parse_result_value(body, expected_id, MAX_TRANSACTION_RESPONSE_BYTES)?;
    if result.is_null() {
        return Err(RpcError::TransactionNotFound);
    }
    let record = result
        .as_object()
        .ok_or(RpcError::InvalidTransactionRecord(
            "result must be an object",
        ))?;
    let slot =
        record
            .get("slot")
            .and_then(Value::as_u64)
            .ok_or(RpcError::InvalidTransactionRecord(
                "slot must be an integer",
            ))?;
    let encoded = record.get("transaction").and_then(Value::as_array).ok_or(
        RpcError::InvalidTransactionRecord("transaction must be base64 encoded"),
    )?;
    if encoded.len() != 2 || encoded.get(1).and_then(Value::as_str) != Some("base64") {
        return Err(RpcError::InvalidTransactionRecord(
            "transaction must be base64 encoded",
        ));
    }
    let transaction = STANDARD
        .decode(encoded.first().and_then(Value::as_str).ok_or(
            RpcError::InvalidTransactionRecord("transaction must be base64 encoded"),
        )?)
        .map_err(|_| RpcError::InvalidTransactionRecord("transaction base64 is malformed"))?;
    let meta = record
        .get("meta")
        .and_then(Value::as_object)
        .ok_or(RpcError::InvalidTransactionRecord("meta must be an object"))?;
    let failed = !meta
        .get("err")
        .ok_or(RpcError::InvalidTransactionRecord("meta.err is missing"))?
        .is_null();
    Ok(TransactionRecord {
        slot,
        transaction,
        failed,
        pre_token_balances: parse_token_balances(meta.get("preTokenBalances"))?,
        post_token_balances: parse_token_balances(meta.get("postTokenBalances"))?,
    })
}

fn parse_signature_record(value: &Value) -> Result<SignatureRecord, RpcError> {
    let entry = value
        .as_object()
        .ok_or(RpcError::InvalidSignatureRecord("entry must be an object"))?;
    let signature = entry
        .get("signature")
        .and_then(Value::as_str)
        .ok_or(RpcError::InvalidSignatureRecord(
            "signature must be a string",
        ))?
        .parse()
        .map_err(|_| RpcError::InvalidSignatureRecord("signature is not 64 base58 bytes"))?;
    let slot = entry
        .get("slot")
        .and_then(Value::as_u64)
        .ok_or(RpcError::InvalidSignatureRecord("slot must be an integer"))?;
    let failed = !entry
        .get("err")
        .ok_or(RpcError::InvalidSignatureRecord("err is missing"))?
        .is_null();
    let confirmation_status = match entry.get("confirmationStatus") {
        None | Some(Value::Null) => None,
        Some(Value::String(status)) => Some(CommitmentLevel::parse_exact(status).ok_or(
            RpcError::InvalidSignatureRecord("confirmationStatus is not a known commitment"),
        )?),
        Some(_) => {
            return Err(RpcError::InvalidSignatureRecord(
                "confirmationStatus must be a string",
            ))
        }
    };
    Ok(SignatureRecord {
        signature,
        slot,
        failed,
        confirmation_status,
    })
}

fn parse_token_balances(value: Option<&Value>) -> Result<Vec<TokenBalance>, RpcError> {
    let entries = match value {
        None | Some(Value::Null) => return Ok(Vec::new()),
        Some(Value::Array(entries)) => entries,
        Some(_) => {
            return Err(RpcError::InvalidTokenBalance(
                "token balances must be an array",
            ))
        }
    };
    if entries.len() > MAX_TOKEN_BALANCES {
        return Err(RpcError::InvalidTokenBalance("too many token balances"));
    }
    entries.iter().map(parse_token_balance).collect()
}

fn parse_token_balance(value: &Value) -> Result<TokenBalance, RpcError> {
    let entry = value
        .as_object()
        .ok_or(RpcError::InvalidTokenBalance("entry must be an object"))?;
    let account_index = entry
        .get("accountIndex")
        .and_then(Value::as_u64)
        .and_then(|index| usize::try_from(index).ok())
        .ok_or(RpcError::InvalidTokenBalance(
            "accountIndex must be an unsigned integer",
        ))?;
    let mint = entry
        .get("mint")
        .and_then(Value::as_str)
        .ok_or(RpcError::InvalidTokenBalance("mint must be a string"))?
        .parse()
        .map_err(|_| RpcError::InvalidTokenBalance("mint is not a public key"))?;
    let owner = parse_optional_pubkey(entry.get("owner"), "owner is not a public key")?;
    let program_id =
        parse_optional_pubkey(entry.get("programId"), "programId is not a public key")?;
    let ui_amount = entry
        .get("uiTokenAmount")
        .and_then(Value::as_object)
        .ok_or(RpcError::InvalidTokenBalance(
            "uiTokenAmount must be an object",
        ))?;
    // `amount` is the raw base-unit count as a decimal string. `uiAmount` and
    // `uiAmountString` are the endpoint's scaled renderings and are ignored.
    let raw_amount = ui_amount
        .get("amount")
        .and_then(Value::as_str)
        .ok_or(RpcError::InvalidTokenBalance("amount must be a string"))?
        .parse::<u64>()
        .map_err(|_| RpcError::InvalidTokenBalance("amount is not a u64 base-unit count"))?;
    let decimals = ui_amount
        .get("decimals")
        .and_then(Value::as_u64)
        .and_then(|decimals| u8::try_from(decimals).ok())
        .ok_or(RpcError::InvalidTokenBalance("decimals must be a byte"))?;
    Ok(TokenBalance {
        account_index,
        mint,
        owner,
        program_id,
        raw_amount,
        decimals,
    })
}

fn parse_optional_pubkey(
    value: Option<&Value>,
    reason: &'static str,
) -> Result<Option<Pubkey>, RpcError> {
    match value {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(text)) => text
            .parse()
            .map(Some)
            .map_err(|_| RpcError::InvalidTokenBalance(reason)),
        Some(_) => Err(RpcError::InvalidTokenBalance(reason)),
    }
}

fn parse_result(body: &str, expected_id: u64) -> Result<Map<String, Value>, RpcError> {
    parse_result_value(body, expected_id, MAX_RPC_RESPONSE_BYTES)?
        .as_object()
        .cloned()
        .ok_or(RpcError::InvalidEnvelope("result must be an object"))
}

fn parse_result_value(
    body: &str,
    expected_id: u64,
    maximum_bytes: usize,
) -> Result<Value, RpcError> {
    if body.len() > maximum_bytes {
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
        (Some(result), None) => Ok(result.clone()),
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
