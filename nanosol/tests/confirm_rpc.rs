//! Strictness tests for the payment-confirmation RPC surface: signature lists,
//! settled transactions, and token-balance metadata.
//!
//! Every parser is total. A malformed, hostile, or oversized response must be a
//! typed error, never a partially trusted value and never a panic.

use base64::{engine::general_purpose::STANDARD, Engine as _};
use nanosol::{
    pubkey::{Pubkey, LEGACY_TOKEN_PROGRAM_ID},
    rpc::{
        get_signatures_for_address_request, get_transaction_request,
        parse_signatures_for_address_response, parse_transaction_response, CommitmentLevel,
        RpcError, MAX_RPC_RESPONSE_BYTES, MAX_SIGNATURE_RESULTS, MAX_TOKEN_BALANCES,
        MAX_TRANSACTION_RESPONSE_BYTES,
    },
    signature::{ParseSignatureError, Signature},
};
use serde_json::{json, Value};

const REFERENCE: &str = "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v";
const MINT: &str = "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v";
const OWNER: &str = "FnHyam9w4NZoWR6mKN1CuGBritdsEWZQa4Z4oawLZGxa";

fn signature_text(byte: u8) -> String {
    bs58::encode([byte; 64]).into_string()
}

fn envelope(id: u64, result: Value) -> String {
    json!({"jsonrpc":"2.0","id":id,"result":result}).to_string()
}

fn signature_entry(byte: u8, status: Value, err: Value) -> Value {
    json!({
        "signature": signature_text(byte),
        "slot": 300,
        "err": err,
        "memo": null,
        "blockTime": 1_700_000_000_u64,
        "confirmationStatus": status
    })
}

fn token_balance(index: usize, amount: &str) -> Value {
    json!({
        "accountIndex": index,
        "mint": MINT,
        "owner": OWNER,
        "programId": LEGACY_TOKEN_PROGRAM_ID.to_string(),
        "uiTokenAmount": {
            "amount": amount,
            "decimals": 6,
            "uiAmount": 1.5,
            "uiAmountString": "1.5"
        }
    })
}

fn transaction_result(pre: Value, post: Value, err: Value) -> Value {
    json!({
        "slot": 300,
        "blockTime": 1_700_000_000_u64,
        "transaction": [STANDARD.encode([1, 2, 3]), "base64"],
        "meta": {
            "err": err,
            "fee": 5000,
            "logMessages": ["Program log: ignored"],
            "preTokenBalances": pre,
            "postTokenBalances": post
        }
    })
}

#[test]
fn commitment_levels_order_from_weakest_to_strongest_and_parse_exactly() {
    assert!(CommitmentLevel::Processed < CommitmentLevel::Confirmed);
    assert!(CommitmentLevel::Confirmed < CommitmentLevel::Finalized);
    assert!(CommitmentLevel::Finalized.satisfies(CommitmentLevel::Confirmed));
    assert!(!CommitmentLevel::Confirmed.satisfies(CommitmentLevel::Finalized));
    assert!(CommitmentLevel::Confirmed.satisfies(CommitmentLevel::Confirmed));

    for (text, level) in [
        ("processed", CommitmentLevel::Processed),
        ("confirmed", CommitmentLevel::Confirmed),
        ("finalized", CommitmentLevel::Finalized),
    ] {
        assert_eq!(CommitmentLevel::parse_exact(text), Some(level));
        assert_eq!(level.as_str(), text);
        assert_eq!(level.to_string(), text);
    }
    for rejected in ["", "Finalized", "final", "finalized ", "root", "max"] {
        assert_eq!(CommitmentLevel::parse_exact(rejected), None);
    }
}

#[test]
fn signatures_are_exactly_sixty_four_base58_bytes() {
    let text = signature_text(3);
    let signature: Signature = text.parse().expect("signature");
    assert_eq!(signature.to_bytes(), [3; 64]);
    assert_eq!(signature.to_string(), text);
    assert!(!signature.is_zero());
    assert!(Signature::new([0; 64]).is_zero());

    assert_eq!(
        REFERENCE.parse::<Signature>(),
        Err(ParseSignatureError::InvalidLength(32))
    );
    assert!(matches!(
        "not base58!".parse::<Signature>(),
        Err(ParseSignatureError::InvalidBase58(_))
    ));
    assert!(matches!(
        "1".repeat(200).parse::<Signature>(),
        Err(ParseSignatureError::InvalidLength(200))
    ));
}

#[test]
fn request_builders_use_only_documented_methods_and_bounded_options() {
    let reference: Pubkey = REFERENCE.parse().expect("reference");
    let request: Value = serde_json::from_str(&get_signatures_for_address_request(
        1,
        &reference,
        10,
        CommitmentLevel::Finalized,
    ))
    .expect("request");
    assert_eq!(request["method"], "getSignaturesForAddress");
    assert_eq!(request["params"][0], REFERENCE);
    assert_eq!(request["params"][1]["limit"], 10);
    assert_eq!(request["params"][1]["commitment"], "finalized");

    // The RPC accepts 1..=1000; a caller value outside that range is clamped so
    // the endpoint never rejects the whole request.
    for (requested, expected) in [(0_u16, 1), (1, 1), (1_000, 1_000), (u16::MAX, 1_000)] {
        let request: Value = serde_json::from_str(&get_signatures_for_address_request(
            1,
            &reference,
            requested,
            CommitmentLevel::Confirmed,
        ))
        .expect("request");
        assert_eq!(request["params"][1]["limit"], expected);
    }

    let signature: Signature = signature_text(4).parse().expect("signature");
    let request: Value = serde_json::from_str(&get_transaction_request(
        2,
        &signature,
        CommitmentLevel::Finalized,
    ))
    .expect("request");
    assert_eq!(request["method"], "getTransaction");
    assert_eq!(request["params"][0], signature.to_string());
    assert_eq!(request["params"][1]["encoding"], "base64");
    assert_eq!(request["params"][1]["commitment"], "finalized");
    assert_eq!(request["params"][1]["maxSupportedTransactionVersion"], 0);
    assert_eq!(request["params"][1].as_object().expect("options").len(), 3);
}

#[test]
fn signature_lists_are_parsed_strictly_and_preserve_endpoint_order() {
    let body = envelope(
        1,
        json!([
            signature_entry(1, json!("finalized"), Value::Null),
            signature_entry(
                2,
                json!("confirmed"),
                json!({"InstructionError": [0, "Custom"]})
            ),
            signature_entry(3, Value::Null, Value::Null),
        ]),
    );
    let records = parse_signatures_for_address_response(&body, 1).expect("signature list");
    assert_eq!(records.len(), 3);
    assert_eq!(records[0].signature.to_bytes(), [1; 64]);
    assert_eq!(records[0].slot, 300);
    assert!(!records[0].failed);
    assert_eq!(
        records[0].confirmation_status,
        Some(CommitmentLevel::Finalized)
    );
    assert!(records[1].failed);
    assert_eq!(
        records[1].confirmation_status,
        Some(CommitmentLevel::Confirmed)
    );
    // A missing status is reported as unknown, never inferred.
    assert_eq!(records[2].confirmation_status, None);

    assert_eq!(
        parse_signatures_for_address_response(&envelope(1, json!([])), 1),
        Ok(Vec::new())
    );
}

#[test]
fn malformed_signature_entries_are_typed_errors() {
    let cases = [
        json!({"slot": 1, "err": null}),
        json!({"signature": REFERENCE, "slot": 1, "err": null}),
        json!({"signature": signature_text(1), "err": null}),
        json!({"signature": signature_text(1), "slot": -1, "err": null}),
        json!({"signature": signature_text(1), "slot": 1}),
        json!({"signature": signature_text(1), "slot": 1, "err": null, "confirmationStatus": "rooted"}),
        json!({"signature": signature_text(1), "slot": 1, "err": null, "confirmationStatus": 3}),
    ];
    for case in cases {
        assert!(
            matches!(
                parse_signatures_for_address_response(&envelope(1, json!([case.clone()])), 1),
                Err(RpcError::InvalidSignatureRecord(_))
            ),
            "accepted malformed entry {case}"
        );
    }

    for body in [
        envelope(1, json!({"signatures": []})),
        envelope(1, json!("finalized")),
        envelope(1, Value::Null),
    ] {
        assert_eq!(
            parse_signatures_for_address_response(&body, 1),
            Err(RpcError::InvalidSignatureList)
        );
    }

    // A list longer than the RPC maximum is refused, and no entries are
    // returned. The byte ceiling binds first in practice — 1001 signature
    // entries cannot fit in 64 KiB — so either refusal is correct here; the
    // entry-count bound exists so the parser does not depend on that ceiling.
    let too_many: Vec<Value> = (0..=MAX_SIGNATURE_RESULTS)
        .map(|_| signature_entry(1, json!("finalized"), Value::Null))
        .collect();
    assert!(matches!(
        parse_signatures_for_address_response(&envelope(1, json!(too_many)), 1),
        Err(RpcError::InvalidSignatureList | RpcError::ResponseTooLarge(_))
    ));
}

#[test]
fn settled_transactions_expose_only_bytes_slot_error_and_balances() {
    let body = envelope(
        5,
        transaction_result(
            json!([token_balance(4, "500000")]),
            json!([token_balance(4, "2000000")]),
            Value::Null,
        ),
    );
    let record = parse_transaction_response(&body, 5).expect("transaction");
    assert_eq!(record.slot, 300);
    assert_eq!(record.transaction, vec![1, 2, 3]);
    assert!(!record.failed);
    assert_eq!(record.pre_token_balances.len(), 1);
    assert_eq!(record.pre_token_balances[0].raw_amount, 500_000);
    assert_eq!(record.pre_token_balances[0].account_index, 4);
    assert_eq!(record.pre_token_balances[0].decimals, 6);
    assert_eq!(
        record.pre_token_balances[0].owner,
        Some(OWNER.parse().expect("owner"))
    );
    assert_eq!(
        record.pre_token_balances[0].program_id,
        Some(LEGACY_TOKEN_PROGRAM_ID)
    );
    assert_eq!(record.post_token_balances[0].raw_amount, 2_000_000);

    let failed = envelope(
        5,
        transaction_result(
            json!([]),
            json!([]),
            json!({"InstructionError": [1, "Custom"]}),
        ),
    );
    assert!(
        parse_transaction_response(&failed, 5)
            .expect("failed record")
            .failed
    );

    // Absent balance arrays are empty, so a delta check against them fails
    // closed rather than being skipped.
    let without_balances = envelope(
        5,
        json!({
            "slot": 1,
            "transaction": [STANDARD.encode([7]), "base64"],
            "meta": {"err": null}
        }),
    );
    let record = parse_transaction_response(&without_balances, 5).expect("record");
    assert!(record.pre_token_balances.is_empty());
    assert!(record.post_token_balances.is_empty());
}

#[test]
fn missing_or_malformed_transactions_are_typed_errors() {
    assert_eq!(
        parse_transaction_response(&envelope(5, Value::Null), 5),
        Err(RpcError::TransactionNotFound)
    );

    let cases = [
        json!({"transaction": [STANDARD.encode([1]), "base64"], "meta": {"err": null}}),
        json!({"slot": 1, "meta": {"err": null}}),
        json!({"slot": 1, "transaction": [STANDARD.encode([1]), "base58"], "meta": {"err": null}}),
        json!({"slot": 1, "transaction": ["%%%", "base64"], "meta": {"err": null}}),
        json!({"slot": 1, "transaction": STANDARD.encode([1]), "meta": {"err": null}}),
        json!({"slot": 1, "transaction": [STANDARD.encode([1]), "base64"]}),
        json!({"slot": 1, "transaction": [STANDARD.encode([1]), "base64"], "meta": {}}),
        json!({"slot": 1, "transaction": [STANDARD.encode([1]), "base64"], "meta": "ok"}),
    ];
    for case in cases {
        assert!(
            matches!(
                parse_transaction_response(&envelope(5, case.clone()), 5),
                Err(RpcError::InvalidTransactionRecord(_))
            ),
            "accepted malformed transaction {case}"
        );
    }
}

#[test]
fn malformed_token_balances_are_typed_errors() {
    let cases = [
        json!({"mint": MINT, "uiTokenAmount": {"amount": "1", "decimals": 6}}),
        json!({"accountIndex": 1, "uiTokenAmount": {"amount": "1", "decimals": 6}}),
        json!({"accountIndex": 1, "mint": "not-a-key", "uiTokenAmount": {"amount": "1", "decimals": 6}}),
        json!({"accountIndex": 1, "mint": MINT}),
        json!({"accountIndex": 1, "mint": MINT, "uiTokenAmount": {"decimals": 6}}),
        // A raw amount must be a decimal string: a JSON number would lose
        // precision above 2^53 and is refused rather than coerced.
        json!({"accountIndex": 1, "mint": MINT, "uiTokenAmount": {"amount": 1, "decimals": 6}}),
        json!({"accountIndex": 1, "mint": MINT, "uiTokenAmount": {"amount": "-1", "decimals": 6}}),
        json!({"accountIndex": 1, "mint": MINT, "uiTokenAmount": {"amount": "1.5", "decimals": 6}}),
        json!({"accountIndex": 1, "mint": MINT, "uiTokenAmount": {"amount": "18446744073709551616", "decimals": 6}}),
        json!({"accountIndex": 1, "mint": MINT, "uiTokenAmount": {"amount": "1", "decimals": 300}}),
        json!({"accountIndex": 1, "mint": MINT, "owner": 7, "uiTokenAmount": {"amount": "1", "decimals": 6}}),
        json!({"accountIndex": -1, "mint": MINT, "uiTokenAmount": {"amount": "1", "decimals": 6}}),
    ];
    for case in cases {
        let body = envelope(
            5,
            transaction_result(json!([case.clone()]), json!([]), Value::Null),
        );
        assert!(
            matches!(
                parse_transaction_response(&body, 5),
                Err(RpcError::InvalidTokenBalance(_))
            ),
            "accepted malformed balance {case}"
        );
    }

    let too_many: Vec<Value> = (0..=MAX_TOKEN_BALANCES)
        .map(|index| token_balance(index, "1"))
        .collect();
    let body = envelope(
        5,
        transaction_result(json!(too_many), json!([]), Value::Null),
    );
    assert_eq!(
        parse_transaction_response(&body, 5),
        Err(RpcError::InvalidTokenBalance("too many token balances"))
    );
}

#[test]
fn envelope_rules_and_size_ceilings_apply_to_the_new_parsers() {
    let remote_error = json!({
        "jsonrpc":"2.0","id":1,"error":{"code":-32011,"message":"provider detail"}
    })
    .to_string();
    assert_eq!(
        parse_signatures_for_address_response(&remote_error, 1),
        Err(RpcError::Remote { code: -32011 })
    );
    assert_eq!(
        parse_transaction_response(&remote_error, 1),
        Err(RpcError::Remote { code: -32011 })
    );

    let mismatched = envelope(99, json!([]));
    assert!(matches!(
        parse_signatures_for_address_response(&mismatched, 1),
        Err(RpcError::MismatchedId { .. })
    ));
    assert!(matches!(
        parse_transaction_response(&mismatched, 1),
        Err(RpcError::MismatchedId { .. })
    ));

    for malformed in [
        "not json".to_string(),
        "[]".to_string(),
        json!({"jsonrpc":"1.0","id":1,"result":[]}).to_string(),
        json!({"jsonrpc":"2.0","id":1}).to_string(),
        json!({"jsonrpc":"2.0","id":1,"result":[],"error":{}}).to_string(),
    ] {
        assert!(parse_signatures_for_address_response(&malformed, 1).is_err());
        assert!(parse_transaction_response(&malformed, 1).is_err());
    }

    // Signature lists keep the default ceiling; a transaction carries its own,
    // larger one because it contains the transaction plus its metadata.
    let over_default = format!("{{\"padding\":\"{}\"}}", "x".repeat(MAX_RPC_RESPONSE_BYTES));
    assert!(matches!(
        parse_signatures_for_address_response(&over_default, 1),
        Err(RpcError::ResponseTooLarge(_))
    ));
    assert!(matches!(
        parse_transaction_response(&over_default, 1),
        Err(RpcError::InvalidJson | RpcError::InvalidEnvelope(_))
    ));

    let over_transaction_limit = format!(
        "{{\"padding\":\"{}\"}}",
        "x".repeat(MAX_TRANSACTION_RESPONSE_BYTES)
    );
    assert!(matches!(
        parse_transaction_response(&over_transaction_limit, 1),
        Err(RpcError::ResponseTooLarge(_))
    ));
}
