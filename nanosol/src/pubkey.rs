//! Solana public keys, program-derived addresses, and associated token accounts.

use std::{fmt, str::FromStr};

use curve25519_dalek::edwards::CompressedEdwardsY;
use sha2::{Digest, Sha256};

pub const PUBLIC_KEY_BYTES: usize = 32;
pub const MAX_SEEDS: usize = 16;
pub const MAX_SEED_LENGTH: usize = 32;
const PDA_MARKER: &[u8] = b"ProgramDerivedAddress";

#[derive(Clone, Copy, Debug, Default, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub struct Pubkey([u8; PUBLIC_KEY_BYTES]);

impl Pubkey {
    pub const fn new(bytes: [u8; PUBLIC_KEY_BYTES]) -> Self {
        Self(bytes)
    }

    pub const fn to_bytes(self) -> [u8; PUBLIC_KEY_BYTES] {
        self.0
    }

    pub const fn as_bytes(&self) -> &[u8; PUBLIC_KEY_BYTES] {
        &self.0
    }

    pub fn is_on_curve(&self) -> bool {
        CompressedEdwardsY(self.0).decompress().is_some()
    }
}

impl From<[u8; PUBLIC_KEY_BYTES]> for Pubkey {
    fn from(bytes: [u8; PUBLIC_KEY_BYTES]) -> Self {
        Self::new(bytes)
    }
}

impl fmt::Display for Pubkey {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&bs58::encode(self.0).into_string())
    }
}

impl FromStr for Pubkey {
    type Err = ParsePubkeyError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let decoded = bs58::decode(value)
            .into_vec()
            .map_err(|error| ParsePubkeyError::InvalidBase58(error.to_string()))?;
        let length = decoded.len();
        decoded
            .try_into()
            .map(Self)
            .map_err(|_| ParsePubkeyError::InvalidLength(length))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParsePubkeyError {
    InvalidBase58(String),
    InvalidLength(usize),
}

impl fmt::Display for ParsePubkeyError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidBase58(error) => write!(formatter, "invalid base58 public key: {error}"),
            Self::InvalidLength(length) => {
                write!(
                    formatter,
                    "public key decoded to {length} bytes; expected 32"
                )
            }
        }
    }
}

impl std::error::Error for ParsePubkeyError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PdaError {
    TooManySeeds { supplied: usize, maximum: usize },
    SeedTooLong { index: usize, length: usize },
    OnCurve,
    NoViableBump,
}

impl fmt::Display for PdaError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooManySeeds { supplied, maximum } => {
                write!(formatter, "received {supplied} seeds; maximum is {maximum}")
            }
            Self::SeedTooLong { index, length } => write!(
                formatter,
                "seed {index} is {length} bytes; maximum is {MAX_SEED_LENGTH}"
            ),
            Self::OnCurve => formatter.write_str("derived address lies on the Ed25519 curve"),
            Self::NoViableBump => formatter.write_str("no off-curve bump seed was found"),
        }
    }
}

impl std::error::Error for PdaError {}

pub fn create_program_address(seeds: &[&[u8]], program_id: &Pubkey) -> Result<Pubkey, PdaError> {
    validate_seeds(seeds, MAX_SEEDS)?;
    let mut hasher = Sha256::new();
    for seed in seeds {
        hasher.update(seed);
    }
    hasher.update(program_id.as_bytes());
    hasher.update(PDA_MARKER);
    let address = Pubkey::new(hasher.finalize().into());
    if address.is_on_curve() {
        Err(PdaError::OnCurve)
    } else {
        Ok(address)
    }
}

pub fn find_program_address(
    seeds: &[&[u8]],
    program_id: &Pubkey,
) -> Result<(Pubkey, u8), PdaError> {
    validate_seeds(seeds, MAX_SEEDS - 1)?;
    for bump in (0..=u8::MAX).rev() {
        let bump_seed = [bump];
        let mut with_bump = Vec::with_capacity(seeds.len() + 1);
        with_bump.extend_from_slice(seeds);
        with_bump.push(&bump_seed);
        match create_program_address(&with_bump, program_id) {
            Ok(address) => return Ok((address, bump)),
            Err(PdaError::OnCurve) => {}
            Err(error) => return Err(error),
        }
    }
    Err(PdaError::NoViableBump)
}

pub fn derive_associated_token_address(
    owner: &Pubkey,
    mint: &Pubkey,
    token_program: &Pubkey,
) -> Result<(Pubkey, u8), PdaError> {
    find_program_address(
        &[owner.as_bytes(), token_program.as_bytes(), mint.as_bytes()],
        &ASSOCIATED_TOKEN_PROGRAM_ID,
    )
}

fn validate_seeds(seeds: &[&[u8]], maximum: usize) -> Result<(), PdaError> {
    if seeds.len() > maximum {
        return Err(PdaError::TooManySeeds {
            supplied: seeds.len(),
            maximum,
        });
    }
    for (index, seed) in seeds.iter().enumerate() {
        if seed.len() > MAX_SEED_LENGTH {
            return Err(PdaError::SeedTooLong {
                index,
                length: seed.len(),
            });
        }
    }
    Ok(())
}

pub const SYSTEM_PROGRAM_ID: Pubkey = Pubkey::new([0; 32]);
pub const LEGACY_TOKEN_PROGRAM_ID: Pubkey = Pubkey::new([
    6, 221, 246, 225, 215, 101, 161, 147, 217, 203, 225, 70, 206, 235, 121, 172, 28, 180, 133, 237,
    95, 91, 55, 145, 58, 140, 245, 133, 126, 255, 0, 169,
]);
pub const TOKEN_2022_PROGRAM_ID: Pubkey = Pubkey::new([
    6, 221, 246, 225, 238, 117, 143, 222, 24, 66, 93, 188, 228, 108, 205, 218, 182, 26, 252, 77,
    131, 185, 13, 39, 254, 189, 249, 40, 216, 161, 139, 252,
]);
pub const ASSOCIATED_TOKEN_PROGRAM_ID: Pubkey = Pubkey::new([
    140, 151, 37, 143, 78, 36, 137, 241, 187, 61, 16, 41, 20, 142, 13, 131, 11, 90, 19, 153, 218,
    255, 16, 132, 4, 142, 123, 216, 219, 233, 248, 89,
]);
pub const MEMO_V3_PROGRAM_ID: Pubkey = Pubkey::new([
    5, 74, 83, 90, 153, 41, 33, 6, 77, 36, 232, 113, 96, 218, 56, 124, 124, 53, 181, 221, 188, 146,
    187, 129, 228, 31, 168, 64, 65, 5, 68, 141,
]);
pub const COMPUTE_BUDGET_PROGRAM_ID: Pubkey = Pubkey::new([
    3, 6, 70, 111, 229, 33, 23, 50, 255, 236, 173, 186, 114, 195, 155, 231, 188, 140, 229, 187,
    197, 247, 18, 107, 44, 67, 155, 58, 64, 0, 0, 0,
]);
/// `SysvarRecentB1ockHashes11111111111111111111`. Deprecated for on-chain reads
/// but still a required account of the `AdvanceNonceAccount` instruction.
pub const RECENT_BLOCKHASHES_SYSVAR_ID: Pubkey = Pubkey::new([
    6, 167, 213, 23, 25, 44, 86, 142, 224, 138, 132, 95, 115, 210, 151, 136, 207, 3, 92, 49, 69,
    178, 26, 179, 68, 216, 6, 46, 169, 64, 0, 0,
]);
