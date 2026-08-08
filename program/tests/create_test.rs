mod common;

use common::*;
use solana_keypair::Keypair;
use solana_signer::Signer;
use token_metadata::error::TokenMetadataError;

const SPL_TOKEN_PROGRAM_ID: pinocchio::Address = pinocchio_token::ID;
const SPL_TOKEN_2022_PROGRAM_ID: pinocchio::Address = pinocchio_token_2022::ID;

#[test]
fn create_metadata_succeeds_for_classic_spl_mint() {
    let mut svm = setup();
    let payer = funded_keypair(&mut svm);
    let mint_authority = funded_keypair(&mut svm);

    let mint = Keypair::new().pubkey();
    set_account(
        &mut svm,
        mint,
        SPL_TOKEN_PROGRAM_ID,
        mint_account_data(Some(mint_authority.pubkey()), 6),
        10_000_000,
    );

    let (metadata, bump) = metadata_pda(&mint);
    let update_authority = Keypair::new().pubkey();

    let creators = [CreatorSpec {
        address: Keypair::new().pubkey(),
        share: 100,
    }];
    let content = content_args("My Token", "MTK", "https://example.com/metadata.json", 250, &creators);

    let ix = build_create_ix(
        metadata,
        mint,
        mint_authority.pubkey(),
        payer.pubkey(),
        bump,
        update_authority,
        content,
    );

    send(&mut svm, &payer, &[&mint_authority], ix)
        .expect("create_metadata should succeed for a valid classic SPL mint");

    let m = read_metadata(&svm, &metadata);
    assert_eq!(m.discriminator, token_metadata::state::DISCRIMINATOR_TOKEN_METADATA_V1);
    assert_eq!(m.bump, bump);
    assert!(m.is_mutable());
    assert_eq!(m.token_kind, token_metadata::state::TokenKind::Fungible as u8);
    assert_eq!(m.update_authority, update_authority);
    assert_eq!(m.mint, mint);
    assert_eq!(m.royalty_bps(), 250);
    assert_eq!(m.name(), b"My Token");
    assert_eq!(m.symbol(), b"MTK");
    assert_eq!(m.uri(), b"https://example.com/metadata.json");
    assert_eq!(m.creators().len(), 1);
    assert_eq!(m.creators()[0].address, creators[0].address);
    assert_eq!(m.creators()[0].share, 100);
    assert!(!m.creators()[0].is_verified());
}

#[test]
fn create_metadata_succeeds_for_zero_decimal_mint_as_fungible_asset() {
    let mut svm = setup();
    let payer = funded_keypair(&mut svm);
    let mint_authority = funded_keypair(&mut svm);

    let mint = Keypair::new().pubkey();
    set_account(
        &mut svm,
        mint,
        SPL_TOKEN_PROGRAM_ID,
        mint_account_data(Some(mint_authority.pubkey()), 0),
        10_000_000,
    );

    let (metadata, bump) = metadata_pda(&mint);
    let content = content_args("Item", "ITM", "https://example.com/item.json", 0, &[]);
    let ix = build_create_ix(
        metadata,
        mint,
        mint_authority.pubkey(),
        payer.pubkey(),
        bump,
        payer.pubkey(),
        content,
    );

    send(&mut svm, &payer, &[&mint_authority], ix).expect("create_metadata should succeed");

    let m = read_metadata(&svm, &metadata);
    assert_eq!(m.token_kind, token_metadata::state::TokenKind::FungibleAsset as u8);
    assert_eq!(m.creators().len(), 0);
}

#[test]
fn create_metadata_fails_when_signer_is_not_mint_authority() {
    let mut svm = setup();
    let payer = funded_keypair(&mut svm);
    let real_mint_authority = funded_keypair(&mut svm);
    let impostor = funded_keypair(&mut svm);

    let mint = Keypair::new().pubkey();
    set_account(
        &mut svm,
        mint,
        SPL_TOKEN_PROGRAM_ID,
        mint_account_data(Some(real_mint_authority.pubkey()), 6),
        10_000_000,
    );

    let (metadata, bump) = metadata_pda(&mint);
    let content = content_args("Name", "SYM", "https://example.com", 0, &[]);
    let ix = build_create_ix(
        metadata,
        mint,
        impostor.pubkey(),
        payer.pubkey(),
        bump,
        payer.pubkey(),
        content,
    );

    let result = send(&mut svm, &payer, &[&impostor], ix);
    assert_custom_error(result, TokenMetadataError::InvalidMintAuthority as u32);
}

#[test]
fn create_metadata_fails_when_mint_authority_does_not_sign() {
    let mut svm = setup();
    let payer = funded_keypair(&mut svm);
    let mint_authority = funded_keypair(&mut svm);

    let mint = Keypair::new().pubkey();
    set_account(
        &mut svm,
        mint,
        SPL_TOKEN_PROGRAM_ID,
        mint_account_data(Some(mint_authority.pubkey()), 6),
        10_000_000,
    );

    let (metadata, bump) = metadata_pda(&mint);
    let content = content_args("Name", "SYM", "https://example.com", 0, &[]);
    // Note: `mint_authority` is referenced as a non-signer account here, and is deliberately not
    // included in `send`'s extra signers — the runtime still lets the transaction through (only
    // accounts marked `is_signer` in the *message* need real signatures), so this exercises our
    // program's own `is_signer()` check rather than transaction-level signature verification.
    let mut ix = build_create_ix(
        metadata,
        mint,
        mint_authority.pubkey(),
        payer.pubkey(),
        bump,
        payer.pubkey(),
        content,
    );
    ix.accounts[2].is_signer = false;

    let result = send(&mut svm, &payer, &[], ix);
    assert_custom_error(result, TokenMetadataError::MintAuthorityNotSigner as u32);
}

#[test]
fn create_metadata_fails_when_mint_has_no_authority() {
    let mut svm = setup();
    let payer = funded_keypair(&mut svm);
    let mint_authority = funded_keypair(&mut svm);

    let mint = Keypair::new().pubkey();
    set_account(
        &mut svm,
        mint,
        SPL_TOKEN_PROGRAM_ID,
        mint_account_data(None, 6), // fixed-supply mint, no mint authority
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
        payer.pubkey(),
        content,
    );

    let result = send(&mut svm, &payer, &[&mint_authority], ix);
    assert_custom_error(result, TokenMetadataError::MintHasNoAuthority as u32);
}

#[test]
fn create_metadata_fails_when_mint_is_owned_by_unrelated_program() {
    let mut svm = setup();
    let payer = funded_keypair(&mut svm);
    let mint_authority = funded_keypair(&mut svm);

    // Owned by our own program instead of a token program — not a legitimate mint at all.
    let mint = Keypair::new().pubkey();
    set_account(
        &mut svm,
        mint,
        program_id(),
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
        payer.pubkey(),
        content,
    );

    let result = send(&mut svm, &payer, &[&mint_authority], ix);
    assert_custom_error(result, TokenMetadataError::IncorrectMintOwner as u32);
}

#[test]
fn create_metadata_fails_when_creator_shares_do_not_sum_to_100() {
    let mut svm = setup();
    let payer = funded_keypair(&mut svm);
    let mint_authority = funded_keypair(&mut svm);

    let mint = Keypair::new().pubkey();
    set_account(
        &mut svm,
        mint,
        SPL_TOKEN_PROGRAM_ID,
        mint_account_data(Some(mint_authority.pubkey()), 6),
        10_000_000,
    );

    let (metadata, bump) = metadata_pda(&mint);
    let creators = [
        CreatorSpec { address: Keypair::new().pubkey(), share: 40 },
        CreatorSpec { address: Keypair::new().pubkey(), share: 40 },
    ];
    let content = content_args("Name", "SYM", "https://example.com", 0, &creators);
    let ix = build_create_ix(
        metadata,
        mint,
        mint_authority.pubkey(),
        payer.pubkey(),
        bump,
        payer.pubkey(),
        content,
    );

    let result = send(&mut svm, &payer, &[&mint_authority], ix);
    assert_custom_error(result, TokenMetadataError::CreatorSharesMustSumTo100 as u32);
}

#[test]
fn create_metadata_fails_on_duplicate_creator_addresses() {
    let mut svm = setup();
    let payer = funded_keypair(&mut svm);
    let mint_authority = funded_keypair(&mut svm);

    let mint = Keypair::new().pubkey();
    set_account(
        &mut svm,
        mint,
        SPL_TOKEN_PROGRAM_ID,
        mint_account_data(Some(mint_authority.pubkey()), 6),
        10_000_000,
    );

    let (metadata, bump) = metadata_pda(&mint);
    let same = Keypair::new().pubkey();
    let creators = [
        CreatorSpec { address: same, share: 50 },
        CreatorSpec { address: same, share: 50 },
    ];
    let content = content_args("Name", "SYM", "https://example.com", 0, &creators);
    let ix = build_create_ix(
        metadata,
        mint,
        mint_authority.pubkey(),
        payer.pubkey(),
        bump,
        payer.pubkey(),
        content,
    );

    let result = send(&mut svm, &payer, &[&mint_authority], ix);
    assert_custom_error(result, TokenMetadataError::DuplicateCreatorAddress as u32);
}

#[test]
fn create_metadata_succeeds_for_token2022_mint_without_metadata_pointer() {
    let mut svm = setup();
    let payer = funded_keypair(&mut svm);
    let mint_authority = funded_keypair(&mut svm);

    let mint = Keypair::new().pubkey();
    set_account(
        &mut svm,
        mint,
        SPL_TOKEN_2022_PROGRAM_ID,
        mint2022_account_data(Some(mint_authority.pubkey()), 6, None),
        10_000_000,
    );

    let (metadata, bump) = metadata_pda(&mint);
    let content = content_args("T22", "T22", "https://example.com", 0, &[]);
    let ix = build_create_ix(
        metadata,
        mint,
        mint_authority.pubkey(),
        payer.pubkey(),
        bump,
        payer.pubkey(),
        content,
    );

    send(&mut svm, &payer, &[&mint_authority], ix)
        .expect("create_metadata should succeed for a Token-2022 mint with no MetadataPointer extension");
}

#[test]
fn create_metadata_succeeds_for_token2022_mint_with_matching_metadata_pointer() {
    let mut svm = setup();
    let payer = funded_keypair(&mut svm);
    let mint_authority = funded_keypair(&mut svm);

    let mint = Keypair::new().pubkey();
    let (metadata, bump) = metadata_pda(&mint);

    set_account(
        &mut svm,
        mint,
        SPL_TOKEN_2022_PROGRAM_ID,
        mint2022_account_data(Some(mint_authority.pubkey()), 6, Some((None, Some(metadata)))),
        10_000_000,
    );

    let content = content_args("T22", "T22", "https://example.com", 0, &[]);
    let ix = build_create_ix(
        metadata,
        mint,
        mint_authority.pubkey(),
        payer.pubkey(),
        bump,
        payer.pubkey(),
        content,
    );

    send(&mut svm, &payer, &[&mint_authority], ix)
        .expect("create_metadata should succeed when MetadataPointer already points at this PDA");
}

#[test]
fn create_metadata_fails_for_token2022_mint_with_mismatched_metadata_pointer() {
    let mut svm = setup();
    let payer = funded_keypair(&mut svm);
    let mint_authority = funded_keypair(&mut svm);

    let mint = Keypair::new().pubkey();
    let (metadata, bump) = metadata_pda(&mint);
    let some_other_account = Keypair::new().pubkey();

    set_account(
        &mut svm,
        mint,
        SPL_TOKEN_2022_PROGRAM_ID,
        mint2022_account_data(Some(mint_authority.pubkey()), 6, Some((None, Some(some_other_account)))),
        10_000_000,
    );

    let content = content_args("T22", "T22", "https://example.com", 0, &[]);
    let ix = build_create_ix(
        metadata,
        mint,
        mint_authority.pubkey(),
        payer.pubkey(),
        bump,
        payer.pubkey(),
        content,
    );

    let result = send(&mut svm, &payer, &[&mint_authority], ix);
    assert_custom_error(result, TokenMetadataError::InvalidMetadataPointer as u32);
}

/// Unlike the other tests in this file, which inject a hand-constructed but byte-accurate `Mint`
/// account directly into LiteSVM's account store (see `mint_account_data`), this one actually
/// CPIs into the real, built-in SPL Token program (`system_program::CreateAccount` +
/// `InitializeMint2`) to produce the mint, then runs `CreateMetadata` against it — closing the gap
/// between "our program correctly reads our own understanding of the wire format" and "our program
/// correctly reads what the real token program actually produces".
#[test]
fn create_metadata_succeeds_against_a_genuinely_initialized_classic_mint() {
    let mut svm = setup();
    let payer = funded_keypair(&mut svm);
    let mint_authority = funded_keypair(&mut svm);

    let mint = create_real_mint(&mut svm, &payer, SPL_TOKEN_PROGRAM_ID, mint_authority.pubkey(), 6);

    let (metadata, bump) = metadata_pda(&mint);
    let content = content_args("Real Mint", "REAL", "https://example.com/real.json", 0, &[]);
    let ix = build_create_ix(
        metadata,
        mint,
        mint_authority.pubkey(),
        payer.pubkey(),
        bump,
        payer.pubkey(),
        content,
    );

    send(&mut svm, &payer, &[&mint_authority], ix)
        .expect("create_metadata should succeed against a genuinely initialized classic SPL mint");

    let m = read_metadata(&svm, &metadata);
    assert_eq!(m.mint, mint);
    assert_eq!(m.name(), b"Real Mint");
    assert_eq!(m.token_kind, token_metadata::state::TokenKind::Fungible as u8);
}

/// Same as above, but for a genuinely initialized Token-2022 mint (no extensions).
#[test]
fn create_metadata_succeeds_against_a_genuinely_initialized_token2022_mint() {
    let mut svm = setup();
    let payer = funded_keypair(&mut svm);
    let mint_authority = funded_keypair(&mut svm);

    let mint = create_real_mint(&mut svm, &payer, SPL_TOKEN_2022_PROGRAM_ID, mint_authority.pubkey(), 9);

    let (metadata, bump) = metadata_pda(&mint);
    let content = content_args("Real T22", "RT22", "https://example.com/real22.json", 0, &[]);
    let ix = build_create_ix(
        metadata,
        mint,
        mint_authority.pubkey(),
        payer.pubkey(),
        bump,
        payer.pubkey(),
        content,
    );

    send(&mut svm, &payer, &[&mint_authority], ix)
        .expect("create_metadata should succeed against a genuinely initialized Token-2022 mint");

    let m = read_metadata(&svm, &metadata);
    assert_eq!(m.mint, mint);
    assert_eq!(m.name(), b"Real T22");
}
