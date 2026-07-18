use std::str::FromStr;

use nanosol::{pubkey::Pubkey, reference::derive_payment_reference};

const RECIPIENT: &str = "7xKXtg2CW87d97TXJSDpbD5jBkheTqA83TZRuJosgAsU";
const MINT: &str = "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v";
const REFERENCE: &str = "ECvLKMSgRzVdJjZsdiGAPcRSjwVjS9f7HxizfC256Kei";

#[test]
fn payment_reference_preserves_the_m2_golden_vector() {
    let recipient = Pubkey::from_str(RECIPIENT).expect("recipient fixture");
    let mint = Pubkey::from_str(MINT).expect("mint fixture");
    let reference = derive_payment_reference(&recipient, Some(&mint), "25.01", "412");
    assert_eq!(reference.to_string(), REFERENCE);
    assert_eq!(
        reference.to_bytes(),
        [
            0xc4, 0x35, 0x9f, 0x70, 0x25, 0x80, 0xc9, 0x70, 0xdb, 0x69, 0x3f, 0x7a, 0xba, 0x0d,
            0xa6, 0xce, 0xae, 0x3a, 0xac, 0xa3, 0x53, 0x34, 0xe4, 0xbb, 0x44, 0x64, 0x24, 0x1c,
            0x51, 0x9d, 0x74, 0x7b,
        ]
    );
}

#[test]
fn framing_and_asset_tag_prevent_known_ambiguities() {
    let recipient = Pubkey::from_str(RECIPIENT).expect("recipient fixture");
    assert_ne!(
        derive_payment_reference(&recipient, None, "1", "23"),
        derive_payment_reference(&recipient, None, "12", "3")
    );
    assert_ne!(
        derive_payment_reference(&recipient, None, "1", "23"),
        derive_payment_reference(&recipient, Some(&Pubkey::new([0; 32])), "1", "23")
    );
}
