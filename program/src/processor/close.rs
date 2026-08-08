use pinocchio::{error::ProgramError, Address, AccountView, ProgramResult};

use crate::{
    checks::{assert_mutable, assert_update_authority_is_correct, assert_writable},
    error::TokenMetadataError,
    state::TokenMetadata,
};

pub fn process_close(program_id: &Address, accounts: &mut [AccountView]) -> ProgramResult {
    let [metadata_account, mint_account, authority_account, destination_account] = accounts
    else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };

    assert_writable(metadata_account)?;
    assert_writable(destination_account)?;

    {
        let metadata = TokenMetadata::from_account_view_mut(metadata_account, program_id)?;

        assert_update_authority_is_correct(&metadata, authority_account)?;
        // Closing is only allowed while mutable: once an authority commits to immutability, the
        // metadata is guaranteed to holders to neither change nor disappear.
        assert_mutable(&metadata)?;

        if metadata.mint != *mint_account.address() {
            return Err(TokenMetadataError::MintMismatch.into());
        }
        // `metadata` (a `RefMut` borrowing `metadata_account`'s data) is dropped at the end of
        // this block, which is required before `close()` below: the account's own borrow
        // tracking would otherwise reject the close while a live borrow is outstanding.
    }

    let reclaimed = metadata_account.lamports();
    let destination_balance = destination_account.lamports();
    destination_account.set_lamports(
        destination_balance
            .checked_add(reclaimed)
            .ok_or(TokenMetadataError::NumericalOverflow)?,
    );
    metadata_account.set_lamports(0);
    metadata_account.close()?;

    Ok(())
}
