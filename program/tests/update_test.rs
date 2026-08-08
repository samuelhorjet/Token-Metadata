mod common;

use common::*;
use solana_keypair::Keypair;
use solana_signer::Signer;
use token_metadata::error::TokenMetadataError;

const SPL_TOKEN_PROGRAM_ID: pinocchio::Address = pinocchio_token::ID;

/// Creates a mint + fresh mutable metadata account, returning everything a test needs to then
/// exercise `UpdateMetadata` (or other authority-gated instructions) against it.
struct Fixture {
    mint: pinocchio::Address,
    metadata: pinocchio::Address,
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
    let content = content_args("Initial", "INI", "https://example.com/initial.json", 0, &[]);
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
fn update_metadata_succeeds_and_replaces_content() {
    let mut svm = setup();
    let payer = funded_keypair(&mut svm);
    let f = create_fixture(&mut svm, &payer);

    let creators = [CreatorSpec { address: Keypair::new().pubkey(), share: 100 }];
    let content = content_args("Updated", "UPD", "https://example.com/updated.json", 500, &creators);
    let ix = build_update_ix(f.metadata, f.mint, f.update_authority.pubkey(), content);

    send(&mut svm, &payer, &[&f.update_authority], ix).expect("update_metadata should succeed");

    let m = read_metadata(&svm, &f.metadata);
    assert_eq!(m.name(), b"Updated");
    assert_eq!(m.symbol(), b"UPD");
    assert_eq!(m.uri(), b"https://example.com/updated.json");
    assert_eq!(m.royalty_bps(), 500);
    assert_eq!(m.creators().len(), 1);
    assert_eq!(m.creators()[0].address, creators[0].address);
}

#[test]
fn update_metadata_fails_with_wrong_update_authority() {
    let mut svm = setup();
    let payer = funded_keypair(&mut svm);
    let f = create_fixture(&mut svm, &payer);
    let impostor = funded_keypair(&mut svm);

    let content = content_args("Updated", "UPD", "https://example.com", 0, &[]);
    let ix = build_update_ix(f.metadata, f.mint, impostor.pubkey(), content);

    let result = send(&mut svm, &payer, &[&impostor], ix);
    assert_custom_error(result, TokenMetadataError::UpdateAuthorityIncorrect as u32);
}

#[test]
fn update_metadata_fails_when_authority_does_not_sign() {
    let mut svm = setup();
    let payer = funded_keypair(&mut svm);
    let f = create_fixture(&mut svm, &payer);

    let content = content_args("Updated", "UPD", "https://example.com", 0, &[]);
    let mut ix = build_update_ix(f.metadata, f.mint, f.update_authority.pubkey(), content);
    ix.accounts[2].is_signer = false;

    let result = send(&mut svm, &payer, &[], ix);
    assert_custom_error(result, TokenMetadataError::UpdateAuthorityNotSigner as u32);
}

#[test]
fn update_metadata_fails_with_mint_mismatch() {
    let mut svm = setup();
    let payer = funded_keypair(&mut svm);
    let f = create_fixture(&mut svm, &payer);
    let other_mint = Keypair::new().pubkey();

    let content = content_args("Updated", "UPD", "https://example.com", 0, &[]);
    let ix = build_update_ix(f.metadata, other_mint, f.update_authority.pubkey(), content);

    let result = send(&mut svm, &payer, &[&f.update_authority], ix);
    assert_custom_error(result, TokenMetadataError::MintMismatch as u32);
}

#[test]
fn update_metadata_fails_once_immutable() {
    let mut svm = setup();
    let payer = funded_keypair(&mut svm);
    let f = create_fixture(&mut svm, &payer);

    let lock_ix = build_set_immutable_ix(f.metadata, f.update_authority.pubkey());
    send(&mut svm, &payer, &[&f.update_authority], lock_ix).expect("set_immutable should succeed");

    let content = content_args("Updated", "UPD", "https://example.com", 0, &[]);
    let ix = build_update_ix(f.metadata, f.mint, f.update_authority.pubkey(), content);
    let result = send(&mut svm, &payer, &[&f.update_authority], ix);
    assert_custom_error(result, TokenMetadataError::DataIsImmutable as u32);
}

#[test]
fn update_metadata_fails_on_name_too_long() {
    let mut svm = setup();
    let payer = funded_keypair(&mut svm);
    let f = create_fixture(&mut svm, &payer);

    let mut content = content_args("short", "SYM", "https://example.com", 0, &[]);
    content.name_len = (token_metadata::state::MAX_NAME_LEN + 1) as u8;
    let ix = build_update_ix(f.metadata, f.mint, f.update_authority.pubkey(), content);

    let result = send(&mut svm, &payer, &[&f.update_authority], ix);
    assert_custom_error(result, TokenMetadataError::NameTooLong as u32);
}
