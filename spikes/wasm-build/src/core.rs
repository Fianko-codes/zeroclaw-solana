//! Host-testable logic for the WASM component spike.

#![forbid(unsafe_code)]

use std::fmt;

use base64::{engine::general_purpose::STANDARD, Engine as _};
use curve25519_dalek::edwards::CompressedEdwardsY;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// Arguments accepted by the spike tool.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProbeRequest {
    pub public_key: String,
    pub payload: String,
}

/// Structured proof returned by the spike tool.
#[derive(Debug, Deserialize, PartialEq, Eq, Serialize)]
pub struct ProbeOutput {
    pub public_key: String,
    pub public_key_base64: String,
    pub payload_sha256_base64: String,
    pub is_off_curve: bool,
}

#[derive(Debug, PartialEq, Eq)]
pub enum ProbeError {
    InvalidJson(String),
    InvalidBase58(String),
    InvalidPublicKeyLength(usize),
    JsonSerialization(String),
}

impl fmt::Display for ProbeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidJson(error) => write!(formatter, "invalid JSON: {error}"),
            Self::InvalidBase58(error) => write!(formatter, "invalid public-key base58: {error}"),
            Self::InvalidPublicKeyLength(length) => {
                write!(
                    formatter,
                    "public key decoded to {length} bytes; expected 32"
                )
            }
            Self::JsonSerialization(error) => {
                write!(formatter, "could not serialize probe output: {error}")
            }
        }
    }
}

impl std::error::Error for ProbeError {}

/// Parse JSON, execute the dependency probe, and serialize the result.
pub fn probe_json(args: &str) -> Result<String, ProbeError> {
    let request: ProbeRequest =
        serde_json::from_str(args).map_err(|error| ProbeError::InvalidJson(error.to_string()))?;
    let output = run_probe(&request)?;
    serde_json::to_string(&output).map_err(|error| ProbeError::JsonSerialization(error.to_string()))
}

/// Exercise base58, SHA-256, curve decompression, base64, and serde without I/O.
pub fn run_probe(request: &ProbeRequest) -> Result<ProbeOutput, ProbeError> {
    let decoded = bs58::decode(&request.public_key)
        .into_vec()
        .map_err(|error| ProbeError::InvalidBase58(error.to_string()))?;
    let decoded_length = decoded.len();
    let public_key: [u8; 32] = decoded
        .try_into()
        .map_err(|_| ProbeError::InvalidPublicKeyLength(decoded_length))?;

    let digest: [u8; 32] = Sha256::digest(request.payload.as_bytes()).into();
    let is_off_curve = CompressedEdwardsY(public_key).decompress().is_none();

    Ok(ProbeOutput {
        public_key: bs58::encode(public_key).into_string(),
        public_key_base64: STANDARD.encode(public_key),
        payload_sha256_base64: STANDARD.encode(digest),
        is_off_curve,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const SYSTEM_PROGRAM: &str = "11111111111111111111111111111111";

    #[test]
    fn exercises_the_dependency_set_deterministically() {
        let request = ProbeRequest {
            public_key: SYSTEM_PROGRAM.to_string(),
            payload: "abc".to_string(),
        };

        let first = run_probe(&request).expect("valid probe");
        let second = run_probe(&request).expect("valid probe");

        assert_eq!(first, second);
        assert_eq!(first.public_key, SYSTEM_PROGRAM);
        assert_eq!(
            first.public_key_base64,
            "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA="
        );
        assert_eq!(
            first.payload_sha256_base64,
            "ungWv48Bz+pBQUDeXa4iI7ADYaOWF3qctBD/YfIAFa0="
        );
    }

    #[test]
    fn serializes_valid_json_output() {
        let args = format!(r#"{{"public_key":"{SYSTEM_PROGRAM}","payload":"m0"}}"#);
        let json = probe_json(&args).expect("probe output");
        let output: ProbeOutput = serde_json::from_str(&json).expect("output JSON");
        assert_eq!(output.public_key, SYSTEM_PROGRAM);
    }

    #[test]
    fn rejects_bad_base58_and_wrong_lengths() {
        let bad_base58 = ProbeRequest {
            public_key: "not!base58".to_string(),
            payload: String::new(),
        };
        assert!(matches!(
            run_probe(&bad_base58),
            Err(ProbeError::InvalidBase58(_))
        ));

        let short = ProbeRequest {
            public_key: "1".to_string(),
            payload: String::new(),
        };
        assert_eq!(
            run_probe(&short),
            Err(ProbeError::InvalidPublicKeyLength(1))
        );
    }
}
