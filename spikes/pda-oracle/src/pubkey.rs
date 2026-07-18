//! Independent Solana public-key parsing, PDA creation/search, and ATA derivation.

use std::{fmt, str::FromStr};

use curve25519_dalek::edwards::CompressedEdwardsY;
use sha2::{Digest, Sha256};

pub const PUBLIC_KEY_BYTES: usize = 32;
pub const MAX_SEEDS: usize = 16;
pub const MAX_SEED_LENGTH: usize = 32;
pub const PDA_MARKER: &[u8] = b"ProgramDerivedAddress";

pub const ASSOCIATED_TOKEN_PROGRAM_ID: &str = "ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL";
pub const LEGACY_TOKEN_PROGRAM_ID: &str = "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA";
pub const TOKEN_2022_PROGRAM_ID: &str = "TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb";

#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
pub struct PublicKey([u8; PUBLIC_KEY_BYTES]);

impl PublicKey {
    pub const fn from_bytes(bytes: [u8; PUBLIC_KEY_BYTES]) -> Self {
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

impl fmt::Display for PublicKey {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&bs58::encode(self.0).into_string())
    }
}

impl FromStr for PublicKey {
    type Err = ParsePublicKeyError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let decoded = bs58::decode(value)
            .into_vec()
            .map_err(|error| ParsePublicKeyError::InvalidBase58(error.to_string()))?;
        let decoded_length = decoded.len();
        let bytes = decoded
            .try_into()
            .map_err(|_| ParsePublicKeyError::InvalidLength(decoded_length))?;
        Ok(Self(bytes))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParsePublicKeyError {
    InvalidBase58(String),
    InvalidLength(usize),
}

impl fmt::Display for ParsePublicKeyError {
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

impl std::error::Error for ParsePublicKeyError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PdaError {
    TooManySeeds { supplied: usize, maximum: usize },
    SeedTooLong { index: usize, length: usize },
    OnCurve,
    NoViableBump,
    InvalidAssociatedTokenProgram(ParsePublicKeyError),
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
            Self::InvalidAssociatedTokenProgram(error) => {
                write!(
                    formatter,
                    "invalid associated-token program constant: {error}"
                )
            }
        }
    }
}

impl std::error::Error for PdaError {}

/// Create a PDA from explicitly supplied seeds.
pub fn create_program_address(
    seeds: &[&[u8]],
    program_id: &PublicKey,
) -> Result<PublicKey, PdaError> {
    validate_seeds(seeds, MAX_SEEDS)?;

    let mut hasher = Sha256::new();
    for seed in seeds {
        hasher.update(seed);
    }
    hasher.update(program_id.as_bytes());
    hasher.update(PDA_MARKER);
    let bytes: [u8; PUBLIC_KEY_BYTES] = hasher.finalize().into();
    let address = PublicKey::from_bytes(bytes);

    if address.is_on_curve() {
        Err(PdaError::OnCurve)
    } else {
        Ok(address)
    }
}

/// Search bump seeds from 255 down to 0, matching Solana's PDA convention.
pub fn find_program_address(
    seeds: &[&[u8]],
    program_id: &PublicKey,
) -> Result<(PublicKey, u8), PdaError> {
    // The bump occupies one of the 16 seed slots.
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

/// Derive the ATA for an owner, mint, and selected token program.
pub fn derive_associated_token_address(
    owner: &PublicKey,
    mint: &PublicKey,
    token_program: &PublicKey,
) -> Result<(PublicKey, u8), PdaError> {
    let associated_token_program = ASSOCIATED_TOKEN_PROGRAM_ID
        .parse()
        .map_err(PdaError::InvalidAssociatedTokenProgram)?;
    find_program_address(
        &[owner.as_bytes(), token_program.as_bytes(), mint.as_bytes()],
        &associated_token_program,
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
