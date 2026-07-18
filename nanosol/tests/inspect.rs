use nanosol::{
    inspect::{
        decode_ata_create_idempotent, decode_memo, decode_transfer_checked,
        decode_unsigned_v0_transaction, InspectError,
    },
    instruction::{
        create_associated_token_account_idempotent, memo, transfer_checked, AccountMeta,
        TokenProgram,
    },
    message::{Message, MessageError, MessageVersion, Transaction},
    pubkey::{derive_associated_token_address, Pubkey},
};

fn key(byte: u8) -> Pubkey {
    Pubkey::new([byte; 32])
}

fn fixture(with_reference: bool, with_memo: bool) -> (Vec<u8>, Pubkey, Pubkey, Pubkey, Pubkey) {
    let sender = key(1);
    let recipient = key(2);
    let mint = key(3);
    let reference = key(4);
    let token_program = TokenProgram::Legacy;
    let (source, _) =
        derive_associated_token_address(&sender, &mint, &token_program.id()).expect("source ATA");
    let (create, destination) =
        create_associated_token_account_idempotent(sender, recipient, mint, token_program)
            .expect("destination ATA");
    let mut transfer = transfer_checked(
        source,
        mint,
        destination,
        sender,
        25_010_000,
        6,
        token_program,
    );
    if with_reference {
        transfer
            .accounts
            .push(AccountMeta::readonly(reference, false));
    }
    let mut instructions = vec![create, transfer];
    if with_memo {
        instructions.push(memo("invoice 412"));
    }
    let message = Message::compile(MessageVersion::V0, sender, [9; 32], &instructions)
        .expect("compile fixture");
    let bytes = Transaction::new_unsigned(message)
        .serialize()
        .expect("serialize fixture");
    (bytes, sender, recipient, mint, reference)
}

#[test]
fn exact_supported_instruction_subset_decodes_semantically() {
    let (bytes, sender, recipient, mint, reference) = fixture(true, true);
    let transaction = decode_unsigned_v0_transaction(&bytes).expect("unsigned v0 transaction");
    assert_eq!(transaction.signatures, vec![[0; 64]]);
    assert_eq!(transaction.message.header.num_required_signatures, 1);

    let create = decode_ata_create_idempotent(&transaction.message, 0).expect("ATA instruction");
    assert_eq!(create.payer, sender);
    assert_eq!(create.owner, recipient);
    assert_eq!(create.mint, mint);
    assert_eq!(create.token_program, TokenProgram::Legacy);

    let transfer = decode_transfer_checked(&transaction.message, 1).expect("transfer instruction");
    assert_eq!(transfer.authority, sender);
    assert_eq!(transfer.mint, mint);
    assert_eq!(transfer.amount, 25_010_000);
    assert_eq!(transfer.decimals, 6);
    assert_eq!(transfer.reference, Some(reference));
    assert_eq!(
        transfer.source,
        nanosol::pubkey::derive_associated_token_address(
            &sender,
            &mint,
            &TokenProgram::Legacy.id(),
        )
        .expect("source")
        .0
    );
    assert_eq!(transfer.destination, create.ata);
    assert_eq!(
        decode_memo(&transaction.message, 2).expect("memo"),
        "invoice 412"
    );
}

#[test]
fn optional_reference_and_memo_are_absent_without_extra_structure() {
    let (bytes, _, _, _, _) = fixture(false, false);
    let transaction = decode_unsigned_v0_transaction(&bytes).expect("transaction");
    assert_eq!(transaction.message.instructions.len(), 2);
    assert_eq!(
        decode_transfer_checked(&transaction.message, 1)
            .expect("transfer")
            .reference,
        None
    );
    assert_eq!(
        decode_memo(&transaction.message, 2),
        Err(InspectError::InstructionOutOfBounds(2))
    );
}

#[test]
fn nonzero_signature_legacy_trailing_and_malformed_instructions_fail_closed() {
    let (bytes, sender, _, _, _) = fixture(true, true);

    let mut signed = bytes.clone();
    signed[1] = 1;
    assert_eq!(
        decode_unsigned_v0_transaction(&signed),
        Err(InspectError::NonzeroSignature)
    );

    let legacy_message =
        Message::compile(MessageVersion::Legacy, sender, [9; 32], &[]).expect("legacy message");
    let legacy = Transaction::new_unsigned(legacy_message)
        .serialize()
        .expect("legacy transaction");
    assert_eq!(
        decode_unsigned_v0_transaction(&legacy),
        Err(InspectError::WrongMessageVersion)
    );

    let mut trailing = bytes.clone();
    trailing.push(0);
    assert!(matches!(
        decode_unsigned_v0_transaction(&trailing),
        Err(InspectError::Message(MessageError::TrailingBytes(1)))
    ));

    let mut transaction = Transaction::deserialize(&bytes).expect("fixture transaction");
    transaction.message.instructions[0].data.clear();
    assert_eq!(
        decode_ata_create_idempotent(&transaction.message, 0),
        Err(InspectError::InvalidInstructionData)
    );

    transaction = Transaction::deserialize(&bytes).expect("fixture transaction");
    transaction.message.instructions[1].data[0] = 3;
    assert_eq!(
        decode_transfer_checked(&transaction.message, 1),
        Err(InspectError::InvalidInstructionData)
    );

    transaction = Transaction::deserialize(&bytes).expect("fixture transaction");
    transaction.message.instructions[2].account_indexes.push(0);
    assert_eq!(
        decode_memo(&transaction.message, 2),
        Err(InspectError::InvalidAccountCount)
    );
}
