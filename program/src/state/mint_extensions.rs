use pinocchio::{error::ProgramError, Address};

use crate::error::TokenMetadataError;

/// `ExtensionType::MetadataPointer` discriminant (see `spl-token-2022-interface`'s
/// `extension::ExtensionType` enum, `#[repr(u16)]`, positional — `MetadataPointer` is the 19th
/// variant, index 18).
const EXTENSION_TYPE_METADATA_POINTER: u16 = 18;
/// Size of the `MetadataPointer` extension's value: `authority: Address` (32) +
/// `metadata_address: Address` (32).
const METADATA_POINTER_VALUE_LEN: usize = 64;

/// Offset within a Token-2022 mint account's data at which the single `AccountType` marker byte
/// sits, immediately followed by the TLV-encoded extension list.
///
/// This is intentionally `pinocchio_token_2022::state::Account::BASE_LEN` (165), **not**
/// `Mint::BASE_LEN` (82): the SPL Token-2022 program stores this marker at the same fixed offset
/// for both mint and token accounts, so that a single length threshold distinguishes "extended
/// account with a type marker" from "bare Multisig" for both account kinds. This is confirmed
/// directly against `spl-token-2022-interface`'s `extension::BASE_ACCOUNT_LENGTH` handling.
const ACCOUNT_TYPE_MARKER_OFFSET: usize = pinocchio_token_2022::state::Account::BASE_LEN;

/// Scan a Token-2022 mint account's raw data for a `MetadataPointer` extension and return its
/// `metadata_address` field, if the extension is present and that field is set.
///
/// Returns `Ok(None)` both when the mint has no extensions at all, and when it has a
/// `MetadataPointer` extension whose `metadata_address` is unset (the all-zero sentinel) — in
/// both cases there is no on-mint pointer constraint to enforce. Returns `Err` only if the TLV
/// data is malformed (truncated headers/values), which should not happen for an account that has
/// already passed the base `Mint::from_account_view` layout check.
pub fn read_metadata_pointer(mint_data: &[u8]) -> Result<Option<Address>, ProgramError> {
    if mint_data.len() <= ACCOUNT_TYPE_MARKER_OFFSET {
        // Bare mint, no room for an `AccountType` marker or extensions.
        return Ok(None);
    }

    let mut offset = ACCOUNT_TYPE_MARKER_OFFSET + 1;

    loop {
        // Fewer than 4 bytes left means only trailing zero padding (reserved for future
        // extensions) remains; the TLV list is terminated there.
        if offset + 4 > mint_data.len() {
            return Ok(None);
        }

        let ext_type = u16::from_le_bytes([mint_data[offset], mint_data[offset + 1]]);
        let ext_len =
            u16::from_le_bytes([mint_data[offset + 2], mint_data[offset + 3]]) as usize;

        // `ExtensionType::Uninitialized == 0` marks the end of the initialized TLV list.
        if ext_type == 0 {
            return Ok(None);
        }

        let value_start = offset + 4;
        let value_end = value_start
            .checked_add(ext_len)
            .ok_or(TokenMetadataError::InvalidMint)?;
        if value_end > mint_data.len() {
            return Err(TokenMetadataError::InvalidMint.into());
        }

        if ext_type == EXTENSION_TYPE_METADATA_POINTER {
            if ext_len != METADATA_POINTER_VALUE_LEN {
                return Err(TokenMetadataError::InvalidMint.into());
            }
            let metadata_address_bytes = &mint_data[value_start + 32..value_end];
            if metadata_address_bytes == [0u8; 32] {
                return Ok(None);
            }
            let mut bytes = [0u8; 32];
            bytes.copy_from_slice(metadata_address_bytes);
            return Ok(Some(Address::from(bytes)));
        }

        offset = value_end;
    }
}
