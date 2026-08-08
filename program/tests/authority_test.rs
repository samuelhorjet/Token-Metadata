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
fn set_update_authority_transfers_to_new_authority() {
    let mut svm = setup();
    let payer = funded_keypair(&mut svm);
    let f = create_fixture(&mut svm, &payer);
    let new_authority = Keypair::new().pubkey();

    let ix = build_set_update_authority_ix(f.metadata, f.update_authority.pubkey(), false, new_authority);
    send(&mut svm, &payer, &[&f.update_authority], ix).expect("set_update_authority should succeed");

    let m = read_metadata(&svm, &f.metadata);
    assert_eq!(m.update_authority, new_authority);

    // Old authority can no longer update.
    let content = content_args("X", "X", "https://example.com", 0, &[]);
    let ix = build_update_ix(f.metadata, f.mint, f.update_authority.pubkey(), content);
    let result = send(&mut svm, &payer, &[&f.update_authority], ix);
    assert_custom_error(result, TokenMetadataError::UpdateAuthorityIncorrect as u32);
}

#[test]
fn set_update_authority_renounce_sets_default_address() {
    let mut svm = setup();
    let payer = funded_keypair(&mut svm);
    let f = create_fixture(&mut svm, &payer);

    let ix = build_set_update_authority_ix(f.metadata, f.update_authority.pubkey(), true, Address::default());
    send(&mut svm, &payer, &[&f.update_authority], ix).expect("renounce should succeed");

    let m = read_metadata(&svm, &f.metadata);
    assert_eq!(m.update_authority, Address::default());

    // Nobody (not even the former authority) can update anymore.
    let content = content_args("X", "X", "https://example.com", 0, &[]);
    let ix = build_update_ix(f.metadata, f.mint, f.update_authority.pubkey(), content);
    let result = send(&mut svm, &payer, &[&f.update_authority], ix);
    assert_custom_error(result, TokenMetadataError::UpdateAuthorityIncorrect as u32);
}

#[test]
fn set_update_authority_rejects_default_address_without_renounce_flag() {
    let mut svm = setup();
    let payer = funded_keypair(&mut svm);
    let f = create_fixture(&mut svm, &payer);

    let ix = build_set_update_authority_ix(f.metadata, f.update_authority.pubkey(), false, Address::default());
    let result = send(&mut svm, &payer, &[&f.update_authority], ix);
    assert_custom_error(result, TokenMetadataError::InvalidNewUpdateAuthority as u32);
}

#[test]
fn set_update_authority_fails_with_wrong_authority() {
    let mut svm = setup();
    let payer = funded_keypair(&mut svm);
    let f = create_fixture(&mut svm, &payer);
    let impostor = funded_keypair(&mut svm);

    let ix = build_set_update_authority_ix(f.metadata, impostor.pubkey(), false, Keypair::new().pubkey());
    let result = send(&mut svm, &payer, &[&impostor], ix);
    assert_custom_error(result, TokenMetadataError::UpdateAuthorityIncorrect as u32);
}

#[test]
fn set_immutable_is_one_way() {
    let mut svm = setup();
    let payer = funded_keypair(&mut svm);
    let f = create_fixture(&mut svm, &payer);

    let ix = build_set_immutable_ix(f.metadata, f.update_authority.pubkey());
    send(&mut svm, &payer, &[&f.update_authority], ix).expect("first set_immutable should succeed");

    let m = read_metadata(&svm, &f.metadata);
    assert!(!m.is_mutable());

    let ix = build_set_immutable_ix(f.metadata, f.update_authority.pubkey());
    let result = send(&mut svm, &payer, &[&f.update_authority], ix);
    assert_custom_error(result, TokenMetadataError::AlreadyImmutable as u32);
}

#[test]
fn set_immutable_fails_with_wrong_authority() {
    let mut svm = setup();
    let payer = funded_keypair(&mut svm);
    let f = create_fixture(&mut svm, &payer);
    let impostor = funded_keypair(&mut svm);

    let ix = build_set_immutable_ix(f.metadata, impostor.pubkey());
    let result = send(&mut svm, &payer, &[&impostor], ix);
    assert_custom_error(result, TokenMetadataError::UpdateAuthorityIncorrect as u32);
}

#[test]
fn set_update_authority_fails_once_immutable() {
    let mut svm = setup();
    let payer = funded_keypair(&mut svm);
    let f = create_fixture(&mut svm, &payer);

    let lock_ix = build_set_immutable_ix(f.metadata, f.update_authority.pubkey());
    send(&mut svm, &payer, &[&f.update_authority], lock_ix).expect("set_immutable should succeed");

    let ix = build_set_update_authority_ix(f.metadata, f.update_authority.pubkey(), false, Keypair::new().pubkey());
    let result = send(&mut svm, &payer, &[&f.update_authority], ix);
    assert_custom_error(result, TokenMetadataError::DataIsImmutable as u32);
}
