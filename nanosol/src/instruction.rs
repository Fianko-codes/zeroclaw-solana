//! Instruction types and the exact subset required by the planned plugins.

use std::fmt;

use crate::pubkey::{
    derive_associated_token_address, PdaError, Pubkey, ASSOCIATED_TOKEN_PROGRAM_ID,
    COMPUTE_BUDGET_PROGRAM_ID, LEGACY_TOKEN_PROGRAM_ID, MEMO_V3_PROGRAM_ID,
    RECENT_BLOCKHASHES_SYSVAR_ID, SYSTEM_PROGRAM_ID, TOKEN_2022_PROGRAM_ID,
};

/// `SystemInstruction::AdvanceNonceAccount` bincode encoding: the enum
/// discriminant `4` as a little-endian `u32`, with no payload. Confirmed
/// byte-for-byte against `solana-system-interface`.
pub const ADVANCE_NONCE_ACCOUNT_DATA: [u8; 4] = [4, 0, 0, 0];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AccountMeta {
    pub pubkey: Pubkey,
    pub is_signer: bool,
    pub is_writable: bool,
}

impl AccountMeta {
    pub const fn writable(pubkey: Pubkey, is_signer: bool) -> Self {
        Self {
            pubkey,
            is_signer,
            is_writable: true,
        }
    }

    pub const fn readonly(pubkey: Pubkey, is_signer: bool) -> Self {
        Self {
            pubkey,
            is_signer,
            is_writable: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Instruction {
    pub program_id: Pubkey,
    pub accounts: Vec<AccountMeta>,
    pub data: Vec<u8>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TokenProgram {
    Legacy,
    Token2022,
}

impl TokenProgram {
    pub const fn id(self) -> Pubkey {
        match self {
            Self::Legacy => LEGACY_TOKEN_PROGRAM_ID,
            Self::Token2022 => TOKEN_2022_PROGRAM_ID,
        }
    }

    pub fn identify(owner: &Pubkey) -> Result<Self, InstructionError> {
        if owner == &LEGACY_TOKEN_PROGRAM_ID {
            Ok(Self::Legacy)
        } else if owner == &TOKEN_2022_PROGRAM_ID {
            Ok(Self::Token2022)
        } else {
            Err(InstructionError::UnsupportedTokenProgram(*owner))
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InstructionError {
    UnsupportedTokenProgram(Pubkey),
    Pda(PdaError),
}

impl fmt::Display for InstructionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedTokenProgram(owner) => {
                write!(formatter, "unsupported token program owner {owner}")
            }
            Self::Pda(error) => write!(
                formatter,
                "could not derive associated token account: {error}"
            ),
        }
    }
}

impl std::error::Error for InstructionError {}

impl From<PdaError> for InstructionError {
    fn from(error: PdaError) -> Self {
        Self::Pda(error)
    }
}

pub fn transfer_checked(
    source: Pubkey,
    mint: Pubkey,
    destination: Pubkey,
    authority: Pubkey,
    amount: u64,
    decimals: u8,
    token_program: TokenProgram,
) -> Instruction {
    let mut data = Vec::with_capacity(10);
    data.push(12);
    data.extend_from_slice(&amount.to_le_bytes());
    data.push(decimals);
    Instruction {
        program_id: token_program.id(),
        accounts: vec![
            AccountMeta::writable(source, false),
            AccountMeta::readonly(mint, false),
            AccountMeta::writable(destination, false),
            AccountMeta::readonly(authority, true),
        ],
        data,
    }
}

/// Build ATA `CreateIdempotent` and return both the instruction and derived ATA.
pub fn create_associated_token_account_idempotent(
    payer: Pubkey,
    owner: Pubkey,
    mint: Pubkey,
    token_program: TokenProgram,
) -> Result<(Instruction, Pubkey), InstructionError> {
    let token_program_id = token_program.id();
    let (ata, _) = derive_associated_token_address(&owner, &mint, &token_program_id)?;
    let instruction = Instruction {
        program_id: ASSOCIATED_TOKEN_PROGRAM_ID,
        accounts: vec![
            AccountMeta::writable(payer, true),
            AccountMeta::writable(ata, false),
            AccountMeta::readonly(owner, false),
            AccountMeta::readonly(mint, false),
            AccountMeta::readonly(SYSTEM_PROGRAM_ID, false),
            AccountMeta::readonly(token_program_id, false),
        ],
        data: vec![1],
    };
    Ok((instruction, ata))
}

pub fn memo(text: &str) -> Instruction {
    Instruction {
        program_id: MEMO_V3_PROGRAM_ID,
        accounts: Vec::new(),
        data: text.as_bytes().to_vec(),
    }
}

pub fn set_compute_unit_limit(units: u32) -> Instruction {
    let mut data = Vec::with_capacity(5);
    data.push(2);
    data.extend_from_slice(&units.to_le_bytes());
    Instruction {
        program_id: COMPUTE_BUDGET_PROGRAM_ID,
        accounts: Vec::new(),
        data,
    }
}

pub fn set_compute_unit_price(micro_lamports: u64) -> Instruction {
    let mut data = Vec::with_capacity(9);
    data.push(3);
    data.extend_from_slice(&micro_lamports.to_le_bytes());
    Instruction {
        program_id: COMPUTE_BUDGET_PROGRAM_ID,
        accounts: Vec::new(),
        data,
    }
}

/// Build the System Program `AdvanceNonceAccount` instruction. Account order and
/// privileges match `solana_system_interface::instruction::advance_nonce_account`:
/// `[0]` nonce account (writable, non-signer), `[1]` recent-blockhashes sysvar
/// (readonly, non-signer), `[2]` nonce authority (readonly signer at the
/// instruction level). This must be the first instruction of a durable-nonce
/// transaction, and the message blockhash must equal the stored durable nonce.
pub fn advance_nonce_account(nonce_account: Pubkey, nonce_authority: Pubkey) -> Instruction {
    Instruction {
        program_id: SYSTEM_PROGRAM_ID,
        accounts: vec![
            AccountMeta::writable(nonce_account, false),
            AccountMeta::readonly(RECENT_BLOCKHASHES_SYSVAR_ID, false),
            AccountMeta::readonly(nonce_authority, true),
        ],
        data: ADVANCE_NONCE_ACCOUNT_DATA.to_vec(),
    }
}
