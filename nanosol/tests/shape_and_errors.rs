use nanosol::{
    amount::AmountError,
    pubkey::Pubkey,
    shape::{elide_address, quote_untrusted, single_line},
    Error,
};

#[test]
fn addresses_are_elided_deterministically() {
    let address: Pubkey = "7xKXtg2CW87d97TXJSDpbD5jBkheTqA83TZRuJosgAsU"
        .parse()
        .expect("fixture");
    assert_eq!(elide_address(&address), "7xKX…gAsU");
}

#[test]
fn untrusted_text_is_single_line_quoted_and_bounded() {
    assert_eq!(single_line("pay\nnow\tplease", 100), "pay now please");
    assert_eq!(single_line("🐆abc", 2), "🐆a…");
    assert_eq!(quote_untrusted("Bob's\ninvoice", 100), "'Bob’s invoice'");
    assert_eq!(quote_untrusted("abcdef", 3), "'abc…'");
}

#[test]
fn unified_errors_preserve_typed_sources() {
    let error = Error::from(AmountError::Overflow);
    assert_eq!(error.to_string(), "amount exceeds u64::MAX base units");
    assert!(matches!(error, Error::Amount(AmountError::Overflow)));
}
