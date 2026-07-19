//! Empirical probe of the official durable-nonce crate APIs and byte layouts.
//! Prints exact serialized bytes so the nanosol prototype can be built to match.

use solana_hash::Hash;
use solana_nonce::{
    state::{Data, State},
    versions::Versions,
};
use solana_pubkey::Pubkey;
use solana_system_interface::instruction as system_instruction;

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn main() {
    // ---- Nonce account bytes (Current / Initialized) ----
    let authority = Pubkey::new_from_array([0x11; 32]);
    let durable = solana_nonce::state::DurableNonce::from_blockhash(&Hash::new_from_array([0x22; 32]));
    let lamports_per_signature: u64 = 5000;
    let data = Data::new(authority, durable, lamports_per_signature);
    let state = State::Initialized(data.clone());
    let versions_current = Versions::new(state.clone());
    let ser = bincode::serialize(&versions_current).expect("serialize current initialized");
    println!("== NONCE ACCOUNT: Versions::new(Initialized) ==");
    println!("len={}", ser.len());
    println!("bytes={}", hex(&ser));
    println!("durable_nonce_hash={}", hex(durable.as_hash().as_ref()));
    println!("authority={}", authority);

    // Uninitialized
    let versions_uninit = Versions::new(State::Uninitialized);
    let ser_uninit = bincode::serialize(&versions_uninit).expect("serialize uninit");
    println!("== NONCE ACCOUNT: Versions::new(Uninitialized) ==");
    println!("len={} bytes={}", ser_uninit.len(), hex(&ser_uninit));

    // Legacy variant (explicit)
    let versions_legacy = Versions::Legacy(std::boxed::Box::new(state.clone()));
    let ser_legacy = bincode::serialize(&versions_legacy).expect("serialize legacy");
    println!("== NONCE ACCOUNT: Versions::Legacy(Initialized) ==");
    println!("len={} bytes={}", ser_legacy.len(), hex(&ser_legacy));

    // trailing-byte behaviour of bincode::deserialize
    let mut oversized = ser.clone();
    oversized.push(0x00);
    let de: Result<Versions, _> = bincode::deserialize(&oversized);
    println!("== bincode::deserialize on 81 bytes -> is_err={} ==", de.is_err());

    // ---- AdvanceNonceAccount instruction ----
    let nonce_account = Pubkey::new_from_array([0x33; 32]);
    let nonce_authority = Pubkey::new_from_array([0x44; 32]);
    let ix = system_instruction::advance_nonce_account(&nonce_account, &nonce_authority);
    println!("== AdvanceNonceAccount ==");
    println!("program_id={} raw={}", ix.program_id, hex(ix.program_id.as_ref()));
    println!("data={}", hex(&ix.data));
    for (i, m) in ix.accounts.iter().enumerate() {
        println!(
            "acct[{i}] pubkey={} signer={} writable={} raw={}",
            m.pubkey, m.is_signer, m.is_writable, hex(m.pubkey.as_ref())
        );
    }

    // ---- IDs ----
    println!("== IDs ==");
    println!("system_program={} raw={}", solana_sdk_ids::system_program::ID, hex(solana_sdk_ids::system_program::ID.as_ref()));
    #[allow(deprecated)]
    {
        println!(
            "recent_blockhashes_sysvar={} raw={}",
            solana_sdk_ids::sysvar::recent_blockhashes::ID,
            hex(solana_sdk_ids::sysvar::recent_blockhashes::ID.as_ref())
        );
    }
}
