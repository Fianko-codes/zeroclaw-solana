//! Strict parsing of System Program durable-nonce account data.
//!
//! The parser is a fixed-layout reader for an **initialized, current-version**
//! nonce account, deliberately stricter than `bincode::deserialize::<Versions>`:
//! it requires exactly 80 bytes (bincode tolerates trailing bytes; we do not),
//! the `Current` version wrapper, and the `Initialized` state. Legacy or future
//! version wrappers, the uninitialized state, unknown state discriminants, and
//! any other length fail closed. Confirmed byte-for-byte against `solana-nonce`
//! 3.2.0 and a real devnet-created nonce account (see `spikes/durable-nonce`).

use std::fmt;

use crate::pubkey::Pubkey;

/// Canonical serialized length of an initialized nonce account
/// (`solana_nonce::state::State::size()` == 80).
pub const NONCE_ACCOUNT_LENGTH: usize = 80;

// bincode enum discriminants, encoded as little-endian u32.
const VERSION_CURRENT: u32 = 1; // Versions::Current
const STATE_UNINITIALIZED: u32 = 0; // State::Uninitialized
const STATE_INITIALIZED: u32 = 1; // State::Initialized

/// A strictly-parsed, initialized, current-version nonce account.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NonceAccount {
    pub authority: Pubkey,
    pub durable_nonce: [u8; 32],
    pub lamports_per_signature: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NonceError {
    InvalidLength(usize),
    UnsupportedVersion(u32),
    Uninitialized,
    UnknownState(u32),
}

impl fmt::Display for NonceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidLength(length) => write!(
                formatter,
                "nonce account data is {length} bytes; expected {NONCE_ACCOUNT_LENGTH}"
            ),
            Self::UnsupportedVersion(version) => write!(
                formatter,
                "nonce account uses unsupported version wrapper {version}; only current (1) is supported"
            ),
            Self::Uninitialized => formatter.write_str("nonce account is not initialized"),
            Self::UnknownState(state) => {
                write!(formatter, "nonce account has unknown state discriminant {state}")
            }
        }
    }
}

impl std::error::Error for NonceError {}

/// Parse the raw account data of an initialized, current-version nonce account.
pub fn parse_nonce_account_data(data: &[u8]) -> Result<NonceAccount, NonceError> {
    if data.len() != NONCE_ACCOUNT_LENGTH {
        return Err(NonceError::InvalidLength(data.len()));
    }
    let version = read_u32(&data[0..4]);
    if version != VERSION_CURRENT {
        return Err(NonceError::UnsupportedVersion(version));
    }
    match read_u32(&data[4..8]) {
        STATE_UNINITIALIZED => return Err(NonceError::Uninitialized),
        STATE_INITIALIZED => {}
        other => return Err(NonceError::UnknownState(other)),
    }
    let mut authority = [0u8; 32];
    authority.copy_from_slice(&data[8..40]);
    let mut durable_nonce = [0u8; 32];
    durable_nonce.copy_from_slice(&data[40..72]);
    let lamports_per_signature = u64::from_le_bytes(data[72..80].try_into().expect("8 bytes"));
    Ok(NonceAccount {
        authority: Pubkey::new(authority),
        durable_nonce,
        lamports_per_signature,
    })
}

fn read_u32(bytes: &[u8]) -> u32 {
    u32::from_le_bytes(bytes.try_into().expect("4 bytes"))
}
