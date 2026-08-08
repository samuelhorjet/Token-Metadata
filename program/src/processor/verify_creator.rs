use pinocchio::{error::ProgramError, Address, AccountView, ProgramResult};

use crate::{
    checks::{assert_mutable, assert_signer},
    error::TokenMetadataError,
    state::TokenMetadata,
};

/// A listed creator signs to flip their own `verified` flag to `true`.
///
/// Unlike `mpl-token-metadata`, no other authority (including the update authority) can ever set
/// `verified` on a creator's behalf — see [`crate::state::CreatorEntry`].
///
/// Gated by `is_mutable`, diverging from `mpl-token-metadata`'s `sign_metadata` (which is not):
/// here, "immutable" is defined to mean the entire record — content, authority, and creator
/// verification — is permanently frozen, so nothing about it can change after the fact, including
/// via a creator's own signature.
pub fn process_verify_creator(program_id: &Address, accounts: &mut [AccountView]) -> ProgramResult {
    set_creator_verified(program_id, accounts, true)
}

pub(super) fn set_creator_verified(
    program_id: &Address,
    accounts: &mut [AccountView],
    verified: bool,
) -> ProgramResult {
    let [metadata_account, creator_account] = accounts else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };

    assert_signer(creator_account)?;

    let mut metadata = TokenMetadata::from_account_view_mut(metadata_account, program_id)?;
    assert_mutable(&metadata)?;

    let creator_key = creator_account.address();
    let entry = metadata
        .creators_mut()
        .iter_mut()
        .find(|c| &c.address == creator_key)
        .ok_or(TokenMetadataError::SignerNotACreator)?;

    entry.verified = verified as u8;

    Ok(())
}
