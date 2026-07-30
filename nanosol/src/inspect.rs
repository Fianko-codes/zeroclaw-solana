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

/// SPL Token / Token-2022 `Transfer` instruction discriminant.
pub const TOKEN_TRANSFER_DISCRIMINANT: u8 = 3;
/// SPL Token / Token-2022 `TransferChecked` instruction discriminant.
pub const TOKEN_TRANSFER_CHECKED_DISCRIMINANT: u8 = 12;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TokenTransferKind {
    /// `Transfer`: carries no mint and no decimals, so a caller must obtain both
    /// from the token accounts or from chain state.
    Transfer,
    /// `TransferChecked`: names the mint and asserts its decimals.
    TransferChecked,
}

/// A decoded SPL Token / Token-2022 transfer, in either encoding a wallet may
/// have used to settle a Solana Pay request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecodedTokenTransfer {
    pub kind: TokenTransferKind,
    pub source: Pubkey,
    /// Present only for `TransferChecked`.
    pub mint: Option<Pubkey>,
    pub destination: Pubkey,
    /// The authority account with the privileges the message actually granted
    /// it. A single owner signs; a multisig authority is a non-signer whose
    /// participating signers follow in `extra_accounts`.
    pub authority: AccountMeta,
    pub amount: u64,
    /// Present only for `TransferChecked`.
    pub decimals: Option<u8>,
    pub token_program: TokenProgram,
    /// Accounts after the fixed prefix: Solana Pay reference keys and, for a
    /// multisig authority, the participating signers.
    pub extra_accounts: Vec<AccountMeta>,
}

impl DecodedTokenTransfer {
    /// True when `key` is attached to *this instruction* as a read-only
    /// non-signer, which is exactly how Solana Pay attaches a reference.
    ///
    /// A key that appears elsewhere in the transaction, or that appears here
    /// with any privilege, does not satisfy this test: a payment must be bound
    /// to the reference by the transfer instruction itself.
    pub fn carries_reference(&self, key: &Pubkey) -> bool {
        self.extra_accounts
            .iter()
            .any(|account| account.pubkey == *key && !account.is_signer && !account.is_writable)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InspectError {
    Message(MessageError),
    WrongMessageVersion,
    NonzeroSignature,
    MissingSignature,
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
            Self::MissingSignature => {
                formatter.write_str("settled transaction contains an all-zero signature slot")
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

/// Decode a settled transaction of either supported message version.
///
/// [`decode_unsigned_v0_transaction`] requires a v0 message with all-zero
/// signature slots, which a landed transaction never has: a wallet may submit a
/// legacy or a v0 message, and every required signature is filled in. Messages
/// carrying address-table lookups are still refused by the message decoder, so
/// every account index in the decoded message indexes the static key list.
pub fn decode_signed_transaction(bytes: &[u8]) -> Result<Transaction, InspectError> {
    let transaction = Transaction::deserialize(bytes)?;
    if transaction
        .signatures
        .iter()
        .any(|signature| signature.iter().all(|byte| *byte == 0))
    {
        return Err(InspectError::MissingSignature);
    }
    Ok(transaction)
}

/// Decode every SPL Token / Token-2022 transfer in a message.
///
/// Any instruction addressed to a token program whose discriminant is
/// `Transfer` or `TransferChecked` must decode strictly; a malformed one is an
/// error rather than a skipped instruction, so a hostile transaction cannot
/// hide a transfer behind a decoder that gives up. Instructions belonging to
/// other programs, and other token instructions, are ignored.
///
/// Inner (CPI) instructions are not visible in message bytes and are therefore
/// not decoded. Callers reconcile net effect through the transaction's token
/// balance deltas instead.
pub fn find_token_transfers(
    message: &Message,
) -> Result<Vec<(usize, DecodedTokenTransfer)>, InspectError> {
    let mut transfers = Vec::new();
    for (index, instruction) in message.instructions.iter().enumerate() {
        let program_id = message
            .account_keys
            .get(usize::from(instruction.program_id_index))
            .copied()
            .ok_or(InspectError::ProgramIndexOutOfBounds)?;
        if TokenProgram::identify(&program_id).is_err() {
            continue;
        }
        if matches!(
            instruction.data.first().copied(),
            Some(TOKEN_TRANSFER_DISCRIMINANT | TOKEN_TRANSFER_CHECKED_DISCRIMINANT)
        ) {
            transfers.push((index, decode_token_transfer(message, index)?));
        }
    }
    Ok(transfers)
}

/// Semantically decode one SPL Token / Token-2022 `Transfer` or
/// `TransferChecked` instruction, including any accounts appended after the
/// fixed prefix.
///
/// Unlike [`decode_transfer_checked`], which verifies bytes this workspace
/// built, this decoder accepts the shapes third-party wallets actually produce:
/// either discriminant, a multisig authority, and any number of trailing
/// reference accounts.
pub fn decode_token_transfer(
    message: &Message,
    instruction_index: usize,
) -> Result<DecodedTokenTransfer, InspectError> {
    let (program_id, accounts, data) = decode_instruction(message, instruction_index)?;
    let token_program =
        TokenProgram::identify(&program_id).map_err(|_| InspectError::UnsupportedTokenProgram)?;
    let (kind, fixed_accounts) = match (data.first().copied(), data.len()) {
        (Some(TOKEN_TRANSFER_DISCRIMINANT), 9) => (TokenTransferKind::Transfer, 3),
        (Some(TOKEN_TRANSFER_CHECKED_DISCRIMINANT), 10) => (TokenTransferKind::TransferChecked, 4),
        _ => return Err(InspectError::InvalidInstructionData),
    };
    if accounts.len() < fixed_accounts {
        return Err(InspectError::InvalidAccountCount);
    }

    // Token accounts and the mint are never signers; source and destination are
    // always written. The authority is left to the caller: a single owner signs,
    // while a multisig authority is a read-only account followed by its signers.
    require_privileges(&accounts, 0, false, true)?;
    let (mint, destination_index) = match kind {
        TokenTransferKind::Transfer => (None, 1),
        TokenTransferKind::TransferChecked => {
            require_privileges(&accounts, 1, false, false)?;
            (Some(accounts[1].pubkey), 2)
        }
    };
    require_privileges(&accounts, destination_index, false, true)?;
    let amount = u64::from_le_bytes(
        data[1..9]
            .try_into()
            .map_err(|_| InspectError::InvalidInstructionData)?,
    );
    Ok(DecodedTokenTransfer {
        kind,
        source: accounts[0].pubkey,
        mint,
        destination: accounts[destination_index].pubkey,
        authority: accounts[fixed_accounts - 1],
        amount,
        decimals: matches!(kind, TokenTransferKind::TransferChecked).then(|| data[9]),
        token_program,
        extra_accounts: accounts[fixed_accounts..].to_vec(),
    })
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
