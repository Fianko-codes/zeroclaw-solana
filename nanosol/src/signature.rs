//! Transaction signatures: strict base58 parsing and canonical formatting.
//!
//! Signatures are 64 bytes and therefore cannot reuse [`crate::pubkey::Pubkey`].
//! Parsing is total: any input that is not exactly 64 base58-decoded bytes is a
//! typed error, so a signature echoed back to a caller is always canonical.

use std::{fmt, str::FromStr};

use crate::message::SIGNATURE_BYTES;

/// The longest base58 encoding of 64 bytes. Used to bound untrusted input
/// before the decoder allocates.
pub const MAX_SIGNATURE_CHARS: usize = 88;

#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub struct Signature([u8; SIGNATURE_BYTES]);

impl Signature {
    pub const fn new(bytes: [u8; SIGNATURE_BYTES]) -> Self {
        Self(bytes)
    }

    pub const fn to_bytes(self) -> [u8; SIGNATURE_BYTES] {
        self.0
    }

    pub const fn as_bytes(&self) -> &[u8; SIGNATURE_BYTES] {
        &self.0
    }

    /// True when every byte is zero, i.e. an unsigned placeholder slot.
    pub fn is_zero(&self) -> bool {
        self.0.iter().all(|byte| *byte == 0)
    }
}

impl fmt::Display for Signature {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&bs58::encode(self.0).into_string())
    }
}

impl FromStr for Signature {
    type Err = ParseSignatureError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        if value.len() > MAX_SIGNATURE_CHARS {
            return Err(ParseSignatureError::InvalidLength(value.len()));
        }
        let decoded = bs58::decode(value)
            .into_vec()
            .map_err(|error| ParseSignatureError::InvalidBase58(error.to_string()))?;
        let length = decoded.len();
        decoded
            .try_into()
            .map(Self)
            .map_err(|_| ParseSignatureError::InvalidLength(length))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParseSignatureError {
    InvalidBase58(String),
    InvalidLength(usize),
}

impl fmt::Display for ParseSignatureError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidBase58(error) => write!(formatter, "invalid base58 signature: {error}"),
            Self::InvalidLength(length) => write!(
                formatter,
                "signature decoded to {length} bytes; expected {SIGNATURE_BYTES}"
            ),
        }
    }
}

impl std::error::Error for ParseSignatureError {}
