//! Byte-exact oracles for the durable-nonce prototype against official crates.

use durable_nonce_spike::{
    advance_nonce_account, parse_nonce_account_data, NonceAccount, NonceError,
    ADVANCE_NONCE_ACCOUNT_DATA, NONCE_ACCOUNT_LENGTH, RECENT_BLOCKHASHES_SYSVAR_ID,
};
use nanosol::pubkey::{Pubkey, SYSTEM_PROGRAM_ID};

use solana_hash::Hash;
use solana_nonce::{
    state::{Data, DurableNonce, State},
    versions::Versions,
};
use solana_pubkey::Pubkey as OfficialPubkey;
use solana_system_interface::instruction as system_instruction;

fn official(key: Pubkey) -> OfficialPubkey {
    OfficialPubkey::from(key.to_bytes())
}

fn key(byte: u8) -> Pubkey {
    Pubkey::new([byte; 32])
}

/// Serialize an initialized, current-version nonce account exactly as the
/// System Program stores it, using official crates.
fn official_initialized_bytes(authority: Pubkey, nonce_seed: [u8; 32], lamports: u64) -> Vec<u8> {
    let durable = DurableNonce::from_blockhash(&Hash::new_from_array(nonce_seed));
    let data = Data::new(official(authority), durable, lamports);
    bincode::serialize(&Versions::new(State::Initialized(data))).expect("serialize nonce account")
}

#[test]
fn initialized_nonce_account_parses_and_matches_official_bytes() {
    let authority = key(0x11);
    let lamports = 5000u64;
    let bytes = official_initialized_bytes(authority, [0x22; 32], lamports);

    // Official serialized length is exactly the canonical 80.
    assert_eq!(bytes.len(), NONCE_ACCOUNT_LENGTH);
    // Version wrapper = Current (1), state = Initialized (1), both u32 LE.
    assert_eq!(&bytes[0..4], &[1, 0, 0, 0]);
    assert_eq!(&bytes[4..8], &[1, 0, 0, 0]);
    // Authority at [8,40); lamports_per_signature u64 LE at [72,80).
    assert_eq!(&bytes[8..40], authority.as_bytes());
    assert_eq!(&bytes[72..80], &lamports.to_le_bytes());

    let parsed = parse_nonce_account_data(&bytes).expect("parse official bytes");
    let expected_nonce: [u8; 32] = bytes[40..72].try_into().unwrap();
    assert_eq!(
        parsed,
        NonceAccount {
            authority,
            durable_nonce: expected_nonce,
            lamports_per_signature: lamports,
        }
    );
}

#[test]
fn parser_rejects_legacy_uninitialized_and_malformed() {
    let authority = key(0x11);
    let data = Data::new(
        official(authority),
        DurableNonce::from_blockhash(&Hash::new_from_array([0x22; 32])),
        5000,
    );

    // Legacy wrapper (version disc 0) — refused as unsupported.
    let legacy = bincode::serialize(&Versions::Legacy(Box::new(State::Initialized(data.clone()))))
        .expect("serialize legacy");
    assert_eq!(legacy.len(), NONCE_ACCOUNT_LENGTH);
    assert_eq!(
        parse_nonce_account_data(&legacy),
        Err(NonceError::UnsupportedVersion(0))
    );

    // Current + Uninitialized, padded to the canonical length.
    let mut uninit =
        bincode::serialize(&Versions::new(State::Uninitialized)).expect("serialize uninit");
    uninit.resize(NONCE_ACCOUNT_LENGTH, 0);
    assert_eq!(
        parse_nonce_account_data(&uninit),
        Err(NonceError::Uninitialized)
    );

    // A freshly allocated (all-zero) 80-byte account decodes as version 0.
    assert_eq!(
        parse_nonce_account_data(&[0u8; NONCE_ACCOUNT_LENGTH]),
        Err(NonceError::UnsupportedVersion(0))
    );

    // Length must be exactly 80 (bincode tolerates trailing bytes; we do not).
    let good = official_initialized_bytes(authority, [0x22; 32], 5000);
    let mut oversized = good.clone();
    oversized.push(0);
    assert_eq!(
        parse_nonce_account_data(&oversized),
        Err(NonceError::InvalidLength(81))
    );
    assert_eq!(
        parse_nonce_account_data(&good[..79]),
        Err(NonceError::InvalidLength(79))
    );

    // Unknown future version / state discriminants fail closed.
    let mut future_version = good.clone();
    future_version[0..4].copy_from_slice(&2u32.to_le_bytes());
    assert_eq!(
        parse_nonce_account_data(&future_version),
        Err(NonceError::UnsupportedVersion(2))
    );
    let mut future_state = good;
    future_state[4..8].copy_from_slice(&7u32.to_le_bytes());
    assert_eq!(
        parse_nonce_account_data(&future_state),
        Err(NonceError::UnknownState(7))
    );
}

#[test]
fn advance_nonce_account_matches_official_instruction() {
    let nonce_account = key(0x33);
    let nonce_authority = key(0x44);

    let actual = advance_nonce_account(nonce_account, nonce_authority);
    let expected =
        system_instruction::advance_nonce_account(&official(nonce_account), &official(nonce_authority));

    assert_eq!(actual.program_id.to_bytes(), expected.program_id.to_bytes());
    assert_eq!(actual.data, expected.data);
    assert_eq!(actual.data, ADVANCE_NONCE_ACCOUNT_DATA);
    assert_eq!(actual.accounts.len(), expected.accounts.len());
    assert_eq!(actual.accounts.len(), 3);
    for (i, (a, e)) in actual.accounts.iter().zip(&expected.accounts).enumerate() {
        assert_eq!(a.pubkey.to_bytes(), e.pubkey.to_bytes(), "account {i} pubkey");
        assert_eq!(a.is_signer, e.is_signer, "account {i} signer");
        assert_eq!(a.is_writable, e.is_writable, "account {i} writable");
    }
    // Explicit privilege expectations.
    assert!(!actual.accounts[0].is_signer && actual.accounts[0].is_writable);
    assert!(!actual.accounts[1].is_signer && !actual.accounts[1].is_writable);
    assert!(actual.accounts[2].is_signer && !actual.accounts[2].is_writable);
}

#[test]
fn program_and_sysvar_ids_match_official() {
    // System program id is all-zero and equals nanosol's constant.
    assert_eq!(
        SYSTEM_PROGRAM_ID.to_bytes(),
        solana_sdk_ids::system_program::ID.to_bytes()
    );
    assert_eq!(SYSTEM_PROGRAM_ID.to_bytes(), [0u8; 32]);

    // Recent-blockhashes sysvar id matches the official (deprecated-for-read,
    // still-required-as-account) constant.
    #[allow(deprecated)]
    let official_sysvar = solana_sdk_ids::sysvar::recent_blockhashes::ID;
    assert_eq!(
        RECENT_BLOCKHASHES_SYSVAR_ID.to_bytes(),
        official_sysvar.to_bytes()
    );
    assert_eq!(
        RECENT_BLOCKHASHES_SYSVAR_ID.to_string(),
        "SysvarRecentB1ockHashes11111111111111111111"
    );
}
