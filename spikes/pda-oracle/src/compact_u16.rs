//! Independent compact-u16 codec used by Solana message lengths.

use std::fmt;

const MAX_ENCODING_BYTES: usize = 3;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompactU16Error {
    Truncated,
    TooLong,
    Overflow,
    NonCanonical,
}

impl fmt::Display for CompactU16Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = match self {
            Self::Truncated => "compact-u16 ended before a terminal byte",
            Self::TooLong => "compact-u16 exceeds three bytes",
            Self::Overflow => "compact-u16 exceeds u16::MAX",
            Self::NonCanonical => "compact-u16 uses a non-canonical alias encoding",
        };
        formatter.write_str(message)
    }
}

impl std::error::Error for CompactU16Error {}

/// Encode a `u16` in Solana's one-to-three-byte compact representation.
pub fn encode(value: u16) -> Vec<u8> {
    let mut remaining = value;
    let mut encoded = Vec::with_capacity(MAX_ENCODING_BYTES);
    loop {
        let mut byte = (remaining & 0x7f) as u8;
        remaining >>= 7;
        if remaining != 0 {
            byte |= 0x80;
        }
        encoded.push(byte);
        if remaining == 0 {
            return encoded;
        }
    }
}

/// Decode one compact `u16`, returning the value and bytes consumed.
///
/// Bytes after the terminal byte belong to the caller and are not consumed.
pub fn decode(bytes: &[u8]) -> Result<(u16, usize), CompactU16Error> {
    let mut value = 0_u32;

    for index in 0..MAX_ENCODING_BYTES {
        let byte = *bytes.get(index).ok_or(CompactU16Error::Truncated)?;
        if index > 0 && byte == 0 {
            return Err(CompactU16Error::NonCanonical);
        }

        let continues = byte & 0x80 != 0;
        if index == MAX_ENCODING_BYTES - 1 && continues {
            return Err(CompactU16Error::TooLong);
        }

        let chunk = u32::from(byte & 0x7f) << (index * 7);
        value |= chunk;
        if value > u32::from(u16::MAX) {
            return Err(CompactU16Error::Overflow);
        }

        if !continues {
            return Ok((value as u16, index + 1));
        }
    }

    Err(CompactU16Error::TooLong)
}
