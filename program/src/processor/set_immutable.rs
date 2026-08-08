use pinocchio::{error::ProgramError, Address, AccountView, ProgramResult};

use crate::{
    checks::assert_update_authority_is_correct, error::TokenMetadataError, state::TokenMetadata,
};

pub fn process_set_immutable(program_id: &Address, accounts: &mut [AccountView]) -> ProgramResult {
    let [metadata_account, authority_account] = accounts else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };

    let mut metadata = TokenMetadata::from_account_view_mut(metadata_account, program_id)?;

    assert_update_authority_is_correct(&metadata, authority_account)?;

    if !metadata.is_mutable() {
        return Err(TokenMetadataError::AlreadyImmutable.into());
    }

    metadata.set_mutable(false);

    Ok(())
}
