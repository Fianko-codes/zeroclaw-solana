use std::str::FromStr;

use nanosol::{
    compact_u16,
    pubkey::{
        create_program_address, derive_associated_token_address, find_program_address, PdaError,
        Pubkey, ASSOCIATED_TOKEN_PROGRAM_ID, COMPUTE_BUDGET_PROGRAM_ID, LEGACY_TOKEN_PROGRAM_ID,
        MAX_SEEDS, MEMO_V3_PROGRAM_ID, SYSTEM_PROGRAM_ID, TOKEN_2022_PROGRAM_ID,
    },
};
use solana_pubkey::Pubkey as OfficialPubkey;
use solana_short_vec::{decode_shortu16_len, ShortU16};
use spl_associated_token_account_interface::address::get_associated_token_address_with_program_id;

const WALLET: &str = "7xKXtg2CW87d97TXJSDpbD5jBkheTqA83TZRuJosgAsU";
const MINT: &str = "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v";

fn ours(value: &str) -> Pubkey {
    value.parse().expect("valid nanosol fixture")
}

fn official(key: Pubkey) -> OfficialPubkey {
    OfficialPubkey::from(key.to_bytes())
}

#[test]
fn public_key_parser_and_program_constants_match_official_types() {
    for text in [
        WALLET,
        MINT,
        "11111111111111111111111111111111",
        "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA",
        "TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb",
    ] {
        let actual = ours(text);
        let expected = OfficialPubkey::from_str(text).expect("official fixture");
        assert_eq!(actual.to_bytes(), expected.to_bytes());
        assert_eq!(actual.to_string(), text);
    }

    assert_eq!(
        SYSTEM_PROGRAM_ID.to_string(),
        "11111111111111111111111111111111"
    );
    assert_eq!(official(LEGACY_TOKEN_PROGRAM_ID), spl_token_interface::id());
    assert_eq!(
        official(TOKEN_2022_PROGRAM_ID),
        spl_token_2022_interface::id()
    );
    assert_eq!(
        official(ASSOCIATED_TOKEN_PROGRAM_ID),
        spl_associated_token_account_interface::program::id()
    );
    assert_eq!(official(MEMO_V3_PROGRAM_ID), spl_memo_interface::v3::id());
    assert_eq!(
        official(COMPUTE_BUDGET_PROGRAM_ID),
        solana_compute_budget_interface::id()
    );

    for invalid in ["", "1", "not!base58"] {
        assert!(Pubkey::from_str(invalid).is_err());
        assert!(OfficialPubkey::from_str(invalid).is_err());
    }
}

#[test]
fn pda_and_ata_results_match_official_oracles() {
    let program = SYSTEM_PROGRAM_ID;
    let wallet = ours(WALLET);
    let mint = ours(MINT);
    let seed_sets: Vec<Vec<&[u8]>> = vec![
        vec![b"vault", wallet.as_bytes()],
        vec![b"metadata", mint.as_bytes(), b"v1"],
        vec![b"", b"deterministic", &[0, 1, 2, 3]],
    ];
    for seeds in seed_sets {
        let actual = find_program_address(&seeds, &program).expect("nanosol PDA");
        let expected = OfficialPubkey::find_program_address(&seeds, &official(program));
        assert_eq!(actual.0.to_bytes(), expected.0.to_bytes());
        assert_eq!(actual.1, expected.1);

        let bump = [actual.1];
        let mut explicit = seeds;
        explicit.push(&bump);
        assert_eq!(
            create_program_address(&explicit, &program)
                .expect("explicit nanosol PDA")
                .to_bytes(),
            OfficialPubkey::create_program_address(&explicit, &official(program))
                .expect("explicit official PDA")
                .to_bytes()
        );
    }

    for token_program in [LEGACY_TOKEN_PROGRAM_ID, TOKEN_2022_PROGRAM_ID] {
        let (actual, bump) =
            derive_associated_token_address(&wallet, &mint, &token_program).expect("nanosol ATA");
        let expected = get_associated_token_address_with_program_id(
            &official(wallet),
            &official(mint),
            &official(token_program),
        );
        assert_eq!(actual.to_bytes(), expected.to_bytes());
        let (_, expected_bump) = OfficialPubkey::find_program_address(
            &[wallet.as_bytes(), token_program.as_bytes(), mint.as_bytes()],
            &official(ASSOCIATED_TOKEN_PROGRAM_ID),
        );
        assert_eq!(bump, expected_bump);
    }
}

#[test]
fn pda_rejections_match_official_behavior() {
    let oversized = [7_u8; 33];
    assert!(matches!(
        create_program_address(&[&oversized], &SYSTEM_PROGRAM_ID),
        Err(PdaError::SeedTooLong { .. })
    ));
    assert!(
        OfficialPubkey::create_program_address(&[&oversized], &official(SYSTEM_PROGRAM_ID))
            .is_err()
    );

    let unit = [1_u8];
    let too_many: Vec<&[u8]> = (0..=MAX_SEEDS).map(|_| unit.as_slice()).collect();
    assert!(matches!(
        create_program_address(&too_many, &SYSTEM_PROGRAM_ID),
        Err(PdaError::TooManySeeds { .. })
    ));

    let on_curve_seed = (0_u16..=u16::MAX)
        .map(u16::to_le_bytes)
        .find(|seed| create_program_address(&[seed], &SYSTEM_PROGRAM_ID) == Err(PdaError::OnCurve))
        .expect("deterministic on-curve seed");
    assert!(OfficialPubkey::create_program_address(
        &[&on_curve_seed],
        &official(SYSTEM_PROGRAM_ID)
    )
    .is_err());
}

#[test]
fn compact_u16_matches_official_boundaries_and_rejections() {
    for value in [0_u16, 1, 127, 128, 255, 16_383, 16_384, u16::MAX] {
        let encoded = compact_u16::encode(value);
        assert_eq!(
            encoded,
            bincode::serialize(&ShortU16(value)).expect("official compact encoding")
        );
        assert_eq!(compact_u16::decode(&encoded), Ok((value, encoded.len())));
        assert_eq!(
            decode_shortu16_len(&encoded),
            Ok((usize::from(value), encoded.len()))
        );
    }

    for malformed in [
        &[][..],
        &[0x80][..],
        &[0x80, 0][..],
        &[0xff, 0xff, 4][..],
        &[0xff, 0xff, 0x83][..],
    ] {
        assert!(compact_u16::decode(malformed).is_err());
        assert!(decode_shortu16_len(malformed).is_err());
    }
    assert!(compact_u16::encode_len(usize::from(u16::MAX) + 1).is_err());
}

#[test]
fn derivations_are_repeatable() {
    let wallet = ours(WALLET);
    let mint = ours(MINT);
    let baseline =
        derive_associated_token_address(&wallet, &mint, &TOKEN_2022_PROGRAM_ID).expect("baseline");
    for _ in 0..64 {
        assert_eq!(
            derive_associated_token_address(&wallet, &mint, &TOKEN_2022_PROGRAM_ID),
            Ok(baseline)
        );
    }
}
