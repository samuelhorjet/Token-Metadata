mod common;

use common::*;
use solana_address::Address;
use solana_keypair::Keypair;
use solana_signer::Signer;
use token_metadata::error::TokenMetadataError;

const SPL_TOKEN_PROGRAM_ID: pinocchio::Address = pinocchio_token::ID;

struct Fixture {
    mint: Address,
    metadata: Address,
    update_authority: Keypair,
}

fn create_fixture(svm: &mut litesvm::LiteSVM, payer: &Keypair) -> Fixture {
    let mint_authority = funded_keypair(svm);
    let update_authority = funded_keypair(svm);

    let mint = Keypair::new().pubkey();
    set_account(
        svm,
        mint,
        SPL_TOKEN_PROGRAM_ID,
        mint_account_data(Some(mint_authority.pubkey()), 6),
        10_000_000,
    );

    let (metadata, bump) = metadata_pda(&mint);
    let content = content_args("Name", "SYM", "https://example.com", 0, &[]);
    let ix = build_create_ix(
        metadata,
        mint,
        mint_authority.pubkey(),
        payer.pubkey(),
        bump,
        update_authority.pubkey(),
        content,
    );
    send(svm, payer, &[&mint_authority], ix).expect("fixture creation should succeed");

    Fixture { mint, metadata, update_authority }
}

#[test]
fn close_metadata_closes_account_and_reclaims_rent() {
    let mut svm = setup();
    let payer = funded_keypair(&mut svm);
    let f = create_fixture(&mut svm, &payer);
    let destination = Keypair::new().pubkey();

    let reclaimable = svm.get_account(&f.metadata).unwrap().lamports;
    assert!(reclaimable > 0);

    let ix = build_close_ix(f.metadata, f.mint, f.update_authority.pubkey(), destination);
    send(&mut svm, &payer, &[&f.update_authority], ix).expect("close_metadata should succeed");

    assert!(svm.get_account(&f.metadata).is_none(), "metadata account should be closed");
    assert_eq!(svm.get_balance(&destination).unwrap(), reclaimable);
}

#[test]
fn close_metadata_fails_with_wrong_update_authority() {
    let mut svm = setup();
    let payer = funded_keypair(&mut svm);
    let f = create_fixture(&mut svm, &payer);
    let impostor = funded_keypair(&mut svm);
    let destination = Keypair::new().pubkey();

    let ix = build_close_ix(f.metadata, f.mint, impostor.pubkey(), destination);
    let result = send(&mut svm, &payer, &[&impostor], ix);
    assert_custom_error(result, TokenMetadataError::UpdateAuthorityIncorrect as u32);
}

#[test]
fn close_metadata_fails_once_immutable() {
    let mut svm = setup();
    let payer = funded_keypair(&mut svm);
    let f = create_fixture(&mut svm, &payer);
    let destination = Keypair::new().pubkey();

    let lock_ix = build_set_immutable_ix(f.metadata, f.update_authority.pubkey());
    send(&mut svm, &payer, &[&f.update_authority], lock_ix).expect("set_immutable should succeed");

    let ix = build_close_ix(f.metadata, f.mint, f.update_authority.pubkey(), destination);
    let result = send(&mut svm, &payer, &[&f.update_authority], ix);
    assert_custom_error(result, TokenMetadataError::DataIsImmutable as u32);
}

#[test]
fn close_metadata_fails_with_mint_mismatch() {
    let mut svm = setup();
    let payer = funded_keypair(&mut svm);
    let f = create_fixture(&mut svm, &payer);
    let other_mint = Keypair::new().pubkey();
    let destination = Keypair::new().pubkey();

    let ix = build_close_ix(f.metadata, other_mint, f.update_authority.pubkey(), destination);
    let result = send(&mut svm, &payer, &[&f.update_authority], ix);
    assert_custom_error(result, TokenMetadataError::MintMismatch as u32);
}
