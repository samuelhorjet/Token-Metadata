mod common;

use common::*;
use solana_address::Address;
use solana_keypair::Keypair;
use solana_signer::Signer;
use token_metadata::error::TokenMetadataError;

const SPL_TOKEN_PROGRAM_ID: pinocchio::Address = pinocchio_token::ID;

struct Fixture {
    metadata: Address,
    update_authority: Keypair,
    creator: Keypair,
}

fn create_fixture(svm: &mut litesvm::LiteSVM, payer: &Keypair) -> Fixture {
    let mint_authority = funded_keypair(svm);
    let update_authority = funded_keypair(svm);
    let creator = funded_keypair(svm);

    let mint = Keypair::new().pubkey();
    set_account(
        svm,
        mint,
        SPL_TOKEN_PROGRAM_ID,
        mint_account_data(Some(mint_authority.pubkey()), 6),
        10_000_000,
    );

    let (metadata, bump) = metadata_pda(&mint);
    let creators = [CreatorSpec { address: creator.pubkey(), share: 100 }];
    let content = content_args("Name", "SYM", "https://example.com", 0, &creators);
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

    Fixture { metadata, update_authority, creator }
}

#[test]
fn verify_creator_succeeds_when_signed_by_the_creator_itself() {
    let mut svm = setup();
    let payer = funded_keypair(&mut svm);
    let f = create_fixture(&mut svm, &payer);

    let ix = build_verify_creator_ix(f.metadata, f.creator.pubkey());
    send(&mut svm, &payer, &[&f.creator], ix).expect("verify_creator should succeed");

    let m = read_metadata(&svm, &f.metadata);
    assert!(m.creators()[0].is_verified());
}

#[test]
fn unverify_creator_succeeds_after_verify() {
    let mut svm = setup();
    let payer = funded_keypair(&mut svm);
    let f = create_fixture(&mut svm, &payer);

    let ix = build_verify_creator_ix(f.metadata, f.creator.pubkey());
    send(&mut svm, &payer, &[&f.creator], ix).expect("verify_creator should succeed");

    let ix = build_unverify_creator_ix(f.metadata, f.creator.pubkey());
    send(&mut svm, &payer, &[&f.creator], ix).expect("unverify_creator should succeed");

    let m = read_metadata(&svm, &f.metadata);
    assert!(!m.creators()[0].is_verified());
}

#[test]
fn verify_creator_fails_when_signer_is_not_a_listed_creator() {
    let mut svm = setup();
    let payer = funded_keypair(&mut svm);
    let f = create_fixture(&mut svm, &payer);
    let stranger = funded_keypair(&mut svm);

    let ix = build_verify_creator_ix(f.metadata, stranger.pubkey());
    let result = send(&mut svm, &payer, &[&stranger], ix);
    assert_custom_error(result, TokenMetadataError::SignerNotACreator as u32);
}

#[test]
fn verify_creator_fails_when_update_authority_tries_to_verify_on_creators_behalf() {
    let mut svm = setup();
    let payer = funded_keypair(&mut svm);
    let f = create_fixture(&mut svm, &payer);

    // Even the update authority cannot flip another address's `verified` bit — only the
    // creator's own signature can, by design (see `state::CreatorEntry`).
    let ix = build_verify_creator_ix(f.metadata, f.update_authority.pubkey());
    let result = send(&mut svm, &payer, &[&f.update_authority], ix);
    assert_custom_error(result, TokenMetadataError::SignerNotACreator as u32);
}

#[test]
fn verify_creator_fails_once_immutable() {
    let mut svm = setup();
    let payer = funded_keypair(&mut svm);
    let f = create_fixture(&mut svm, &payer);

    let lock_ix = build_set_immutable_ix(f.metadata, f.update_authority.pubkey());
    send(&mut svm, &payer, &[&f.update_authority], lock_ix).expect("set_immutable should succeed");

    let ix = build_verify_creator_ix(f.metadata, f.creator.pubkey());
    let result = send(&mut svm, &payer, &[&f.creator], ix);
    assert_custom_error(result, TokenMetadataError::DataIsImmutable as u32);
}
