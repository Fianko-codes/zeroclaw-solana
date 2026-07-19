//! Semantic decoding for the exact instruction subset built by M3.

use std::{fmt, str};

use crate::{
    instruction::{AccountMeta, TokenProgram, ADVANCE_NONCE_ACCOUNT_DATA},
    message::{Message, MessageError, MessageVersion, Transaction},
    pubkey::{
        Pubkey, ASSOCIATED_TOKEN_PROGRAM_ID, MEMO_V3_PROGRAM_ID, RECENT_BLOCKHASHES_SYSVAR_ID,
        SYSTEM_PROGRAM_ID,
    },
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecodedAtaCreateIdempotent {
    pub payer: Pubkey,
    pub ata: Pubkey,
    pub owner: Pubkey,
    pub mint: Pubkey,
    pub token_program: TokenProgram,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecodedAdvanceNonce {
    pub nonce_account: Pubkey,
    pub nonce_authority: Pubkey,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecodedTransferChecked {
    pub source: Pubkey,
    pub mint: Pubkey,
    pub destination: Pubkey,
    pub authority: Pubkey,
    pub amount: u64,
    pub decimals: u8,
    pub token_program: TokenProgram,
    pub reference: Option<Pubkey>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InspectError {
    Message(MessageError),
    WrongMessageVersion,
    NonzeroSignature,
    InstructionOutOfBounds(usize),
    ProgramIndexOutOfBounds,
    ProgramAccountPrivileges,
    UnexpectedProgram,
    InvalidAccountCount,
    InvalidAccountPrivileges(usize),
    InvalidAccountAddress(usize),
    InvalidInstructionData,
    UnsupportedTokenProgram,
    InvalidMemoUtf8,
}

impl fmt::Display for InspectError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Message(error) => error.fmt(formatter),
            Self::WrongMessageVersion => {
                formatter.write_str("transaction message is not version 0")
            }
            Self::NonzeroSignature => {
                formatter.write_str("transaction contains a nonzero signature")
            }
            Self::InstructionOutOfBounds(index) => {
                write!(formatter, "transaction has no instruction at index {index}")
            }
            Self::ProgramIndexOutOfBounds => {
                formatter.write_str("instruction program index is out of bounds")
            }
            Self::ProgramAccountPrivileges => {
                formatter.write_str("instruction program account has invalid privileges")
            }
            Self::UnexpectedProgram => {
                formatter.write_str("instruction uses an unexpected program")
            }
            Self::InvalidAccountCount => {
                formatter.write_str("instruction has an invalid account count")
            }
            Self::InvalidAccountPrivileges(index) => {
                write!(
                    formatter,
                    "instruction account {index} has invalid privileges"
                )
            }
            Self::InvalidAccountAddress(index) => {
                write!(
                    formatter,
                    "instruction account {index} has an invalid address"
                )
            }
            Self::InvalidInstructionData => formatter.write_str("instruction data is malformed"),
            Self::UnsupportedTokenProgram => {
                formatter.write_str("transfer instruction uses an unsupported token program")
            }
            Self::InvalidMemoUtf8 => formatter.write_str("memo instruction is not valid UTF-8"),
        }
    }
}

impl std::error::Error for InspectError {}

impl From<MessageError> for InspectError {
    fn from(error: MessageError) -> Self {
        Self::Message(error)
    }
}

pub fn decode_unsigned_v0_transaction(bytes: &[u8]) -> Result<Transaction, InspectError> {
    let transaction = Transaction::deserialize(bytes)?;
    if transaction.message.version != MessageVersion::V0 {
        return Err(InspectError::WrongMessageVersion);
    }
    if !transaction.is_unsigned() {
        return Err(InspectError::NonzeroSignature);
    }
    Ok(transaction)
}

/// Semantically decode a System Program `AdvanceNonceAccount` instruction and
/// verify its account count, order, and privileges. Used to require durable-nonce
/// instruction zero in `verify_final_bytes`.
///
/// The nonce authority is checked only for the signer privilege: when (as in M4)
/// the authority is also the fee payer, the compiled message marks that key
/// writable+signer, and the runtime only requires it to be a signer. The
/// `authority == sender` identity is enforced by the caller.
pub fn decode_advance_nonce_account(
    message: &Message,
    instruction_index: usize,
) -> Result<DecodedAdvanceNonce, InspectError> {
    let (program_id, accounts, data) = decode_instruction(message, instruction_index)?;
    if program_id != SYSTEM_PROGRAM_ID {
        return Err(InspectError::UnexpectedProgram);
    }
    if data != ADVANCE_NONCE_ACCOUNT_DATA {
        return Err(InspectError::InvalidInstructionData);
    }
    if accounts.len() != 3 {
        return Err(InspectError::InvalidAccountCount);
    }
    require_privileges(&accounts, 0, false, true)?; // nonce account: writable, non-signer
    require_privileges(&accounts, 1, false, false)?; // recent-blockhashes sysvar: readonly, non-signer
    if accounts[1].pubkey != RECENT_BLOCKHASHES_SYSVAR_ID {
        return Err(InspectError::InvalidAccountAddress(1));
    }
    if !accounts[2].is_signer {
        return Err(InspectError::InvalidAccountPrivileges(2)); // nonce authority: signer
    }
    Ok(DecodedAdvanceNonce {
        nonce_account: accounts[0].pubkey,
        nonce_authority: accounts[2].pubkey,
    })
}

pub fn decode_ata_create_idempotent(
    message: &Message,
    instruction_index: usize,
) -> Result<DecodedAtaCreateIdempotent, InspectError> {
    let (program_id, accounts, data) = decode_instruction(message, instruction_index)?;
    if program_id != ASSOCIATED_TOKEN_PROGRAM_ID {
        return Err(InspectError::UnexpectedProgram);
    }
    if data != [1] {
        return Err(InspectError::InvalidInstructionData);
    }
    if accounts.len() != 6 {
        return Err(InspectError::InvalidAccountCount);
    }
    require_privileges(&accounts, 0, true, true)?;
    require_privileges(&accounts, 1, false, true)?;
    require_privileges(&accounts, 2, false, false)?;
    require_privileges(&accounts, 3, false, false)?;
    require_privileges(&accounts, 4, false, false)?;
    require_privileges(&accounts, 5, false, false)?;
    if accounts[4].pubkey != SYSTEM_PROGRAM_ID {
        return Err(InspectError::InvalidAccountAddress(4));
    }
    let token_program = TokenProgram::identify(&accounts[5].pubkey)
        .map_err(|_| InspectError::UnsupportedTokenProgram)?;
    Ok(DecodedAtaCreateIdempotent {
        payer: accounts[0].pubkey,
        ata: accounts[1].pubkey,
        owner: accounts[2].pubkey,
        mint: accounts[3].pubkey,
        token_program,
    })
}

pub fn decode_transfer_checked(
    message: &Message,
    instruction_index: usize,
) -> Result<DecodedTransferChecked, InspectError> {
    let (program_id, accounts, data) = decode_instruction(message, instruction_index)?;
    let token_program =
        TokenProgram::identify(&program_id).map_err(|_| InspectError::UnsupportedTokenProgram)?;
    if !matches!(accounts.len(), 4 | 5) {
        return Err(InspectError::InvalidAccountCount);
    }
    if data.len() != 10 || data[0] != 12 {
        return Err(InspectError::InvalidInstructionData);
    }
    require_privileges(&accounts, 0, false, true)?;
    require_privileges(&accounts, 1, false, false)?;
    require_privileges(&accounts, 2, false, true)?;
    if !accounts[3].is_signer {
        return Err(InspectError::InvalidAccountPrivileges(3));
    }
    let reference = if accounts.len() == 5 {
        require_privileges(&accounts, 4, false, false)?;
        Some(accounts[4].pubkey)
    } else {
        None
    };
    let amount = u64::from_le_bytes(
        data[1..9]
            .try_into()
            .map_err(|_| InspectError::InvalidInstructionData)?,
    );
    Ok(DecodedTransferChecked {
        source: accounts[0].pubkey,
        mint: accounts[1].pubkey,
        destination: accounts[2].pubkey,
        authority: accounts[3].pubkey,
        amount,
        decimals: data[9],
        token_program,
        reference,
    })
}

pub fn decode_memo(message: &Message, instruction_index: usize) -> Result<String, InspectError> {
    let (program_id, accounts, data) = decode_instruction(message, instruction_index)?;
    if program_id != MEMO_V3_PROGRAM_ID {
        return Err(InspectError::UnexpectedProgram);
    }
    if !accounts.is_empty() {
        return Err(InspectError::InvalidAccountCount);
    }
    str::from_utf8(data)
        .map(str::to_owned)
        .map_err(|_| InspectError::InvalidMemoUtf8)
}

fn decode_instruction(
    message: &Message,
    instruction_index: usize,
) -> Result<(Pubkey, Vec<AccountMeta>, &[u8]), InspectError> {
    let instruction = message
        .instructions
        .get(instruction_index)
        .ok_or(InspectError::InstructionOutOfBounds(instruction_index))?;
    let program_id = message
        .account_keys
        .get(usize::from(instruction.program_id_index))
        .copied()
        .ok_or(InspectError::ProgramIndexOutOfBounds)?;
    let program_meta = account_meta(message, instruction.program_id_index)?;
    if program_meta.is_signer || program_meta.is_writable {
        return Err(InspectError::ProgramAccountPrivileges);
    }
    let accounts = instruction
        .account_indexes
        .iter()
        .map(|index| account_meta(message, *index))
        .collect::<Result<Vec<_>, _>>()?;
    Ok((program_id, accounts, &instruction.data))
}

fn account_meta(message: &Message, index: u8) -> Result<AccountMeta, InspectError> {
    let index = usize::from(index);
    let pubkey = message
        .account_keys
        .get(index)
        .copied()
        .ok_or(InspectError::ProgramIndexOutOfBounds)?;
    let required = usize::from(message.header.num_required_signatures);
    let readonly_signed = usize::from(message.header.num_readonly_signed_accounts);
    let readonly_unsigned = usize::from(message.header.num_readonly_unsigned_accounts);
    let is_signer = index < required;
    let is_writable = if is_signer {
        index < required.saturating_sub(readonly_signed)
    } else {
        index < message.account_keys.len().saturating_sub(readonly_unsigned)
    };
    Ok(AccountMeta {
        pubkey,
        is_signer,
        is_writable,
    })
}

fn require_privileges(
    accounts: &[AccountMeta],
    index: usize,
    is_signer: bool,
    is_writable: bool,
) -> Result<(), InspectError> {
    let account = accounts
        .get(index)
        .ok_or(InspectError::InvalidAccountCount)?;
    if account.is_signer == is_signer && account.is_writable == is_writable {
        Ok(())
    } else {
        Err(InspectError::InvalidAccountPrivileges(index))
    }
}
