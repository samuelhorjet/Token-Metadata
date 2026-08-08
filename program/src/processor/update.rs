use pinocchio::{error::ProgramError, Address, AccountView, ProgramResult};

use crate::{
    checks::{assert_mutable, assert_update_authority_is_correct, validate_metadata_content},
    error::TokenMetadataError,
    instruction::UpdateMetadataArgs,
    state::{CreatorEntry, TokenMetadata, MAX_CREATORS},
};

pub fn process_update(
    program_id: &Address,
    accounts: &mut [AccountView],
    data: &[u8],
) -> ProgramResult {
    let [metadata_account, mint_account, authority_account] = accounts else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };

    let args = UpdateMetadataArgs::from_bytes(data)?;
    args.validate_lengths()?;

    let mut metadata = TokenMetadata::from_account_view_mut(metadata_account, program_id)?;

    assert_update_authority_is_correct(&metadata, authority_account)?;
    assert_mutable(&metadata)?;

    if metadata.mint != *mint_account.address() {
        return Err(TokenMetadataError::MintMismatch.into());
    }

    let creator_inputs = args.creators()?;
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
        args.name(),
        args.symbol(),
        args.uri(),
        args.royalty_bps(),
        creator_entries,
    )?;

    metadata.set_royalty_bps(args.royalty_bps());
    metadata.set_content(args.name(), args.symbol(), args.uri());
    metadata.set_creators(creator_entries);

    Ok(())
}
