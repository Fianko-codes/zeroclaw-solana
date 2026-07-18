use base64::{engine::general_purpose::STANDARD, Engine as _};
use nanosol::{
    pubkey::LEGACY_TOKEN_PROGRAM_ID,
    rpc::{
        get_account_info_request, get_latest_blockhash_request, parse_account_info_response,
        parse_latest_blockhash_response, parse_simulation_response, simulate_transaction_request,
        RpcError, MAX_RPC_RESPONSE_BYTES,
    },
};
use serde_json::{json, Value};

const MINT: &str = "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v";
const BLOCKHASH: &str = "EkSnNWid2cvwEVnVx9aBqawnmiCNiDgp3gUdkDPTKN1N";

fn envelope(id: u64, result: Value) -> String {
    json!({"jsonrpc":"2.0","id":id,"result":result}).to_string()
}

fn account_response(id: u64, data: &[u8]) -> String {
    envelope(
        id,
        json!({
            "context":{"slot":123},
            "value":{
                "data":[STANDARD.encode(data),"base64"],
                "executable":false,
                "lamports":1,
                "owner":LEGACY_TOKEN_PROGRAM_ID.to_string(),
                "space":data.len()
            }
        }),
    )
}

#[test]
fn request_builders_use_only_the_documented_methods_and_options() {
    let latest: Value = serde_json::from_str(&get_latest_blockhash_request(7)).expect("request");
    assert_eq!(latest["method"], "getLatestBlockhash");
    assert_eq!(latest["params"][0]["commitment"], "confirmed");

    let mint = MINT.parse().expect("mint fixture");
    let account: Value =
        serde_json::from_str(&get_account_info_request(8, &mint)).expect("request");
    assert_eq!(account["method"], "getAccountInfo");
    assert_eq!(account["params"][0], MINT);
    assert_eq!(account["params"][1]["encoding"], "base64");

    let simulation: Value =
        serde_json::from_str(&simulate_transaction_request(9, "AQID")).expect("request");
    assert_eq!(simulation["method"], "simulateTransaction");
    assert_eq!(simulation["params"][0], "AQID");
    assert_eq!(simulation["params"][1]["encoding"], "base64");
    assert_eq!(simulation["params"][1]["sigVerify"], false);
    assert_eq!(simulation["params"][1]["replaceRecentBlockhash"], true);
}

#[test]
fn latest_blockhash_response_is_strict() {
    let valid = envelope(
        1,
        json!({
            "context":{"slot":123},
            "value":{"blockhash":BLOCKHASH,"lastValidBlockHeight":3090}
        }),
    );
    let parsed = parse_latest_blockhash_response(&valid, 1).expect("valid response");
    assert_eq!(parsed.last_valid_block_height, 3090);
    assert_eq!(bs58::encode(parsed.blockhash).into_string(), BLOCKHASH);

    for malformed in [
        envelope(
            1,
            json!({"value":{"blockhash":"bad","lastValidBlockHeight":1}}),
        ),
        envelope(
            1,
            json!({"value":{"blockhash":BLOCKHASH,"lastValidBlockHeight":-1}}),
        ),
        envelope(
            1,
            json!({"value":{"blockhash":BLOCKHASH,"lastValidBlockHeight":"3090"}}),
        ),
        envelope(1, json!({"context":{"slot":1}})),
    ] {
        assert!(parse_latest_blockhash_response(&malformed, 1).is_err());
    }
}

#[test]
fn account_response_accepts_base64_and_rejects_null_or_invalid_data() {
    let data = [7_u8; 82];
    let account = parse_account_info_response(&account_response(2, &data), 2).expect("account");
    assert_eq!(account.data, data);
    assert_eq!(account.owner, LEGACY_TOKEN_PROGRAM_ID);
    assert!(!account.executable);

    let null = envelope(2, json!({"context":{"slot":1},"value":null}));
    assert_eq!(
        parse_account_info_response(&null, 2),
        Err(RpcError::AccountNotFound)
    );

    let invalid_base64 = envelope(
        2,
        json!({"context":{"slot":1},"value":{
            "data":["%%%","base64"],"executable":false,"owner":LEGACY_TOKEN_PROGRAM_ID.to_string()
        }}),
    );
    assert_eq!(
        parse_account_info_response(&invalid_base64, 2),
        Err(RpcError::InvalidAccountData)
    );

    let wrong_encoding = envelope(
        2,
        json!({"context":{"slot":1},"value":{
            "data":["111","base58"],"executable":false,"owner":LEGACY_TOKEN_PROGRAM_ID.to_string()
        }}),
    );
    assert_eq!(
        parse_account_info_response(&wrong_encoding, 2),
        Err(RpcError::InvalidAccountDataEncoding)
    );
}

#[test]
fn simulation_response_ignores_logs_and_shapes_only_the_error_category() {
    let success = envelope(
        3,
        json!({"context":{"slot":1},"value":{"err":null,"logs":["secret log"]}}),
    );
    assert!(parse_simulation_response(&success, 3)
        .expect("simulation")
        .is_success());

    let failure = envelope(
        3,
        json!({"context":{"slot":1},"value":{
            "err":{"InstructionError":[1,"Custom"]},
            "logs":["x".repeat(10_000)]
        }}),
    );
    let parsed = parse_simulation_response(&failure, 3).expect("simulation failure envelope");
    assert_eq!(parsed.error.as_deref(), Some("InstructionError"));
    assert!(!parsed.error.expect("reason").contains("Custom"));

    let malformed = envelope(3, json!({"context":{"slot":1},"value":{"logs":[]}}));
    assert_eq!(
        parse_simulation_response(&malformed, 3),
        Err(RpcError::InvalidSimulationResult)
    );
}

#[test]
fn every_parser_rejects_malformed_envelopes_errors_ids_and_oversize() {
    let json_rpc_error = json!({
        "jsonrpc":"2.0","id":1,"error":{"code":-32000,"message":"provider detail"}
    })
    .to_string();
    assert_eq!(
        parse_latest_blockhash_response(&json_rpc_error, 1),
        Err(RpcError::Remote { code: -32000 })
    );

    let mismatched = envelope(99, json!({}));
    assert!(matches!(
        parse_latest_blockhash_response(&mismatched, 1),
        Err(RpcError::MismatchedId { .. })
    ));
    assert!(matches!(
        parse_account_info_response(&mismatched, 2),
        Err(RpcError::MismatchedId { .. })
    ));
    assert!(matches!(
        parse_simulation_response(&mismatched, 3),
        Err(RpcError::MismatchedId { .. })
    ));

    for malformed in [
        "not json".to_string(),
        "[]".to_string(),
        json!({"jsonrpc":"1.0","id":1,"result":{}}).to_string(),
        json!({"jsonrpc":"2.0","id":1}).to_string(),
        json!({"jsonrpc":"2.0","id":1,"result":{},"error":{}}).to_string(),
    ] {
        assert!(parse_latest_blockhash_response(&malformed, 1).is_err());
    }

    let oversized = format!("{{\"padding\":\"{}\"}}", "x".repeat(MAX_RPC_RESPONSE_BYTES));
    assert!(matches!(
        parse_latest_blockhash_response(&oversized, 1),
        Err(RpcError::ResponseTooLarge(_))
    ));
    assert!(matches!(
        parse_account_info_response(&oversized, 2),
        Err(RpcError::ResponseTooLarge(_))
    ));
    assert!(matches!(
        parse_simulation_response(&oversized, 3),
        Err(RpcError::ResponseTooLarge(_))
    ));
}
