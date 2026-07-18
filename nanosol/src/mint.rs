//! Strict SPL Mint parsing and minimal Token-2022 TLV inspection.

use std::{collections::BTreeSet, fmt};

use crate::{
    instruction::{InstructionError, TokenProgram},
    pubkey::Pubkey,
    rpc::RpcAccount,
};

pub const MINT_BASE_BYTES: usize = 82;
pub const TOKEN_ACCOUNT_BASE_BYTES: usize = 165;
pub const TOKEN_2022_TLV_OFFSET: usize = 166;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum MintExtensionType {
    TransferFeeConfig,
    TransferFeeAmount,
    MintCloseAuthority,
    ConfidentialTransferMint,
    ConfidentialTransferAccount,
    DefaultAccountState,
    ImmutableOwner,
    MemoTransfer,
    NonTransferable,
    InterestBearingConfig,
    CpiGuard,
    PermanentDelegate,
    NonTransferableAccount,
    TransferHook,
    TransferHookAccount,
    ConfidentialTransferFeeConfig,
    ConfidentialTransferFeeAmount,
    MetadataPointer,
    TokenMetadata,
    GroupPointer,
    TokenGroup,
    GroupMemberPointer,
    TokenGroupMember,
    ConfidentialMintBurn,
    ScaledUiAmount,
    Pausable,
    PausableAccount,
    PermissionedBurn,
    Unknown(u16),
}

impl MintExtensionType {
    pub const fn from_discriminant(value: u16) -> Self {
        match value {
            1 => Self::TransferFeeConfig,
            2 => Self::TransferFeeAmount,
            3 => Self::MintCloseAuthority,
            4 => Self::ConfidentialTransferMint,
            5 => Self::ConfidentialTransferAccount,
            6 => Self::DefaultAccountState,
            7 => Self::ImmutableOwner,
            8 => Self::MemoTransfer,
            9 => Self::NonTransferable,
            10 => Self::InterestBearingConfig,
            11 => Self::CpiGuard,
            12 => Self::PermanentDelegate,
            13 => Self::NonTransferableAccount,
            14 => Self::TransferHook,
            15 => Self::TransferHookAccount,
            16 => Self::ConfidentialTransferFeeConfig,
            17 => Self::ConfidentialTransferFeeAmount,
            18 => Self::MetadataPointer,
            19 => Self::TokenMetadata,
            20 => Self::GroupPointer,
            21 => Self::TokenGroup,
            22 => Self::GroupMemberPointer,
            23 => Self::TokenGroupMember,
            24 => Self::ConfidentialMintBurn,
            25 => Self::ScaledUiAmount,
            26 => Self::Pausable,
            27 => Self::PausableAccount,
            28 => Self::PermissionedBurn,
            other => Self::Unknown(other),
        }
    }

    pub const fn discriminant(self) -> u16 {
        match self {
            Self::TransferFeeConfig => 1,
            Self::TransferFeeAmount => 2,
            Self::MintCloseAuthority => 3,
            Self::ConfidentialTransferMint => 4,
            Self::ConfidentialTransferAccount => 5,
            Self::DefaultAccountState => 6,
            Self::ImmutableOwner => 7,
            Self::MemoTransfer => 8,
            Self::NonTransferable => 9,
            Self::InterestBearingConfig => 10,
            Self::CpiGuard => 11,
            Self::PermanentDelegate => 12,
            Self::NonTransferableAccount => 13,
            Self::TransferHook => 14,
            Self::TransferHookAccount => 15,
            Self::ConfidentialTransferFeeConfig => 16,
            Self::ConfidentialTransferFeeAmount => 17,
            Self::MetadataPointer => 18,
            Self::TokenMetadata => 19,
            Self::GroupPointer => 20,
            Self::TokenGroup => 21,
            Self::GroupMemberPointer => 22,
            Self::TokenGroupMember => 23,
            Self::ConfidentialMintBurn => 24,
            Self::ScaledUiAmount => 25,
            Self::Pausable => 26,
            Self::PausableAccount => 27,
            Self::PermissionedBurn => 28,
            Self::Unknown(value) => value,
        }
    }

    pub const fn is_mint_extension(self) -> bool {
        !matches!(
            self,
            Self::TransferFeeAmount
                | Self::ConfidentialTransferAccount
                | Self::ImmutableOwner
                | Self::MemoTransfer
                | Self::CpiGuard
                | Self::NonTransferableAccount
                | Self::TransferHookAccount
                | Self::ConfidentialTransferFeeAmount
                | Self::PausableAccount
        )
    }
}

impl fmt::Display for MintExtensionType {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self {
            Self::TransferFeeConfig => "TransferFeeConfig",
            Self::TransferFeeAmount => "TransferFeeAmount",
            Self::MintCloseAuthority => "MintCloseAuthority",
            Self::ConfidentialTransferMint => "ConfidentialTransferMint",
            Self::ConfidentialTransferAccount => "ConfidentialTransferAccount",
            Self::DefaultAccountState => "DefaultAccountState",
            Self::ImmutableOwner => "ImmutableOwner",
            Self::MemoTransfer => "MemoTransfer",
            Self::NonTransferable => "NonTransferable",
            Self::InterestBearingConfig => "InterestBearingConfig",
            Self::CpiGuard => "CpiGuard",
            Self::PermanentDelegate => "PermanentDelegate",
            Self::NonTransferableAccount => "NonTransferableAccount",
            Self::TransferHook => "TransferHook",
            Self::TransferHookAccount => "TransferHookAccount",
            Self::ConfidentialTransferFeeConfig => "ConfidentialTransferFeeConfig",
            Self::ConfidentialTransferFeeAmount => "ConfidentialTransferFeeAmount",
            Self::MetadataPointer => "MetadataPointer",
            Self::TokenMetadata => "TokenMetadata",
            Self::GroupPointer => "GroupPointer",
            Self::TokenGroup => "TokenGroup",
            Self::GroupMemberPointer => "GroupMemberPointer",
            Self::TokenGroupMember => "TokenGroupMember",
            Self::ConfidentialMintBurn => "ConfidentialMintBurn",
            Self::ScaledUiAmount => "ScaledUiAmount",
            Self::Pausable => "Pausable",
            Self::PausableAccount => "PausableAccount",
            Self::PermissionedBurn => "PermissionedBurn",
            Self::Unknown(value) => return write!(formatter, "unknown extension {value}"),
        };
        formatter.write_str(name)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MintExtension {
    pub extension_type: MintExtensionType,
    pub data_length: u16,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MintInfo {
    pub token_program: TokenProgram,
    pub decimals: u8,
    pub extensions: Vec<MintExtension>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MintError {
    UnsupportedOwner(Pubkey),
    Executable,
    InvalidLength(usize),
    InvalidAuthorityOption,
    Uninitialized,
    InvalidInitializedFlag(u8),
    InvalidToken2022Padding,
    InvalidToken2022AccountType(u8),
    MalformedTlv,
    DuplicateExtension(u16),
    ExtensionNotForMint(u16),
}

impl fmt::Display for MintError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedOwner(owner) => {
                write!(
                    formatter,
                    "mint owner {owner} is not an allowed token program"
                )
            }
            Self::Executable => formatter.write_str("mint account must not be executable"),
            Self::InvalidLength(length) => {
                write!(formatter, "mint account has invalid data length {length}")
            }
            Self::InvalidAuthorityOption => {
                formatter.write_str("mint account contains an invalid authority option tag")
            }
            Self::Uninitialized => formatter.write_str("mint account is not initialized"),
            Self::InvalidInitializedFlag(value) => {
                write!(
                    formatter,
                    "mint account has invalid initialized flag {value}"
                )
            }
            Self::InvalidToken2022Padding => {
                formatter.write_str("Token-2022 mint padding is malformed")
            }
            Self::InvalidToken2022AccountType(value) => {
                write!(formatter, "Token-2022 account type {value} is not Mint")
            }
            Self::MalformedTlv => formatter.write_str("Token-2022 extension TLV is malformed"),
            Self::DuplicateExtension(value) => {
                write!(
                    formatter,
                    "Token-2022 extension {value} appears more than once"
                )
            }
            Self::ExtensionNotForMint(value) => {
                write!(
                    formatter,
                    "Token-2022 extension {value} is not valid on a mint"
                )
            }
        }
    }
}

impl std::error::Error for MintError {}

pub fn parse_mint_account(account: &RpcAccount) -> Result<MintInfo, MintError> {
    if account.executable {
        return Err(MintError::Executable);
    }
    let token_program = TokenProgram::identify(&account.owner).map_err(|error| match error {
        InstructionError::UnsupportedTokenProgram(owner) => MintError::UnsupportedOwner(owner),
        InstructionError::Pda(_) => MintError::UnsupportedOwner(account.owner),
    })?;
    let data = &account.data;
    match token_program {
        TokenProgram::Legacy if data.len() != MINT_BASE_BYTES => {
            return Err(MintError::InvalidLength(data.len()));
        }
        TokenProgram::Token2022
            if data.len() != MINT_BASE_BYTES && data.len() < TOKEN_2022_TLV_OFFSET =>
        {
            return Err(MintError::InvalidLength(data.len()));
        }
        TokenProgram::Legacy | TokenProgram::Token2022 => {}
    }
    validate_option_tag(&data[0..4])?;
    validate_option_tag(&data[46..50])?;
    match data[45] {
        0 => return Err(MintError::Uninitialized),
        1 => {}
        value => return Err(MintError::InvalidInitializedFlag(value)),
    }
    let decimals = data[44];

    let extensions = match token_program {
        TokenProgram::Legacy => Vec::new(),
        TokenProgram::Token2022 => parse_token_2022_extensions(data)?,
    };

    Ok(MintInfo {
        token_program,
        decimals,
        extensions,
    })
}

fn validate_option_tag(tag: &[u8]) -> Result<(), MintError> {
    match tag {
        [0, 0, 0, 0] | [1, 0, 0, 0] => Ok(()),
        _ => Err(MintError::InvalidAuthorityOption),
    }
}

fn parse_token_2022_extensions(data: &[u8]) -> Result<Vec<MintExtension>, MintError> {
    if data.len() == MINT_BASE_BYTES {
        return Ok(Vec::new());
    }
    if data.len() < TOKEN_2022_TLV_OFFSET {
        return Err(MintError::InvalidLength(data.len()));
    }
    if data[MINT_BASE_BYTES..TOKEN_ACCOUNT_BASE_BYTES]
        .iter()
        .any(|byte| *byte != 0)
    {
        return Err(MintError::InvalidToken2022Padding);
    }
    if data[TOKEN_ACCOUNT_BASE_BYTES] != 1 {
        return Err(MintError::InvalidToken2022AccountType(
            data[TOKEN_ACCOUNT_BASE_BYTES],
        ));
    }

    let tlv = &data[TOKEN_2022_TLV_OFFSET..];
    let mut offset = 0_usize;
    let mut seen = BTreeSet::new();
    let mut extensions = Vec::new();
    while offset < tlv.len() {
        let remaining = &tlv[offset..];
        if remaining.len() < 2 {
            if remaining.iter().all(|byte| *byte == 0) {
                break;
            }
            return Err(MintError::MalformedTlv);
        }
        let discriminant = u16::from_le_bytes([remaining[0], remaining[1]]);
        if discriminant == 0 {
            if remaining.iter().any(|byte| *byte != 0) {
                return Err(MintError::MalformedTlv);
            }
            break;
        }
        if remaining.len() < 4 {
            return Err(MintError::MalformedTlv);
        }
        let data_length = u16::from_le_bytes([remaining[2], remaining[3]]);
        let entry_length = 4_usize
            .checked_add(usize::from(data_length))
            .ok_or(MintError::MalformedTlv)?;
        if entry_length > remaining.len() {
            return Err(MintError::MalformedTlv);
        }
        if !seen.insert(discriminant) {
            return Err(MintError::DuplicateExtension(discriminant));
        }
        let extension_type = MintExtensionType::from_discriminant(discriminant);
        if !extension_type.is_mint_extension() {
            return Err(MintError::ExtensionNotForMint(discriminant));
        }
        extensions.push(MintExtension {
            extension_type,
            data_length,
        });
        offset += entry_length;
    }
    Ok(extensions)
}
