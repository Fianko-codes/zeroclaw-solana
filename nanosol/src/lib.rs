//! Minimal Solana primitives for the ZeroClaw Solana tool plugins.
//!
//! `nanosol` deliberately contains no signer, RPC client, async runtime, or
//! generic SDK surface. Its M1 scope is exact amount arithmetic, public keys
//! and PDAs, the instruction subset required by the planned tools, legacy/v0
//! message and unsigned-transaction codecs, and deterministic output shaping.

#![forbid(unsafe_code)]

pub mod amount;
pub mod compact_u16;
pub mod error;
pub mod inspect;
pub mod instruction;
pub mod message;
pub mod mint;
pub mod nonce;
pub mod pubkey;
pub mod reference;
pub mod rpc;
pub mod shape;

pub use error::Error;
