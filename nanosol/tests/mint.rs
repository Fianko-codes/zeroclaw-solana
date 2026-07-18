use std::mem::size_of;

use nanosol::{
    instruction::TokenProgram,
    mint::{
        parse_mint_account, MintError, MintExtensionType, MINT_BASE_BYTES, TOKEN_2022_TLV_OFFSET,
        TOKEN_ACCOUNT_BASE_BYTES,
    },
    pubkey::{LEGACY_TOKEN_PROGRAM_ID, TOKEN_2022_PROGRAM_ID},
    rpc::RpcAccount,
};
use solana_program_pack::Pack;
use spl_token_2022_interface::{
    extension::{
        confidential_mint_burn::ConfidentialMintBurn,
        confidential_transfer::ConfidentialTransferMint,
        confidential_transfer_fee::ConfidentialTransferFeeConfig,
        default_account_state::DefaultAccountState, non_transferable::NonTransferable,
        pausable::PausableConfig, permanent_delegate::PermanentDelegate,
        transfer_fee::TransferFeeConfig, transfer_hook::TransferHook, AccountType, ExtensionType,
    },
    state::Mint as OfficialMint,
};

fn base_mint(decimals: u8) -> Vec<u8> {
    let mut data = vec![0; MINT_BASE_BYTES];
    data[44] = decimals;
    data[45] = 1;
    OfficialMint::unpack(&data).expect("official mint fixture");
    data
}

fn account(owner: nanosol::pubkey::Pubkey, data: Vec<u8>) -> RpcAccount {
    RpcAccount {
        owner,
        executable: false,
        data,
    }
}

fn token_2022_with_entries(entries: &[(u16, usize)]) -> Vec<u8> {
    let mut data = vec![0; TOKEN_2022_TLV_OFFSET];
    data[..MINT_BASE_BYTES].copy_from_slice(&base_mint(6));
    data[TOKEN_ACCOUNT_BASE_BYTES] = u8::from(AccountType::Mint);
    for (discriminant, length) in entries {
        data.extend_from_slice(&discriminant.to_le_bytes());
        data.extend_from_slice(
            &u16::try_from(*length)
                .expect("fixture length")
                .to_le_bytes(),
        );
        data.extend(std::iter::repeat(0).take(*length));
    }
    data
}

#[test]
fn legacy_and_extension_free_token_2022_mints_parse_strictly() {
    let legacy =
        parse_mint_account(&account(LEGACY_TOKEN_PROGRAM_ID, base_mint(6))).expect("legacy mint");
    assert_eq!(legacy.token_program, TokenProgram::Legacy);
    assert_eq!(legacy.decimals, 6);
    assert!(legacy.extensions.is_empty());

    let token_2022 = parse_mint_account(&account(TOKEN_2022_PROGRAM_ID, base_mint(9)))
        .expect("extension-free Token-2022 mint");
    assert_eq!(token_2022.token_program, TokenProgram::Token2022);
    assert_eq!(token_2022.decimals, 9);
    assert!(token_2022.extensions.is_empty());
}

#[test]
fn discriminants_and_denied_layout_sizes_match_the_official_interface() {
    let fixtures = [
        (
            ExtensionType::TransferFeeConfig,
            MintExtensionType::TransferFeeConfig,
            size_of::<TransferFeeConfig>(),
        ),
        (
            ExtensionType::ConfidentialTransferMint,
            MintExtensionType::ConfidentialTransferMint,
            size_of::<ConfidentialTransferMint>(),
        ),
        (
            ExtensionType::DefaultAccountState,
            MintExtensionType::DefaultAccountState,
            size_of::<DefaultAccountState>(),
        ),
        (
            ExtensionType::NonTransferable,
            MintExtensionType::NonTransferable,
            size_of::<NonTransferable>(),
        ),
        (
            ExtensionType::PermanentDelegate,
            MintExtensionType::PermanentDelegate,
            size_of::<PermanentDelegate>(),
        ),
        (
            ExtensionType::TransferHook,
            MintExtensionType::TransferHook,
            size_of::<TransferHook>(),
        ),
        (
            ExtensionType::ConfidentialTransferFeeConfig,
            MintExtensionType::ConfidentialTransferFeeConfig,
            size_of::<ConfidentialTransferFeeConfig>(),
        ),
        (
            ExtensionType::ConfidentialMintBurn,
            MintExtensionType::ConfidentialMintBurn,
            size_of::<ConfidentialMintBurn>(),
        ),
        (
            ExtensionType::Pausable,
            MintExtensionType::Pausable,
            size_of::<PausableConfig>(),
        ),
    ];

    for (official, expected, length) in fixtures {
        let discriminant = u16::from(official);
        assert_eq!(expected.discriminant(), discriminant);
        let data = token_2022_with_entries(&[(discriminant, length)]);
        let parsed = parse_mint_account(&account(TOKEN_2022_PROGRAM_ID, data))
            .expect("official extension fixture");
        assert_eq!(parsed.extensions.len(), 1);
        assert_eq!(parsed.extensions[0].extension_type, expected);
        assert_eq!(usize::from(parsed.extensions[0].data_length), length);
    }

    assert_eq!(MintExtensionType::Pausable.discriminant(), 26);
}

#[test]
fn malformed_wrong_type_duplicate_and_unknown_tlv_are_distinguished() {
    let mut malformed = token_2022_with_entries(&[(14, 64)]);
    malformed.pop();
    assert_eq!(
        parse_mint_account(&account(TOKEN_2022_PROGRAM_ID, malformed)),
        Err(MintError::MalformedTlv)
    );

    let duplicate = token_2022_with_entries(&[(14, 64), (14, 64)]);
    assert_eq!(
        parse_mint_account(&account(TOKEN_2022_PROGRAM_ID, duplicate)),
        Err(MintError::DuplicateExtension(14))
    );

    let account_only = token_2022_with_entries(&[(u16::from(ExtensionType::TransferFeeAmount), 8)]);
    assert_eq!(
        parse_mint_account(&account(TOKEN_2022_PROGRAM_ID, account_only)),
        Err(MintError::ExtensionNotForMint(2))
    );

    let unknown = token_2022_with_entries(&[(9_999, 3)]);
    let parsed = parse_mint_account(&account(TOKEN_2022_PROGRAM_ID, unknown))
        .expect("unknown TLV remains inspectable for policy refusal");
    assert_eq!(
        parsed.extensions[0].extension_type,
        MintExtensionType::Unknown(9_999)
    );

    let mut bad_padding = token_2022_with_entries(&[]);
    bad_padding[MINT_BASE_BYTES] = 1;
    assert_eq!(
        parse_mint_account(&account(TOKEN_2022_PROGRAM_ID, bad_padding)),
        Err(MintError::InvalidToken2022Padding)
    );

    let mut wrong_account_type = token_2022_with_entries(&[]);
    wrong_account_type[TOKEN_ACCOUNT_BASE_BYTES] = u8::from(AccountType::Account);
    assert_eq!(
        parse_mint_account(&account(TOKEN_2022_PROGRAM_ID, wrong_account_type)),
        Err(MintError::InvalidToken2022AccountType(2))
    );
}

#[test]
fn incorrect_owner_lengths_flags_and_initialization_fail_closed() {
    let mut uninitialized = base_mint(6);
    uninitialized[45] = 0;
    assert_eq!(
        parse_mint_account(&account(LEGACY_TOKEN_PROGRAM_ID, uninitialized)),
        Err(MintError::Uninitialized)
    );

    let mut invalid_flag = base_mint(6);
    invalid_flag[45] = 2;
    assert_eq!(
        parse_mint_account(&account(LEGACY_TOKEN_PROGRAM_ID, invalid_flag)),
        Err(MintError::InvalidInitializedFlag(2))
    );

    let mut bad_option = base_mint(6);
    bad_option[0] = 2;
    assert_eq!(
        parse_mint_account(&account(LEGACY_TOKEN_PROGRAM_ID, bad_option)),
        Err(MintError::InvalidAuthorityOption)
    );

    let truncated = vec![0; MINT_BASE_BYTES - 1];
    assert!(matches!(
        parse_mint_account(&account(LEGACY_TOKEN_PROGRAM_ID, truncated)),
        Err(MintError::InvalidLength(_))
    ));

    let token_account_sized = vec![0; TOKEN_ACCOUNT_BASE_BYTES];
    assert!(matches!(
        parse_mint_account(&account(TOKEN_2022_PROGRAM_ID, token_account_sized)),
        Err(MintError::InvalidLength(_))
    ));

    let executable = RpcAccount {
        owner: LEGACY_TOKEN_PROGRAM_ID,
        executable: true,
        data: base_mint(6),
    };
    assert_eq!(parse_mint_account(&executable), Err(MintError::Executable));

    assert!(matches!(
        parse_mint_account(&account(nanosol::pubkey::SYSTEM_PROGRAM_ID, base_mint(6))),
        Err(MintError::UnsupportedOwner(_))
    ));
}
