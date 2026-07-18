use std::str::FromStr;

use solana_pda_oracle_spike::{
    compact_u16,
    pubkey::{
        create_program_address, derive_associated_token_address, find_program_address, PdaError,
        PublicKey, LEGACY_TOKEN_PROGRAM_ID, TOKEN_2022_PROGRAM_ID,
    },
};
use solana_pubkey::Pubkey as OfficialPubkey;
use solana_short_vec::{decode_shortu16_len, ShortU16};
use spl_associated_token_account_interface::address::get_associated_token_address_with_program_id;

const SYSTEM_PROGRAM: &str = "11111111111111111111111111111111";
const WALLET: &str = "7xKXtg2CW87d97TXJSDpbD5jBkheTqA83TZRuJosgAsU";
const USDC_MINT: &str = "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v";

fn ours(value: &str) -> PublicKey {
    value.parse().expect("valid spike public key")
}

fn official(key: PublicKey) -> OfficialPubkey {
    OfficialPubkey::from(key.to_bytes())
}

#[test]
fn public_key_parser_matches_official_bytes() {
    for value in [SYSTEM_PROGRAM, WALLET, USDC_MINT, LEGACY_TOKEN_PROGRAM_ID] {
        let parsed = ours(value);
        let oracle = OfficialPubkey::from_str(value).expect("official parser accepts fixture");
        assert_eq!(parsed.to_bytes(), oracle.to_bytes());
        assert_eq!(parsed.to_string(), value);
    }

    for invalid in ["not!base58", "1", ""] {
        assert!(PublicKey::from_str(invalid).is_err());
        assert!(OfficialPubkey::from_str(invalid).is_err());
    }
}

#[test]
fn multiple_pda_seed_combinations_match_official_oracle() {
    let program = ours(SYSTEM_PROGRAM);
    let wallet = ours(WALLET);
    let mint = ours(USDC_MINT);
    let seed_sets: Vec<(&str, Vec<&[u8]>)> = vec![
        ("vault", vec![b"vault", wallet.as_bytes()]),
        ("metadata", vec![b"metadata", mint.as_bytes(), b"v1"]),
        ("mixed", vec![b"", b"deterministic", &[0, 1, 2, 3]]),
    ];

    for (label, seeds) in seed_sets {
        let (actual, bump) = find_program_address(&seeds, &program).expect("spike PDA");
        let (expected, expected_bump) =
            OfficialPubkey::find_program_address(&seeds, &official(program));
        assert_eq!(actual.to_bytes(), expected.to_bytes());
        assert_eq!(bump, expected_bump);

        let bump_seed = [bump];
        let mut explicit = seeds.clone();
        explicit.push(&bump_seed);
        let recreated = create_program_address(&explicit, &program).expect("explicit bump PDA");
        let official_recreated =
            OfficialPubkey::create_program_address(&explicit, &official(program))
                .expect("official explicit bump PDA");
        assert_eq!(recreated.to_bytes(), official_recreated.to_bytes());
        eprintln!("PDA {label} = {actual} (bump {bump})");
    }
}

#[test]
fn bump_search_skips_on_curve_candidates_like_official_oracle() {
    let program = ours(SYSTEM_PROGRAM);
    let (seed, expected_address, expected_bump) = (0_u16..=u16::MAX)
        .find_map(|candidate| {
            let seed = candidate.to_le_bytes();
            let (address, bump) =
                OfficialPubkey::try_find_program_address(&[&seed], &official(program))?;
            (bump < u8::MAX).then_some((seed, address, bump))
        })
        .expect("deterministic fixture with at least one rejected bump");

    let (actual, bump) = find_program_address(&[&seed], &program).expect("spike bump search");
    assert_eq!(actual.to_bytes(), expected_address.to_bytes());
    assert_eq!(bump, expected_bump);
    eprintln!("bump-search seed {seed:02x?} = {actual} (bump {bump})");

    for rejected_bump in (bump + 1)..=u8::MAX {
        let bump_seed = [rejected_bump];
        assert_eq!(
            create_program_address(&[&seed, &bump_seed], &program),
            Err(PdaError::OnCurve)
        );
        assert!(
            OfficialPubkey::create_program_address(&[&seed, &bump_seed], &official(program))
                .is_err()
        );
    }
}

#[test]
fn invalid_and_excessive_seeds_match_official_rejection() {
    let program = ours(SYSTEM_PROGRAM);
    let oversized = [7_u8; 33];
    assert!(matches!(
        create_program_address(&[&oversized], &program),
        Err(PdaError::SeedTooLong {
            index: 0,
            length: 33
        })
    ));
    assert!(OfficialPubkey::create_program_address(&[&oversized], &official(program)).is_err());

    let unit = [1_u8];
    let seventeen: Vec<&[u8]> = (0..17).map(|_| unit.as_slice()).collect();
    assert!(matches!(
        create_program_address(&seventeen, &program),
        Err(PdaError::TooManySeeds {
            supplied: 17,
            maximum: 16
        })
    ));
    assert!(OfficialPubkey::create_program_address(&seventeen, &official(program)).is_err());

    let sixteen: Vec<&[u8]> = (0..16).map(|_| unit.as_slice()).collect();
    assert!(matches!(
        find_program_address(&sixteen, &program),
        Err(PdaError::TooManySeeds {
            supplied: 16,
            maximum: 15
        })
    ));
    assert!(OfficialPubkey::try_find_program_address(&sixteen, &official(program)).is_none());
}

#[test]
fn create_program_address_rejects_an_on_curve_digest() {
    let program = ours(SYSTEM_PROGRAM);
    let seed = (0_u16..=u16::MAX)
        .map(u16::to_le_bytes)
        .find(|seed| create_program_address(&[seed], &program) == Err(PdaError::OnCurve))
        .expect("deterministic on-curve seed fixture");

    assert_eq!(
        create_program_address(&[&seed], &program),
        Err(PdaError::OnCurve)
    );
    assert!(OfficialPubkey::create_program_address(&[&seed], &official(program)).is_err());
    eprintln!("on-curve rejection seed = {seed:02x?}");
}

#[test]
fn legacy_and_token_2022_atas_match_official_oracle() {
    let wallet = ours(WALLET);
    let mint = ours(USDC_MINT);

    for token_program_text in [LEGACY_TOKEN_PROGRAM_ID, TOKEN_2022_PROGRAM_ID] {
        let token_program = ours(token_program_text);
        let (actual, bump) = derive_associated_token_address(&wallet, &mint, &token_program)
            .expect("spike ATA derivation");
        let expected = get_associated_token_address_with_program_id(
            &official(wallet),
            &official(mint),
            &official(token_program),
        );
        assert_eq!(actual.to_bytes(), expected.to_bytes());

        let (official_with_bump, expected_bump) = OfficialPubkey::find_program_address(
            &[wallet.as_bytes(), token_program.as_bytes(), mint.as_bytes()],
            &OfficialPubkey::from_str(solana_pda_oracle_spike::pubkey::ASSOCIATED_TOKEN_PROGRAM_ID)
                .expect("official ATA program ID"),
        );
        assert_eq!(actual.to_bytes(), official_with_bump.to_bytes());
        assert_eq!(bump, expected_bump);

        eprintln!("{token_program_text} ATA = {actual} (bump {bump})");
    }
}

#[test]
fn compact_u16_boundaries_match_official_oracle() {
    for value in [0_u16, 1, 127, 128, 255, 16_383, 16_384, u16::MAX] {
        let actual = compact_u16::encode(value);
        let expected = bincode::serialize(&ShortU16(value)).expect("official encoding");
        assert_eq!(actual, expected, "encoding mismatch for {value}");
        assert_eq!(compact_u16::decode(&actual), Ok((value, actual.len())));
        assert_eq!(
            decode_shortu16_len(&actual),
            Ok((usize::from(value), actual.len()))
        );
        eprintln!("compact-u16 {value} = {actual:02x?}");
    }
}

#[test]
fn malformed_compact_u16_matches_official_rejection() {
    let malformed: &[&[u8]] = &[
        &[],
        &[0x80],
        &[0x80, 0x00],
        &[0x81, 0x00],
        &[0xff, 0xff, 0x04],
        &[0xff, 0xff, 0x83],
    ];

    for bytes in malformed {
        assert!(compact_u16::decode(bytes).is_err(), "accepted {bytes:02x?}");
        assert!(
            decode_shortu16_len(bytes).is_err(),
            "official oracle accepted {bytes:02x?}"
        );
    }

    assert_eq!(compact_u16::decode(&[0x7f, 0xff]), Ok((127, 1)));
    assert_eq!(decode_shortu16_len(&[0x7f, 0xff]), Ok((127, 1)));
}

#[test]
fn pda_ata_and_compact_results_repeat_deterministically() {
    let program = ours(SYSTEM_PROGRAM);
    let wallet = ours(WALLET);
    let mint = ours(USDC_MINT);
    let token_program = ours(TOKEN_2022_PROGRAM_ID);
    let pda =
        find_program_address(&[b"repeat", wallet.as_bytes()], &program).expect("baseline PDA");
    let ata =
        derive_associated_token_address(&wallet, &mint, &token_program).expect("baseline ATA");
    let compact = compact_u16::encode(u16::MAX);

    for _ in 0..64 {
        assert_eq!(
            find_program_address(&[b"repeat", wallet.as_bytes()], &program),
            Ok(pda)
        );
        assert_eq!(
            derive_associated_token_address(&wallet, &mint, &token_program),
            Ok(ata)
        );
        assert_eq!(compact_u16::encode(u16::MAX), compact);
    }
}
