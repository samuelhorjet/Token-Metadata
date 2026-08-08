use pinocchio::{Address, AccountView, ProgramResult};

use super::verify_creator::set_creator_verified;

/// A listed creator signs to flip their own `verified` flag back to `false`.
///
/// See [`super::verify_creator::process_verify_creator`] for the shared authorization/mutability
/// rules.
pub fn process_unverify_creator(program_id: &Address, accounts: &mut [AccountView]) -> ProgramResult {
    set_creator_verified(program_id, accounts, false)
}
