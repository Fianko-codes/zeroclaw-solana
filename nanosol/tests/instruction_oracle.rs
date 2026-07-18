use nanosol::{
    instruction::{
        create_associated_token_account_idempotent, memo, set_compute_unit_limit,
        set_compute_unit_price, transfer_checked, AccountMeta, Instruction, InstructionError,
        TokenProgram,
    },
    pubkey::{Pubkey, LEGACY_TOKEN_PROGRAM_ID, TOKEN_2022_PROGRAM_ID},
};
use solana_instruction::Instruction as OfficialInstruction;
use solana_pubkey::Pubkey as OfficialPubkey;

fn key(byte: u8) -> Pubkey {
    Pubkey::new([byte; 32])
}

fn official(key: Pubkey) -> OfficialPubkey {
    OfficialPubkey::from(key.to_bytes())
}

fn assert_instruction_matches(actual: &Instruction, expected: &OfficialInstruction) {
    assert_eq!(actual.program_id.to_bytes(), expected.program_id.to_bytes());
    assert_eq!(actual.data, expected.data);
    assert_eq!(actual.accounts.len(), expected.accounts.len());
    for (actual, expected) in actual.accounts.iter().zip(&expected.accounts) {
        assert_eq!(actual.pubkey.to_bytes(), expected.pubkey.to_bytes());
        assert_eq!(actual.is_signer, expected.is_signer);
        assert_eq!(actual.is_writable, expected.is_writable);
    }
}

#[test]
fn legacy_and_token_2022_transfer_checked_match_official_builders() {
    let source = key(1);
    let mint = key(2);
    let destination = key(3);
    let authority = key(4);
    let amount = 25_010_000;
    let decimals = 6;

    let legacy = transfer_checked(
        source,
        mint,
        destination,
        authority,
        amount,
        decimals,
        TokenProgram::Legacy,
    );
    let official_legacy = spl_token_interface::instruction::transfer_checked(
        &official(LEGACY_TOKEN_PROGRAM_ID),
        &official(source),
        &official(mint),
        &official(destination),
        &official(authority),
        &[],
        amount,
        decimals,
    )
    .expect("official legacy transfer");
    assert_instruction_matches(&legacy, &official_legacy);

    let token_2022 = transfer_checked(
        source,
        mint,
        destination,
        authority,
        amount,
        decimals,
        TokenProgram::Token2022,
    );
    let official_token_2022 = spl_token_2022_interface::instruction::transfer_checked(
        &official(TOKEN_2022_PROGRAM_ID),
        &official(source),
        &official(mint),
        &official(destination),
        &official(authority),
        &[],
        amount,
        decimals,
    )
    .expect("official Token-2022 transfer");
    assert_instruction_matches(&token_2022, &official_token_2022);
}

#[test]
fn ata_create_idempotent_matches_official_builder_for_both_programs() {
    let payer = key(10);
    let owner = key(11);
    let mint = key(12);
    for token_program in [TokenProgram::Legacy, TokenProgram::Token2022] {
        let (actual, actual_ata) =
            create_associated_token_account_idempotent(payer, owner, mint, token_program)
                .expect("nanosol ATA instruction");
        let expected =
            spl_associated_token_account_interface::instruction::create_associated_token_account_idempotent(
                &official(payer),
                &official(owner),
                &official(mint),
                &official(token_program.id()),
            );
        assert_instruction_matches(&actual, &expected);
        assert_eq!(
            actual_ata.to_bytes(),
            expected.accounts[1].pubkey.to_bytes()
        );
    }
}

#[test]
fn compute_budget_and_memo_match_official_builders() {
    let limit = set_compute_unit_limit(200_000);
    let official_limit =
        solana_compute_budget_interface::ComputeBudgetInstruction::set_compute_unit_limit(200_000);
    assert_instruction_matches(&limit, &official_limit);

    let price = set_compute_unit_price(5_000);
    let official_price =
        solana_compute_budget_interface::ComputeBudgetInstruction::set_compute_unit_price(5_000);
    assert_instruction_matches(&price, &official_price);

    let memo = memo("invoice #412 🐆");
    let official_memo = spl_memo_interface::instruction::build_memo(
        &spl_memo_interface::v3::id(),
        "invoice #412 🐆".as_bytes(),
        &[],
    );
    assert_instruction_matches(&memo, &official_memo);
}

#[test]
fn token_program_identification_is_closed() {
    assert_eq!(
        TokenProgram::identify(&LEGACY_TOKEN_PROGRAM_ID),
        Ok(TokenProgram::Legacy)
    );
    assert_eq!(
        TokenProgram::identify(&TOKEN_2022_PROGRAM_ID),
        Ok(TokenProgram::Token2022)
    );
    assert_eq!(
        TokenProgram::identify(&key(99)),
        Err(InstructionError::UnsupportedTokenProgram(key(99)))
    );
}

#[test]
fn account_meta_constructors_preserve_requested_privileges() {
    assert_eq!(
        AccountMeta::writable(key(1), true),
        AccountMeta {
            pubkey: key(1),
            is_signer: true,
            is_writable: true,
        }
    );
    assert!(!AccountMeta::readonly(key(2), false).is_writable);
}
