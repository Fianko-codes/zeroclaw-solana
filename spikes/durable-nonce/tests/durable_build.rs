//! Prove a full unsigned durable transfer composes with the frozen `nanosol`
//! message codec: AdvanceNonceAccount at index 0, message blockhash == nonce,
//! exactly one all-zero signature, and all M3.5 instructions intact.

use durable_nonce_spike::{
    build_durable_transfer, decode_advance_nonce_account, DecodeError,
};
use nanosol::{
    inspect::{
        decode_ata_create_idempotent, decode_memo, decode_transfer_checked,
        decode_unsigned_v0_transaction,
    },
    instruction::{
        create_associated_token_account_idempotent, memo as memo_instruction, transfer_checked,
        TokenProgram,
    },
    pubkey::{derive_associated_token_address, Pubkey},
};

fn key(byte: u8) -> Pubkey {
    Pubkey::new([byte; 32])
}

fn build_sample(nonce: [u8; 32]) -> (Vec<u8>, Pubkey, Pubkey) {
    let sender = key(0x01);
    let recipient = key(0x02);
    let mint = key(0x03);
    let nonce_account = key(0x0a);
    let token_program = TokenProgram::Legacy;

    let (create, destination) =
        create_associated_token_account_idempotent(sender, recipient, mint, token_program)
            .expect("ata create");
    let (source, _) =
        derive_associated_token_address(&sender, &mint, &token_program.id()).expect("source ata");
    let transfer = transfer_checked(source, mint, destination, sender, 1_000_000, 6, token_program);
    let memo = memo_instruction("invoice 412");

    let bytes = build_durable_transfer(sender, nonce_account, nonce, vec![create, transfer, memo])
        .expect("build durable transfer");
    (bytes, sender, nonce_account)
}

#[test]
fn durable_transfer_is_single_signer_v0_with_nonce_blockhash() {
    let nonce = [0x22; 32];
    let (bytes, sender, nonce_account) = build_sample(nonce);
    assert!(bytes.len() <= nanosol::message::MAX_TRANSACTION_BYTES);

    let tx = decode_unsigned_v0_transaction(&bytes).expect("decode");
    let message = &tx.message;

    // Exactly one required signature, one all-zero signature slot, sender is fee payer.
    assert_eq!(tx.signatures.len(), 1);
    assert!(tx.is_unsigned());
    assert_eq!(message.header.num_required_signatures, 1);
    assert_eq!(message.header.num_readonly_signed_accounts, 0);
    assert_eq!(message.account_keys.first(), Some(&sender));

    // Message blockhash equals the durable nonce value (not a recent blockhash).
    assert_eq!(message.recent_blockhash, nonce);

    // Instruction 0 is AdvanceNonceAccount with the configured nonce account and
    // sender as the (only) nonce-authority signer.
    let advance = decode_advance_nonce_account(message, 0).expect("advance at 0");
    assert_eq!(advance.nonce_account, nonce_account);
    assert_eq!(advance.nonce_authority, sender);

    // The M3.5 instruction set is intact at 1..4.
    let create = decode_ata_create_idempotent(message, 1).expect("ata");
    assert_eq!(create.payer, sender);
    let transfer = decode_transfer_checked(message, 2).expect("transfer");
    assert_eq!(transfer.amount, 1_000_000);
    assert_eq!(transfer.authority, sender);
    let memo = decode_memo(message, 3).expect("memo");
    assert_eq!(memo, "invoice 412");
    assert_eq!(message.instructions.len(), 4);
}

#[test]
fn advance_decoder_requires_index_zero_shape() {
    let nonce = [0x22; 32];
    let (bytes, _, _) = build_sample(nonce);
    let tx = decode_unsigned_v0_transaction(&bytes).expect("decode");

    // The ATA-create instruction at index 1 is not an AdvanceNonceAccount.
    assert!(matches!(
        decode_advance_nonce_account(&tx.message, 1),
        Err(DecodeError::UnexpectedProgram) | Err(DecodeError::InvalidData) | Err(DecodeError::InvalidAccountCount)
    ));
    // Out-of-range index.
    assert_eq!(
        decode_advance_nonce_account(&tx.message, 9),
        Err(DecodeError::OutOfBounds)
    );
}
