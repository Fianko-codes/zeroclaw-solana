//! Deterministic reconciliation references shared by Solana payment tools.

use sha2::{Digest, Sha256};

use crate::pubkey::Pubkey;

pub const PAYMENT_REFERENCE_DOMAIN: &[u8] = b"zeroclaw-solana-pay-v1";
const NATIVE_SOL_MINT_SENTINEL: [u8; 32] = [0; 32];

/// Derive the exact framed, asset-discriminated reference used by
/// `solana-pay-request`.
///
/// The two variable-width fields use big-endian `u32` length prefixes. The
/// asset tag separates native SOL from a token whose mint bytes are all zero.
pub fn derive_payment_reference(
    recipient: &Pubkey,
    mint: Option<&Pubkey>,
    canonical_amount: &str,
    invoice_id: &str,
) -> Pubkey {
    let mut hasher = Sha256::new();
    hasher.update(PAYMENT_REFERENCE_DOMAIN);
    hasher.update(recipient.as_bytes());
    match mint {
        Some(mint) => {
            hasher.update([1]);
            hasher.update(mint.as_bytes());
        }
        None => {
            hasher.update([0]);
            hasher.update(NATIVE_SOL_MINT_SENTINEL);
        }
    }
    update_frame(&mut hasher, canonical_amount.as_bytes());
    update_frame(&mut hasher, invoice_id.as_bytes());
    Pubkey::new(hasher.finalize().into())
}

fn update_frame(hasher: &mut Sha256, value: &[u8]) {
    // Callers bound both fields far below u32::MAX. Saturation keeps this
    // deterministic and panic-free if the helper is reused incorrectly.
    let length = u32::try_from(value.len()).unwrap_or(u32::MAX);
    hasher.update(length.to_be_bytes());
    hasher.update(value);
}
