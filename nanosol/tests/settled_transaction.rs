//! Golden vectors for decoding *settled* transactions: the shapes a real wallet
//! submits when it pays a Solana Pay request.
//!
//! Message bytes are produced by the official `solana-message` /
//! `solana-transaction` crates and the official SPL Token instruction builders,
//! then the all-zero signature slots are overwritten with a non-zero pattern.
//! Nothing here verifies Ed25519 — the decoder only requires that a landed
//! transaction has no empty signature slot — so a synthetic pattern is the
//! correct fixture and keeps the test free of any signing dependency.

use nanosol::{
    inspect::{
        decode_signed_transaction, decode_token_transfer, find_token_transfers,
        DecodedTokenTransfer, InspectError, TokenTransferKind,
    },
    instruction::{AccountMeta, Instruction, TokenProgram},
    message::{Message, MessageError, MessageVersion, Transaction, SIGNATURE_BYTES},
    pubkey::{
        derive_associated_token_address, Pubkey, LEGACY_TOKEN_PROGRAM_ID, MEMO_V3_PROGRAM_ID,
        TOKEN_2022_PROGRAM_ID,
    },
};
use solana_hash::Hash as OfficialHash;
use solana_instruction::{AccountMeta as OfficialAccountMeta, Instruction as OfficialInstruction};
use solana_message::{Message as OfficialLegacyMessage, VersionedMessage};
use solana_pubkey::Pubkey as OfficialPubkey;
use solana_transaction::{versioned::VersionedTransaction, Transaction as OfficialTransaction};

const AMOUNT: u64 = 1_500_000;
const DECIMALS: u8 = 6;
const BLOCKHASH: [u8; 32] = [9; 32];

fn key(byte: u8) -> Pubkey {
    Pubkey::new([byte; 32])
}

fn official(key: Pubkey) -> OfficialPubkey {
    OfficialPubkey::from(key.to_bytes())
}

struct Fixture {
    payer: Pubkey,
    recipient: Pubkey,
    mint: Pubkey,
    source: Pubkey,
    destination: Pubkey,
    reference: Pubkey,
    unrelated_reference: Pubkey,
}

fn fixture() -> Fixture {
    let payer = key(1);
    let recipient = key(2);
    let mint = key(3);
    let (source, _) =
        derive_associated_token_address(&payer, &mint, &LEGACY_TOKEN_PROGRAM_ID).expect("source");
    let (destination, _) =
        derive_associated_token_address(&recipient, &mint, &LEGACY_TOKEN_PROGRAM_ID)
            .expect("destination");
    Fixture {
        payer,
        recipient,
        mint,
        source,
        destination,
        reference: key(7),
        unrelated_reference: key(8),
    }
}

/// The official `TransferChecked` a Solana Pay wallet builds, with the reference
/// attached to the transfer instruction as a read-only non-signer.
fn official_transfer_checked(fixture: &Fixture, references: &[Pubkey]) -> OfficialInstruction {
    let mut instruction = spl_token_interface::instruction::transfer_checked(
        &official(LEGACY_TOKEN_PROGRAM_ID),
        &official(fixture.source),
        &official(fixture.mint),
        &official(fixture.destination),
        &official(fixture.payer),
        &[],
        AMOUNT,
        DECIMALS,
    )
    .expect("official transfer_checked");
    for reference in references {
        instruction.accounts.push(OfficialAccountMeta::new_readonly(
            official(*reference),
            false,
        ));
    }
    instruction
}

/// A message-level account that is *not* part of the transfer instruction, used
/// to prove that reference presence is scoped to the transfer instruction.
fn unrelated_instruction(fixture: &Fixture) -> OfficialInstruction {
    OfficialInstruction {
        program_id: official(MEMO_V3_PROGRAM_ID),
        accounts: vec![OfficialAccountMeta::new_readonly(
            official(fixture.unrelated_reference),
            false,
        )],
        data: b"invoice 412".to_vec(),
    }
}

/// Serialize an official transaction and fill every signature slot with a
/// non-zero pattern, reproducing a landed transaction's wire bytes.
fn signed_bytes(message: VersionedMessage) -> Vec<u8> {
    let signature_count = usize::from(message.header().num_required_signatures);
    assert!(
        signature_count < 128,
        "fixture keeps a one-byte length prefix"
    );
    let mut bytes = match message {
        VersionedMessage::Legacy(message) => {
            bincode::serialize(&OfficialTransaction::new_unsigned(message))
        }
        message => bincode::serialize(&VersionedTransaction {
            signatures: vec![Default::default(); signature_count],
            message,
        }),
    }
    .expect("official transaction bytes");
    for byte in &mut bytes[1..=signature_count * SIGNATURE_BYTES] {
        *byte = 0x5a;
    }
    bytes
}

fn legacy_bytes(fixture: &Fixture, instructions: &[OfficialInstruction]) -> Vec<u8> {
    signed_bytes(VersionedMessage::Legacy(
        OfficialLegacyMessage::new_with_blockhash(
            instructions,
            Some(&official(fixture.payer)),
            &OfficialHash::new_from_array(BLOCKHASH),
        ),
    ))
}

fn v0_bytes(fixture: &Fixture, instructions: &[OfficialInstruction]) -> Vec<u8> {
    signed_bytes(VersionedMessage::V0(
        solana_message::v0::Message::try_compile(
            &official(fixture.payer),
            instructions,
            &[],
            OfficialHash::new_from_array(BLOCKHASH),
        )
        .expect("official v0 compile"),
    ))
}

fn single_transfer(bytes: &[u8]) -> (Transaction, DecodedTokenTransfer) {
    let transaction = decode_signed_transaction(bytes).expect("settled transaction");
    let transfers = find_token_transfers(&transaction.message).expect("transfer scan");
    assert_eq!(transfers.len(), 1, "expected exactly one token transfer");
    let (index, transfer) = transfers.into_iter().next().expect("transfer");
    assert_eq!(
        decode_token_transfer(&transaction.message, index),
        Ok(transfer.clone())
    );
    (transaction, transfer)
}

#[test]
fn legacy_and_v0_settled_transfer_checked_decode_identically() {
    let fixture = fixture();
    let instructions = [
        official_transfer_checked(&fixture, &[fixture.reference]),
        unrelated_instruction(&fixture),
    ];

    for bytes in [
        legacy_bytes(&fixture, &instructions),
        v0_bytes(&fixture, &instructions),
    ] {
        let (transaction, transfer) = single_transfer(&bytes);
        assert_eq!(transaction.signatures.len(), 1);
        assert_eq!(transaction.message.recent_blockhash, BLOCKHASH);
        assert_eq!(transfer.kind, TokenTransferKind::TransferChecked);
        assert_eq!(transfer.token_program, TokenProgram::Legacy);
        assert_eq!(transfer.amount, AMOUNT);
        assert_eq!(transfer.decimals, Some(DECIMALS));
        assert_eq!(transfer.mint, Some(fixture.mint));
        assert_eq!(transfer.source, fixture.source);
        assert_eq!(transfer.destination, fixture.destination);
        assert_eq!(transfer.authority.pubkey, fixture.payer);
        assert!(transfer.authority.is_signer);

        // The reference counts only because it is attached to *this* instruction
        // as a read-only non-signer.
        assert!(transfer.carries_reference(&fixture.reference));
        assert!(!transfer.carries_reference(&fixture.unrelated_reference));
        assert!(transaction
            .message
            .account_keys
            .contains(&fixture.unrelated_reference));
        assert!(!transfer.carries_reference(&fixture.mint));
        assert!(!transfer.carries_reference(&fixture.destination));

        // Re-serializing the decoded transaction reproduces the wire bytes, so
        // nothing in the message was dropped by the decoder.
        assert_eq!(
            decode_signed_transaction(bytes.as_slice())
                .and_then(|decoded| decoded.serialize().map_err(InspectError::from)),
            Ok(bytes.clone())
        );
    }
}

#[test]
fn plain_transfer_decodes_without_mint_or_decimals() {
    let fixture = fixture();
    let mut instruction = spl_token_interface::instruction::transfer(
        &official(LEGACY_TOKEN_PROGRAM_ID),
        &official(fixture.source),
        &official(fixture.destination),
        &official(fixture.payer),
        &[],
        AMOUNT,
    )
    .expect("official transfer");
    instruction.accounts.push(OfficialAccountMeta::new_readonly(
        official(fixture.reference),
        false,
    ));

    let (_, transfer) = single_transfer(&legacy_bytes(&fixture, &[instruction]));
    assert_eq!(transfer.kind, TokenTransferKind::Transfer);
    assert_eq!(transfer.mint, None);
    assert_eq!(transfer.decimals, None);
    assert_eq!(transfer.amount, AMOUNT);
    assert_eq!(transfer.source, fixture.source);
    assert_eq!(transfer.destination, fixture.destination);
    assert!(transfer.carries_reference(&fixture.reference));
}

#[test]
fn token_2022_transfer_is_identified_by_its_program() {
    let fixture = fixture();
    let instruction = spl_token_2022_interface::instruction::transfer_checked(
        &official(TOKEN_2022_PROGRAM_ID),
        &official(fixture.source),
        &official(fixture.mint),
        &official(fixture.destination),
        &official(fixture.payer),
        &[],
        AMOUNT,
        DECIMALS,
    )
    .expect("official Token-2022 transfer");
    let (_, transfer) = single_transfer(&v0_bytes(&fixture, &[instruction]));
    assert_eq!(transfer.token_program, TokenProgram::Token2022);
    assert_eq!(transfer.decimals, Some(DECIMALS));
}

#[test]
fn multisig_signers_are_extra_accounts_and_never_count_as_a_reference() {
    let fixture = fixture();
    let multisig = key(20);
    let first_signer = key(21);
    let second_signer = key(22);
    let mut instruction = spl_token_interface::instruction::transfer_checked(
        &official(LEGACY_TOKEN_PROGRAM_ID),
        &official(fixture.source),
        &official(fixture.mint),
        &official(fixture.destination),
        &official(multisig),
        &[&official(first_signer), &official(second_signer)],
        AMOUNT,
        DECIMALS,
    )
    .expect("official multisig transfer");
    instruction.accounts.push(OfficialAccountMeta::new_readonly(
        official(fixture.reference),
        false,
    ));

    let (_, transfer) = single_transfer(&legacy_bytes(&fixture, &[instruction]));
    assert_eq!(transfer.authority.pubkey, multisig);
    assert!(!transfer.authority.is_signer);
    assert_eq!(transfer.extra_accounts.len(), 3);
    assert!(transfer.carries_reference(&fixture.reference));
    // A participating signer is in the instruction's account list but is a
    // signer, so it can never be mistaken for a Solana Pay reference.
    assert!(!transfer.carries_reference(&first_signer));
    assert!(!transfer.carries_reference(&second_signer));
}

#[test]
fn two_transfers_in_one_transaction_are_both_reported() {
    let fixture = fixture();
    let other_recipient = key(30);
    let (other_destination, _) =
        derive_associated_token_address(&other_recipient, &fixture.mint, &LEGACY_TOKEN_PROGRAM_ID)
            .expect("other destination");
    let second = spl_token_interface::instruction::transfer_checked(
        &official(LEGACY_TOKEN_PROGRAM_ID),
        &official(fixture.source),
        &official(fixture.mint),
        &official(other_destination),
        &official(fixture.payer),
        &[],
        AMOUNT,
        DECIMALS,
    )
    .expect("second official transfer");
    let bytes = legacy_bytes(
        &fixture,
        &[
            official_transfer_checked(&fixture, &[fixture.reference]),
            second,
        ],
    );
    let transaction = decode_signed_transaction(&bytes).expect("settled transaction");
    let transfers = find_token_transfers(&transaction.message).expect("transfer scan");
    assert_eq!(transfers.len(), 2);
    assert_eq!(transfers[0].1.destination, fixture.destination);
    assert_eq!(transfers[1].1.destination, other_destination);
}

#[test]
fn unsigned_or_partially_signed_transactions_are_refused() {
    let fixture = fixture();
    let instructions = [official_transfer_checked(&fixture, &[fixture.reference])];
    let mut bytes = legacy_bytes(&fixture, &instructions);
    assert!(decode_signed_transaction(&bytes).is_ok());

    let mut zeroed = bytes.clone();
    for byte in &mut zeroed[1..=SIGNATURE_BYTES] {
        *byte = 0;
    }
    assert_eq!(
        decode_signed_transaction(&zeroed),
        Err(InspectError::MissingSignature)
    );

    // Truncation anywhere is a decode error, never a partial decode.
    for end in 0..bytes.len() {
        assert!(
            decode_signed_transaction(&bytes[..end]).is_err(),
            "accepted truncation at {end}"
        );
    }
    bytes.push(0);
    assert_eq!(
        decode_signed_transaction(&bytes),
        Err(InspectError::Message(MessageError::TrailingBytes(1)))
    );
}

#[test]
fn address_table_lookups_are_refused_rather_than_misindexed() {
    let fixture = fixture();
    let mut bytes = v0_bytes(
        &fixture,
        &[official_transfer_checked(&fixture, &[fixture.reference])],
    );
    *bytes.last_mut().expect("lookup count") = 1;
    assert_eq!(
        decode_signed_transaction(&bytes),
        Err(InspectError::Message(
            MessageError::AddressTableLookupsUnsupported(1)
        ))
    );
}

#[test]
fn malformed_transfer_instructions_fail_closed_instead_of_being_skipped() {
    let fixture = fixture();
    let short_data = Instruction {
        program_id: LEGACY_TOKEN_PROGRAM_ID,
        accounts: vec![
            AccountMeta::writable(fixture.source, false),
            AccountMeta::readonly(fixture.mint, false),
            AccountMeta::writable(fixture.destination, false),
            AccountMeta::readonly(fixture.payer, true),
        ],
        data: vec![12, 0, 0],
    };
    let readonly_destination = Instruction {
        program_id: LEGACY_TOKEN_PROGRAM_ID,
        accounts: vec![
            AccountMeta::writable(fixture.source, false),
            AccountMeta::readonly(fixture.mint, false),
            AccountMeta::readonly(fixture.destination, false),
            AccountMeta::readonly(fixture.payer, true),
        ],
        data: vec![12, 0, 0, 0, 0, 0, 0, 0, 0, DECIMALS],
    };
    let missing_accounts = Instruction {
        program_id: LEGACY_TOKEN_PROGRAM_ID,
        accounts: vec![
            AccountMeta::writable(fixture.source, false),
            AccountMeta::readonly(fixture.mint, false),
            AccountMeta::writable(fixture.destination, false),
        ],
        data: vec![12, 0, 0, 0, 0, 0, 0, 0, 0, DECIMALS],
    };

    for instruction in [short_data, readonly_destination, missing_accounts] {
        let message = Message::compile(
            MessageVersion::V0,
            fixture.recipient,
            BLOCKHASH,
            &[instruction],
        )
        .expect("compile hostile fixture");
        assert!(
            find_token_transfers(&message).is_err(),
            "a malformed token transfer must be an error, not a skipped instruction"
        );
    }
}
