//! Byte-exact oracles for the durable-nonce primitives against official crates:
//! `solana-nonce` (account layout), `solana-system-interface`
//! (`AdvanceNonceAccount`), and `solana-sdk-ids` (program/sysvar ids).

use nanosol::{
    inspect::{decode_advance_nonce_account, DecodedAdvanceNonce, InspectError},
    instruction::{advance_nonce_account, memo, ADVANCE_NONCE_ACCOUNT_DATA},
    message::{Message, MessageVersion},
    nonce::{parse_nonce_account_data, NonceAccount, NonceError, NONCE_ACCOUNT_LENGTH},
    pubkey::{Pubkey, RECENT_BLOCKHASHES_SYSVAR_ID, SYSTEM_PROGRAM_ID},
};

use solana_hash::Hash;
use solana_nonce::{
    state::{Data, DurableNonce, State},
    versions::Versions,
};
use solana_pubkey::Pubkey as OfficialPubkey;
use solana_system_interface::instruction as system_instruction;

fn key(byte: u8) -> Pubkey {
    Pubkey::new([byte; 32])
}

fn official(key: Pubkey) -> OfficialPubkey {
    OfficialPubkey::from(key.to_bytes())
}

fn official_initialized_bytes(authority: Pubkey, nonce_seed: [u8; 32], lamports: u64) -> Vec<u8> {
    let durable = DurableNonce::from_blockhash(&Hash::new_from_array(nonce_seed));
    let data = Data::new(official(authority), durable, lamports);
    bincode::serialize(&Versions::new(State::Initialized(data))).expect("serialize nonce account")
}

#[test]
fn initialized_nonce_account_matches_official_bytes_and_parses() {
    let authority = key(0x11);
    let lamports = 5000u64;
    let bytes = official_initialized_bytes(authority, [0x22; 32], lamports);

    assert_eq!(bytes.len(), NONCE_ACCOUNT_LENGTH);
    assert_eq!(&bytes[0..4], &[1, 0, 0, 0]); // Versions::Current
    assert_eq!(&bytes[4..8], &[1, 0, 0, 0]); // State::Initialized
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

    let legacy = bincode::serialize(&Versions::Legacy(Box::new(State::Initialized(
        data.clone(),
    ))))
    .expect("serialize legacy");
    assert_eq!(
        parse_nonce_account_data(&legacy),
        Err(NonceError::UnsupportedVersion(0))
    );

    let mut uninit =
        bincode::serialize(&Versions::new(State::Uninitialized)).expect("serialize uninit");
    uninit.resize(NONCE_ACCOUNT_LENGTH, 0);
    assert_eq!(
        parse_nonce_account_data(&uninit),
        Err(NonceError::Uninitialized)
    );

    assert_eq!(
        parse_nonce_account_data(&[0u8; NONCE_ACCOUNT_LENGTH]),
        Err(NonceError::UnsupportedVersion(0))
    );

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
    let expected = system_instruction::advance_nonce_account(
        &official(nonce_account),
        &official(nonce_authority),
    );

    assert_eq!(actual.program_id.to_bytes(), expected.program_id.to_bytes());
    assert_eq!(actual.data, expected.data);
    assert_eq!(actual.data, ADVANCE_NONCE_ACCOUNT_DATA);
    assert_eq!(actual.accounts.len(), 3);
    assert_eq!(actual.accounts.len(), expected.accounts.len());
    for (i, (a, e)) in actual.accounts.iter().zip(&expected.accounts).enumerate() {
        assert_eq!(
            a.pubkey.to_bytes(),
            e.pubkey.to_bytes(),
            "account {i} pubkey"
        );
        assert_eq!(a.is_signer, e.is_signer, "account {i} signer");
        assert_eq!(a.is_writable, e.is_writable, "account {i} writable");
    }
    assert!(!actual.accounts[0].is_signer && actual.accounts[0].is_writable);
    assert!(!actual.accounts[1].is_signer && !actual.accounts[1].is_writable);
    assert!(actual.accounts[2].is_signer && !actual.accounts[2].is_writable);
}

#[test]
fn decode_advance_nonce_reads_index_zero_and_rejects_non_advance() {
    let sender = key(0x01); // payer == nonce authority (the M4 arrangement)
    let nonce_account = key(0x0a);
    let instructions = vec![advance_nonce_account(nonce_account, sender), memo("x")];
    let message =
        Message::compile(MessageVersion::V0, sender, [0x22; 32], &instructions).expect("compile");

    assert_eq!(
        decode_advance_nonce_account(&message, 0),
        Ok(DecodedAdvanceNonce {
            nonce_account,
            nonce_authority: sender,
        })
    );
    // The memo at index 1 is not an AdvanceNonceAccount.
    assert_eq!(
        decode_advance_nonce_account(&message, 1),
        Err(InspectError::UnexpectedProgram)
    );
    assert!(matches!(
        decode_advance_nonce_account(&message, 9),
        Err(InspectError::InstructionOutOfBounds(9))
    ));
}

#[test]
fn program_and_sysvar_ids_match_official() {
    assert_eq!(
        SYSTEM_PROGRAM_ID.to_bytes(),
        solana_sdk_ids::system_program::ID.to_bytes()
    );
    assert_eq!(SYSTEM_PROGRAM_ID.to_bytes(), [0u8; 32]);

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

#[test]
fn durable_simulation_request_never_replaces_the_blockhash() {
    let request = nanosol::rpc::simulate_durable_transaction_request(7, "BASE64TX");
    let value: serde_json::Value = serde_json::from_str(&request).expect("json");
    assert_eq!(value["method"], "simulateTransaction");
    assert_eq!(value["id"], 7);
    let options = &value["params"][1];
    assert_eq!(options["replaceRecentBlockhash"], false);
    assert_eq!(options["sigVerify"], false);
    assert_eq!(options["encoding"], "base64");
    assert_eq!(value["params"][0], "BASE64TX");
}

#[test]
fn simulation_response_flags_a_replaced_blockhash() {
    let without = r#"{"jsonrpc":"2.0","id":9,"result":{"context":{"slot":1},"value":{"err":null,"logs":[]}}}"#;
    let parsed = nanosol::rpc::parse_simulation_response(without, 9).expect("no replacement");
    assert!(!parsed.replaced_blockhash);
    assert!(parsed.is_success());

    let replaced = r#"{"jsonrpc":"2.0","id":9,"result":{"context":{"slot":1},"value":{"err":null,"logs":[],"replacementBlockhash":{"blockhash":"C1x…","lastValidBlockHeight":42}}}}"#;
    let parsed = nanosol::rpc::parse_simulation_response(replaced, 9).expect("replacement present");
    assert!(parsed.replaced_blockhash);

    let null_replacement = r#"{"jsonrpc":"2.0","id":9,"result":{"context":{"slot":1},"value":{"err":null,"replacementBlockhash":null}}}"#;
    let parsed =
        nanosol::rpc::parse_simulation_response(null_replacement, 9).expect("null replacement");
    assert!(!parsed.replaced_blockhash);
}
