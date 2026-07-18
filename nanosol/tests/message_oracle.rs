use nanosol::{
    instruction::{
        create_associated_token_account_idempotent, memo, set_compute_unit_limit, transfer_checked,
        AccountMeta, Instruction, TokenProgram,
    },
    message::{Message, MessageError, MessageVersion, Transaction, MAX_TRANSACTION_BYTES},
    pubkey::Pubkey,
};
use solana_hash::Hash as OfficialHash;
use solana_instruction::{AccountMeta as OfficialAccountMeta, Instruction as OfficialInstruction};
use solana_message::{
    Message as OfficialLegacyMessage, VersionedMessage as OfficialVersionedMessage,
};
use solana_pubkey::Pubkey as OfficialPubkey;
use solana_transaction::{versioned::VersionedTransaction, Transaction as OfficialTransaction};

fn key(byte: u8) -> Pubkey {
    Pubkey::new([byte; 32])
}

fn official_key(key: Pubkey) -> OfficialPubkey {
    OfficialPubkey::from(key.to_bytes())
}

fn to_official(instruction: &Instruction) -> OfficialInstruction {
    OfficialInstruction {
        program_id: official_key(instruction.program_id),
        accounts: instruction
            .accounts
            .iter()
            .map(|account| OfficialAccountMeta {
                pubkey: official_key(account.pubkey),
                is_signer: account.is_signer,
                is_writable: account.is_writable,
            })
            .collect(),
        data: instruction.data.clone(),
    }
}

fn fixture() -> (Pubkey, [u8; 32], Vec<Instruction>) {
    let payer = key(1);
    let owner = key(2);
    let mint = key(3);
    let source = key(4);
    let (create_ata, destination) =
        create_associated_token_account_idempotent(payer, owner, mint, TokenProgram::Legacy)
            .expect("ATA fixture");
    let instructions = vec![
        set_compute_unit_limit(200_000),
        create_ata,
        transfer_checked(
            source,
            mint,
            destination,
            payer,
            25_010_000,
            6,
            TokenProgram::Legacy,
        ),
        memo("invoice #412"),
    ];
    (payer, [9; 32], instructions)
}

fn assert_compiled_fields_match(
    actual: &Message,
    expected_header: &solana_message::MessageHeader,
    expected_keys: &[OfficialPubkey],
    expected_instructions: &[solana_message::compiled_instruction::CompiledInstruction],
) {
    assert_eq!(
        actual.header.num_required_signatures,
        expected_header.num_required_signatures
    );
    assert_eq!(
        actual.header.num_readonly_signed_accounts,
        expected_header.num_readonly_signed_accounts
    );
    assert_eq!(
        actual.header.num_readonly_unsigned_accounts,
        expected_header.num_readonly_unsigned_accounts
    );
    assert_eq!(actual.account_keys.len(), expected_keys.len());
    for (actual, expected) in actual.account_keys.iter().zip(expected_keys) {
        assert_eq!(actual.to_bytes(), expected.to_bytes());
    }
    assert_eq!(actual.instructions.len(), expected_instructions.len());
    for (actual, expected) in actual.instructions.iter().zip(expected_instructions) {
        assert_eq!(actual.program_id_index, expected.program_id_index);
        assert_eq!(actual.account_indexes, expected.accounts);
        assert_eq!(actual.data, expected.data);
    }
}

#[test]
fn legacy_message_and_unsigned_transaction_match_official_bytes() {
    let (payer, blockhash, instructions) = fixture();
    let actual = Message::compile(MessageVersion::Legacy, payer, blockhash, &instructions)
        .expect("compile nanosol legacy message");

    let official_instructions: Vec<_> = instructions.iter().map(to_official).collect();
    let official_message = OfficialLegacyMessage::new_with_blockhash(
        &official_instructions,
        Some(&official_key(payer)),
        &OfficialHash::new_from_array(blockhash),
    );
    assert_compiled_fields_match(
        &actual,
        &official_message.header,
        &official_message.account_keys,
        &official_message.instructions,
    );

    let actual_message_bytes = actual.serialize().expect("serialize nanosol message");
    let expected_message_bytes = bincode::serialize(&official_message).expect("official message");
    assert_eq!(actual_message_bytes, expected_message_bytes);
    assert_eq!(
        Message::deserialize(&actual_message_bytes),
        Ok(actual.clone())
    );

    let actual_transaction = Transaction::new_unsigned(actual);
    let official_transaction = OfficialTransaction::new_unsigned(official_message);
    let actual_transaction_bytes = actual_transaction
        .serialize()
        .expect("serialize nanosol transaction");
    let expected_transaction_bytes =
        bincode::serialize(&official_transaction).expect("official transaction");
    assert_eq!(actual_transaction_bytes, expected_transaction_bytes);
    assert_eq!(
        Transaction::deserialize(&actual_transaction_bytes),
        Ok(actual_transaction.clone())
    );
    let base64 = actual_transaction.to_base64().expect("transaction base64");
    assert_eq!(Transaction::from_base64(&base64), Ok(actual_transaction));
}

#[test]
fn v0_static_message_and_unsigned_transaction_match_official_bytes() {
    let (payer, blockhash, instructions) = fixture();
    let actual =
        Message::compile(MessageVersion::V0, payer, blockhash, &instructions).expect("compile v0");
    let official_instructions: Vec<_> = instructions.iter().map(to_official).collect();
    let official_message = solana_message::v0::Message::try_compile(
        &official_key(payer),
        &official_instructions,
        &[],
        OfficialHash::new_from_array(blockhash),
    )
    .expect("official v0 compile");
    assert_compiled_fields_match(
        &actual,
        &official_message.header,
        &official_message.account_keys,
        &official_message.instructions,
    );

    let official_versioned = OfficialVersionedMessage::V0(official_message);
    let actual_message_bytes = actual.serialize().expect("nanosol v0 bytes");
    assert_eq!(
        actual_message_bytes,
        bincode::serialize(&official_versioned).expect("official v0 bytes")
    );
    assert_eq!(
        Message::deserialize(&actual_message_bytes),
        Ok(actual.clone())
    );

    let actual_transaction = Transaction::new_unsigned(actual);
    let official_transaction = VersionedTransaction {
        signatures: vec![
            Default::default();
            usize::from(official_versioned.header().num_required_signatures)
        ],
        message: official_versioned,
    };
    let bytes = actual_transaction
        .serialize()
        .expect("nanosol v0 transaction");
    assert_eq!(
        bytes,
        bincode::serialize(&official_transaction).expect("official v0 transaction")
    );
    assert_eq!(Transaction::deserialize(&bytes), Ok(actual_transaction));
}

#[test]
fn compilation_merges_privileges_and_is_deterministic() {
    let payer = key(1);
    let shared = key(9);
    let program = key(10);
    let instructions = vec![
        Instruction {
            program_id: program,
            accounts: vec![AccountMeta::readonly(shared, false)],
            data: vec![1],
        },
        Instruction {
            program_id: program,
            accounts: vec![AccountMeta::writable(shared, true)],
            data: vec![2],
        },
    ];
    let baseline =
        Message::compile(MessageVersion::Legacy, payer, [7; 32], &instructions).expect("baseline");
    let shared_index = baseline
        .account_keys
        .iter()
        .position(|key| key == &shared)
        .expect("shared key");
    assert!(shared_index < usize::from(baseline.header.num_required_signatures));
    assert_eq!(baseline.header.num_readonly_signed_accounts, 0);

    for _ in 0..64 {
        assert_eq!(
            Message::compile(MessageVersion::Legacy, payer, [7; 32], &instructions),
            Ok(baseline.clone())
        );
    }
}

#[test]
fn decoder_rejects_truncation_aliases_unknown_versions_alts_and_trailing_data() {
    let (payer, blockhash, instructions) = fixture();
    let legacy =
        Message::compile(MessageVersion::Legacy, payer, blockhash, &instructions).expect("legacy");
    let bytes = legacy.serialize().expect("legacy bytes");
    for end in 0..bytes.len() {
        assert!(
            Message::deserialize(&bytes[..end]).is_err(),
            "accepted truncation at {end}"
        );
    }

    let mut trailing = bytes.clone();
    trailing.push(0);
    assert_eq!(
        Message::deserialize(&trailing),
        Err(MessageError::TrailingBytes(1))
    );

    let mut alias = Vec::new();
    alias.extend_from_slice(&bytes[..3]);
    alias.extend_from_slice(&[bytes[3] | 0x80, 0]);
    alias.extend_from_slice(&bytes[4..]);
    assert!(matches!(
        Message::deserialize(&alias),
        Err(MessageError::CompactU16(_))
    ));

    assert_eq!(
        Message::deserialize(&[0x81]),
        Err(MessageError::UnknownMessageVersion(1))
    );

    let v0 = Message::compile(MessageVersion::V0, payer, blockhash, &instructions).expect("v0");
    let mut v0_bytes = v0.serialize().expect("v0 bytes");
    *v0_bytes.last_mut().expect("lookup count") = 1;
    assert_eq!(
        Message::deserialize(&v0_bytes),
        Err(MessageError::AddressTableLookupsUnsupported(1))
    );
}

#[test]
fn validation_rejects_structural_mismatches_and_oversize_wire_data() {
    let (payer, blockhash, instructions) = fixture();
    let message =
        Message::compile(MessageVersion::Legacy, payer, blockhash, &instructions).expect("message");

    let mut duplicate = message.clone();
    duplicate.account_keys[1] = duplicate.account_keys[0];
    assert!(matches!(
        duplicate.serialize(),
        Err(MessageError::DuplicateAccountKey(_))
    ));

    let mut bad_program_index = message.clone();
    bad_program_index.instructions[0].program_id_index = u8::MAX;
    assert!(matches!(
        bad_program_index.serialize(),
        Err(MessageError::ProgramIndexOutOfBounds { .. })
    ));

    let mut wrong_signature_count = Transaction::new_unsigned(message);
    wrong_signature_count.signatures.clear();
    assert!(matches!(
        wrong_signature_count.serialize(),
        Err(MessageError::SignatureCountMismatch { .. })
    ));

    let oversized = Message::compile(
        MessageVersion::Legacy,
        payer,
        blockhash,
        &[memo(&"x".repeat(MAX_TRANSACTION_BYTES))],
    )
    .expect("structurally valid large message");
    assert!(matches!(
        oversized.serialize(),
        Err(MessageError::WireTooLarge(_))
    ));
    assert!(Transaction::from_base64("not base64!").is_err());
}
