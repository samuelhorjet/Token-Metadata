use pinocchio::{error::ProgramError, Address, AccountView, ProgramResult};

use crate::{
    checks::{assert_mutable, assert_update_authority_is_correct},
    error::TokenMetadataError,
    instruction::SetUpdateAuthorityArgs,
    state::TokenMetadata,
};

pub fn process_set_update_authority(
    program_id: &Address,
    accounts: &mut [AccountView],
    data: &[u8],
) -> ProgramResult {
    let [metadata_account, authority_account] = accounts else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };

    let args = SetUpdateAuthorityArgs::from_bytes(data)?;

    let mut metadata = TokenMetadata::from_account_view_mut(metadata_account, program_id)?;

    assert_update_authority_is_correct(&metadata, authority_account)?;
    // Once immutable, the entire record (content, authority, existence) is permanently frozen,
    // not just its content — see `assert_mutable`'s usage in `processor::update`/`processor::close`.
    assert_mutable(&metadata)?;

    let new_authority = if args.renounce != 0 {
        Address::default()
    } else {
        if args.new_update_authority == Address::default() {
            return Err(TokenMetadataError::InvalidNewUpdateAuthority.into());
        }
        args.new_update_authority
    };

    metadata.update_authority = new_authority;

    Ok(())
}
