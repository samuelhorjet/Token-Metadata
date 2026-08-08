use pinocchio::{
    cpi::{Seed, Signer},
    error::ProgramError,
    Address, AccountView, ProgramResult,
};
use pinocchio_system::instructions::CreateAccount;

use crate::{
    checks::{
        assert_address, assert_mint_authority_matches, assert_signer, assert_writable, read_mint,
        validate_metadata_content, TokenProgramKind,
    },
    error::TokenMetadataError,
    instruction::CreateMetadataArgs,
    pda::{verify_metadata_address, METADATA_SEED},
    state::{
        read_metadata_pointer, CreatorEntry, TokenKind, TokenMetadata, DISCRIMINATOR_TOKEN_METADATA_V1,
        MAX_CREATORS,
    },
};

pub fn process_create(
    program_id: &Address,
    accounts: &mut [AccountView],
    data: &[u8],
) -> ProgramResult {
    let [metadata_account, mint_account, mint_authority_account, payer_account, system_program_account] =
        accounts
    else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };

    let args = CreateMetadataArgs::from_bytes(data)?;
    args.content.validate_lengths()?;

    assert_writable(metadata_account)?;
    assert_signer(payer_account)?;
    assert_writable(payer_account)?;
    assert_address(system_program_account, &pinocchio_system::ID)?;

    let mint_info = read_mint(mint_account)?;
    assert_mint_authority_matches(mint_info.mint_authority.as_ref(), mint_authority_account)?;

    verify_metadata_address(
        mint_account.address(),
        args.bump,
        program_id,
        metadata_account.address(),
    )?;

    if mint_info.program == TokenProgramKind::Token2022 {
        let mint_data = mint_account.try_borrow()?;
        if let Some(pointer) = read_metadata_pointer(&mint_data)? {
            if &pointer != metadata_account.address() {
                return Err(TokenMetadataError::InvalidMetadataPointer.into());
            }
        }
    }

    let creator_inputs = args.content.creators()?;
    let mut creator_entries = [CreatorEntry {
        address: Address::default(),
        verified: 0,
        share: 0,
    }; MAX_CREATORS];
    for (i, input) in creator_inputs.iter().enumerate() {
        creator_entries[i] = CreatorEntry {
            address: input.address,
            verified: 0,
            share: input.share,
        };
    }
    let creator_entries = &creator_entries[..creator_inputs.len()];

    validate_metadata_content(
        args.content.name(),
        args.content.symbol(),
        args.content.uri(),
        args.content.royalty_bps(),
        creator_entries,
    )?;

    let bump_seed = [args.bump];
    let seeds = [
        Seed::from(METADATA_SEED),
        Seed::from(mint_account.address().as_ref()),
        Seed::from(bump_seed.as_ref()),
    ];
    let signer = Signer::from(&seeds);

    CreateAccount::with_minimum_balance(
        payer_account,
        metadata_account,
        TokenMetadata::LEN as u64,
        program_id,
        None,
    )?
    .invoke_signed(&[signer])?;

    let mut metadata = TokenMetadata::from_uninitialized_account_view_mut(metadata_account, program_id)?;
    metadata.discriminator = DISCRIMINATOR_TOKEN_METADATA_V1;
    metadata.bump = args.bump;
    metadata.set_mutable(true);
    metadata.token_kind = TokenKind::from_decimals(mint_info.decimals) as u8;
    metadata.update_authority = args.initial_update_authority;
    metadata.mint = *mint_account.address();
    metadata.set_royalty_bps(args.content.royalty_bps());
    metadata.set_content(args.content.name(), args.content.symbol(), args.content.uri());
    metadata.set_creators(creator_entries);

    Ok(())
}
