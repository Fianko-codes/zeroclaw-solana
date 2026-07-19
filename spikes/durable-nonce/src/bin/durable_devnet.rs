//! Phase-A devnet driver helper (spike tooling — NOT production, NOT the plugin).
//!
//! Subcommands:
//!   parse   <account_data_base64>
//!       Run the strict nonce parser on real devnet account bytes and print the
//!       decoded authority / durable nonce / lamports-per-signature.
//!
//!   build   <sender_b58> <nonce_account_b58> <nonce_value_b58> <recipient_b58> <lamports>
//!       Construct an UNSIGNED durable SOL-transfer transaction with
//!       AdvanceNonceAccount as instruction zero and the message blockhash set
//!       to the durable nonce. Prints the base64 unsigned transaction. No key.
//!
//!   sign    <keypair.json> <unsigned_tx_base64>
//!       External signer (deliberately outside the plugin/nanosol): sign the
//!       held message bytes with the disposable key and print the signed tx
//!       base64, ready to submit with `sendTransaction`.
//!
//! The `sign` step is the only place a key is used, and it lives here in the
//! spike, never in `nanosol` or the plugin.

use std::str::FromStr;

use base64::{engine::general_purpose::STANDARD, Engine as _};
use durable_nonce_spike::{advance_nonce_account, parse_nonce_account_data};
use nanosol::{
    instruction::{AccountMeta, Instruction},
    message::{Message, MessageVersion, Transaction, SIGNATURE_BYTES},
    pubkey::{Pubkey, SYSTEM_PROGRAM_ID},
};

fn b58(input: &str) -> Pubkey {
    Pubkey::from_str(input).expect("valid base58 pubkey")
}

fn b58_hash(input: &str) -> [u8; 32] {
    let decoded = bs58::decode(input).into_vec().expect("valid base58");
    decoded.try_into().expect("32-byte value")
}

/// System `Transfer` (instruction index 2): data = disc(u32 LE) ++ lamports(u64 LE).
fn system_transfer(from: Pubkey, to: Pubkey, lamports: u64) -> Instruction {
    let mut data = Vec::with_capacity(12);
    data.extend_from_slice(&2u32.to_le_bytes());
    data.extend_from_slice(&lamports.to_le_bytes());
    Instruction {
        program_id: SYSTEM_PROGRAM_ID,
        accounts: vec![
            AccountMeta::writable(from, true),
            AccountMeta::writable(to, false),
        ],
        data,
    }
}

fn cmd_parse(data_b64: &str) {
    let data = STANDARD.decode(data_b64).expect("base64 account data");
    match parse_nonce_account_data(&data) {
        Ok(account) => {
            println!("PARSE_OK");
            println!("authority={}", account.authority);
            println!("durable_nonce={}", bs58::encode(account.durable_nonce).into_string());
            println!("lamports_per_signature={}", account.lamports_per_signature);
            println!("data_len={}", data.len());
        }
        Err(error) => {
            println!("PARSE_ERR {error:?}");
            std::process::exit(2);
        }
    }
}

fn cmd_build(args: &[String]) {
    let sender = b58(&args[0]);
    let nonce_account = b58(&args[1]);
    let nonce_value = b58_hash(&args[2]);
    let recipient = b58(&args[3]);
    let lamports: u64 = args[4].parse().expect("lamports u64");
    // Optional 6th arg: override the AdvanceNonceAccount authority (to exercise
    // the runtime's wrong-authority rejection under simulation). Defaults to the
    // sender, which is the only arrangement M4 supports.
    let authority = args.get(5).map(|a| b58(a)).unwrap_or(sender);

    let instructions = vec![
        advance_nonce_account(nonce_account, authority),
        system_transfer(sender, recipient, lamports),
    ];
    let message = Message::compile(MessageVersion::V0, sender, nonce_value, &instructions)
        .expect("compile durable message");
    let bytes = Transaction::new_unsigned(message)
        .serialize()
        .expect("serialize unsigned");
    // The M4 arrangement (authority == sender) yields exactly one signature
    // slot. An authority override is a diagnostic that deliberately adds a
    // second signer — the very shape M4 forbids — so we only note it.
    if bytes[0] != 1 {
        eprintln!("note: {} signature slots (authority override in use)", bytes[0]);
    }
    println!("{}", STANDARD.encode(&bytes));
}

fn cmd_sign(keypair_path: &str, unsigned_b64: &str) {
    // Load a Solana CLI keypair file: a JSON array of 64 bytes [secret32|public32].
    let raw = std::fs::read_to_string(keypair_path).expect("read keypair");
    let bytes: Vec<u8> = serde_json::from_str(&raw).expect("keypair json array");
    assert_eq!(bytes.len(), 64, "expected 64-byte keypair");
    let seed: [u8; 32] = bytes[0..32].try_into().unwrap();
    let public: [u8; 32] = bytes[32..64].try_into().unwrap();

    let mut tx = STANDARD.decode(unsigned_b64).expect("base64 unsigned tx");
    // Layout: [sig_count=1][64-byte signature][message...]. Sign the message bytes.
    assert_eq!(tx[0], 1, "expected one signature slot");
    let message = &tx[1 + SIGNATURE_BYTES..];

    let signing_key = ed25519_dalek::SigningKey::from_bytes(&seed);
    // Guard: the keypair's public half must match the derived verifying key.
    assert_eq!(
        signing_key.verifying_key().to_bytes(),
        public,
        "keypair public half mismatch"
    );
    use ed25519_dalek::Signer;
    let signature = signing_key.sign(message);
    tx[1..1 + SIGNATURE_BYTES].copy_from_slice(&signature.to_bytes());
    println!("{}", STANDARD.encode(&tx));
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("parse") => cmd_parse(&args[1]),
        Some("build") => cmd_build(&args[1..]),
        Some("sign") => cmd_sign(&args[1], &args[2]),
        other => {
            eprintln!("unknown subcommand: {other:?}");
            std::process::exit(64);
        }
    }
}
