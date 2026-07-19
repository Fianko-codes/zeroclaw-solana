//! M4 Phase-A durable-nonce prototype.
//!
//! This crate is a *spike*: it prototypes the exact durable-nonce primitives
//! that Phase B ports into `nanosol`, and proves them byte-for-byte against the
//! official Solana crates (host-only dev-dependencies) before any production
//! code is touched. Nothing here signs, submits, or accepts a private key.
//!
//! Confirmed against `solana-nonce 3.2.0`, `solana-system-interface 3.2.0`,
//! `solana-sdk-ids 3.1.0` (see `tests/nonce_oracle.rs` and `M4_SPIKE_RESULTS.md`).

use nanosol::{
    instruction::{AccountMeta, Instruction},
    message::{Message, MessageVersion, Transaction},
    pubkey::{Pubkey, SYSTEM_PROGRAM_ID},
};

/// Canonical serialized length of an initialized nonce account
/// (`solana_nonce::state::State::size()` == 80).
pub const NONCE_ACCOUNT_LENGTH: usize = 80;

/// `SysvarRecentB1ockHashes11111111111111111111`.
/// Raw bytes confirmed against `solana_sdk_ids::sysvar::recent_blockhashes::ID`.
pub const RECENT_BLOCKHASHES_SYSVAR_ID: Pubkey = Pubkey::new([
    0x06, 0xa7, 0xd5, 0x17, 0x19, 0x2c, 0x56, 0x8e, 0xe0, 0x8a, 0x84, 0x5f, 0x73, 0xd2, 0x97, 0x88,
    0xcf, 0x03, 0x5c, 0x31, 0x45, 0xb2, 0x1a, 0xb3, 0x44, 0xd8, 0x06, 0x2e, 0xa9, 0x40, 0x00, 0x00,
]);

/// `SystemInstruction::AdvanceNonceAccount` bincode encoding: the enum
/// discriminant `4` as a little-endian `u32`, with no payload.
pub const ADVANCE_NONCE_ACCOUNT_DATA: [u8; 4] = [4, 0, 0, 0];

// bincode enum discriminants (little-endian u32).
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

/// Strict fixed-layout parser for the account data of an initialized,
/// current-version nonce account.
///
/// Deliberately stricter than `bincode::deserialize::<Versions>`: it requires
/// exactly 80 bytes (rejecting trailing bytes, which bincode tolerates), the
/// `Current` version wrapper, and the `Initialized` state. Legacy or future
/// version wrappers, the uninitialized state, and any other length fail closed.
pub fn parse_nonce_account_data(data: &[u8]) -> Result<NonceAccount, NonceError> {
    if data.len() != NONCE_ACCOUNT_LENGTH {
        return Err(NonceError::InvalidLength(data.len()));
    }
    let version = u32::from_le_bytes(data[0..4].try_into().expect("4 bytes"));
    if version != VERSION_CURRENT {
        return Err(NonceError::UnsupportedVersion(version));
    }
    let state = u32::from_le_bytes(data[4..8].try_into().expect("4 bytes"));
    match state {
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

/// Build the `AdvanceNonceAccount` instruction with the M4 authority
/// arrangement (`nonce_authority == sender`). Account order and privileges
/// match `solana_system_interface::instruction::advance_nonce_account`.
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecodedAdvanceNonce {
    pub nonce_account: Pubkey,
    pub nonce_authority: Pubkey,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DecodeError {
    OutOfBounds,
    UnexpectedProgram,
    InvalidData,
    InvalidAccountCount,
    InvalidPrivileges(usize),
    WrongSysvar,
}

/// Reconstruct an account meta from a compiled v0 message header. This mirrors
/// `nanosol::inspect`'s private `account_meta` privilege derivation exactly.
fn account_meta(message: &Message, index: u8) -> Option<AccountMeta> {
    let idx = usize::from(index);
    let pubkey = *message.account_keys.get(idx)?;
    let required = usize::from(message.header.num_required_signatures);
    let readonly_signed = usize::from(message.header.num_readonly_signed_accounts);
    let readonly_unsigned = usize::from(message.header.num_readonly_unsigned_accounts);
    let is_signer = idx < required;
    let is_writable = if is_signer {
        idx < required.saturating_sub(readonly_signed)
    } else {
        idx < message.account_keys.len().saturating_sub(readonly_unsigned)
    };
    Some(AccountMeta {
        pubkey,
        is_signer,
        is_writable,
    })
}

/// Semantic decode + privilege check of an `AdvanceNonceAccount` instruction at
/// a given index. This is the shape `verify_final_bytes` will require at index 0.
pub fn decode_advance_nonce_account(
    message: &Message,
    index: usize,
) -> Result<DecodedAdvanceNonce, DecodeError> {
    let instruction = message.instructions.get(index).ok_or(DecodeError::OutOfBounds)?;
    let program_id = message
        .account_keys
        .get(usize::from(instruction.program_id_index))
        .copied()
        .ok_or(DecodeError::OutOfBounds)?;
    if program_id != SYSTEM_PROGRAM_ID {
        return Err(DecodeError::UnexpectedProgram);
    }
    if instruction.data != ADVANCE_NONCE_ACCOUNT_DATA {
        return Err(DecodeError::InvalidData);
    }
    if instruction.account_indexes.len() != 3 {
        return Err(DecodeError::InvalidAccountCount);
    }
    let metas: Vec<AccountMeta> = instruction
        .account_indexes
        .iter()
        .map(|i| account_meta(message, *i).ok_or(DecodeError::OutOfBounds))
        .collect::<Result<_, _>>()?;
    // [0] nonce account: writable, non-signer.
    if metas[0].is_signer || !metas[0].is_writable {
        return Err(DecodeError::InvalidPrivileges(0));
    }
    // [1] recent-blockhashes sysvar: readonly, non-signer, exact address.
    if metas[1].is_signer || metas[1].is_writable {
        return Err(DecodeError::InvalidPrivileges(1));
    }
    if metas[1].pubkey != RECENT_BLOCKHASHES_SYSVAR_ID {
        return Err(DecodeError::WrongSysvar);
    }
    // [2] nonce authority: signer. When (as in M4) the authority is also the
    // fee payer, the compiled message marks this key writable+signer; the
    // runtime only requires the authority to be a signer, so we require exactly
    // that and leave the "authority == sender == fee payer" identity to the
    // verifier.
    if !metas[2].is_signer {
        return Err(DecodeError::InvalidPrivileges(2));
    }
    Ok(DecodedAdvanceNonce {
        nonce_account: metas[0].pubkey,
        nonce_authority: metas[2].pubkey,
    })
}

/// Assemble a full unsigned durable transfer transaction using the *frozen*
/// `nanosol` message machinery, with `AdvanceNonceAccount` as instruction zero
/// and the message blockhash set to the durable nonce value.
#[allow(clippy::too_many_arguments)]
pub fn build_durable_transfer(
    sender: Pubkey,
    nonce_account: Pubkey,
    durable_nonce: [u8; 32],
    extra_instructions: Vec<Instruction>,
) -> Result<Vec<u8>, nanosol::message::MessageError> {
    let mut instructions = vec![advance_nonce_account(nonce_account, sender)];
    instructions.extend(extra_instructions);
    let message = Message::compile(MessageVersion::V0, sender, durable_nonce, &instructions)?;
    Transaction::new_unsigned(message).serialize()
}
