//! Solana's canonical one-to-three-byte compact-u16 codec.

use std::fmt;

const MAX_ENCODING_BYTES: usize = 3;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompactU16Error {
    ValueTooLarge(usize),
    Truncated,
    TooLong,
    Overflow,
    NonCanonical,
}

impl fmt::Display for CompactU16Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ValueTooLarge(value) => {
                write!(formatter, "{value} does not fit in compact-u16")
            }
            Self::Truncated => formatter.write_str("compact-u16 ended before a terminal byte"),
            Self::TooLong => formatter.write_str("compact-u16 exceeds three bytes"),
            Self::Overflow => formatter.write_str("compact-u16 exceeds u16::MAX"),
            Self::NonCanonical => {
                formatter.write_str("compact-u16 uses a non-canonical alias encoding")
            }
        }
    }
}

impl std::error::Error for CompactU16Error {}

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

pub fn encode_len(value: usize) -> Result<Vec<u8>, CompactU16Error> {
    let value = u16::try_from(value).map_err(|_| CompactU16Error::ValueTooLarge(value))?;
    Ok(encode(value))
}

/// Decode a value and return both the value and number of consumed bytes.
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
        value |= u32::from(byte & 0x7f) << (index * 7);
        if value > u32::from(u16::MAX) {
            return Err(CompactU16Error::Overflow);
        }
        if !continues {
            return Ok((value as u16, index + 1));
        }
    }
    Err(CompactU16Error::TooLong)
}
