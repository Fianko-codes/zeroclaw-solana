use nanosol::amount::{format_ui_amount, parse_ui_amount, AmountError};

#[test]
fn parses_exact_ui_amounts_without_floats() {
    assert_eq!(parse_ui_amount("25.01", 6), Ok(25_010_000));
    assert_eq!(parse_ui_amount("0.000001", 6), Ok(1));
    assert_eq!(parse_ui_amount("0001.2", 6), Ok(1_200_000));
    assert_eq!(parse_ui_amount("0", 9), Ok(0));
    assert_eq!(parse_ui_amount(&u64::MAX.to_string(), 0), Ok(u64::MAX));
}

#[test]
fn rejects_ambiguous_invalid_and_overflowing_amounts() {
    for invalid in [
        "", ".5", "1.", "+1", "-1", "1e3", " 1", "1 ", "1.2.3", "one",
    ] {
        assert!(parse_ui_amount(invalid, 6).is_err(), "accepted {invalid:?}");
    }
    assert_eq!(
        parse_ui_amount("1.0000000", 6),
        Err(AmountError::TooManyFractionalDigits {
            provided: 7,
            allowed: 6,
        })
    );
    assert_eq!(
        parse_ui_amount("1.0", 0),
        Err(AmountError::TooManyFractionalDigits {
            provided: 1,
            allowed: 0,
        })
    );
    assert_eq!(
        parse_ui_amount("18446744073709551616", 0),
        Err(AmountError::Overflow)
    );
    assert_eq!(
        parse_ui_amount("0", 20),
        Err(AmountError::UnsupportedDecimals(20))
    );
}

#[test]
fn formats_canonical_ui_strings() {
    assert_eq!(format_ui_amount(0, 6).as_deref(), Ok("0"));
    assert_eq!(format_ui_amount(1, 6).as_deref(), Ok("0.000001"));
    assert_eq!(format_ui_amount(25_010_000, 6).as_deref(), Ok("25.01"));
    assert_eq!(format_ui_amount(1_000_000, 6).as_deref(), Ok("1"));
    assert_eq!(format_ui_amount(u64::MAX, 0), Ok(u64::MAX.to_string()));
}

#[test]
fn raw_format_parse_roundtrips_for_supported_mint_precisions() {
    let values = [
        0,
        1,
        9,
        10,
        127,
        1_000,
        999_999,
        1_000_000,
        u32::MAX as u64,
        u64::MAX,
    ];
    for decimals in [0, 6, 9, 19] {
        for raw in values {
            let ui = format_ui_amount(raw, decimals).expect("format supported amount");
            assert_eq!(
                parse_ui_amount(&ui, decimals),
                Ok(raw),
                "roundtrip failed for raw={raw}, decimals={decimals}, ui={ui}"
            );
        }
    }
}
