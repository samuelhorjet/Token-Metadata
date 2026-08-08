use core::mem::size_of;

use pinocchio::{error::ProgramError, Address};

use crate::{
    error::TokenMetadataError,
    state::{MAX_CREATORS, MAX_NAME_LEN, MAX_SYMBOL_LEN, MAX_URI_LEN},
};

/// Instruction discriminators: the first 8 bytes of `sha256("global:<snake_case_name>")`, the
/// same convention Anchor (and this codebase's sibling `naclac` framework) uses for instruction
/// sighashes. Namespacing by name rather than assigning small sequential integers avoids
/// collisions across unrelated programs/tooling and lets indexers/explorers recognize instructions
/// without needing this program's IDL.
pub mod discriminator {
    pub const CREATE_METADATA: [u8; 8] = [0x1e, 0x23, 0x75, 0x86, 0xc4, 0x8b, 0x2c, 0x19];
    pub const UPDATE_METADATA: [u8; 8] = [0xaa, 0xb6, 0x2b, 0xef, 0x61, 0x4e, 0xe1, 0xba];
    pub const SET_UPDATE_AUTHORITY: [u8; 8] = [0xa6, 0xc6, 0xba, 0xff, 0xd9, 0xaa, 0x67, 0x9b];
    pub const SET_IMMUTABLE: [u8; 8] = [0x87, 0xe7, 0x05, 0xd1, 0xd2, 0x6b, 0x4d, 0x83];
    pub const VERIFY_CREATOR: [u8; 8] = [0x34, 0x11, 0x60, 0x84, 0x47, 0x04, 0x55, 0xc2];
    pub const UNVERIFY_CREATOR: [u8; 8] = [0x6b, 0xb2, 0x39, 0x27, 0x69, 0x73, 0x70, 0x98];
    pub const CLOSE_METADATA: [u8; 8] = [0x0a, 0xdc, 0xc4, 0x8a, 0x13, 0x3c, 0xcc, 0x82];
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TokenMetadataInstruction {
    /// Attach metadata to an existing, initialized mint. Accounts:
    /// 0. `[writable]` metadata PDA (`["metadata", mint, bump]`), must be uninitialized
    /// 1. `[]` mint
    /// 2. `[signer]` mint authority (must equal the mint's recorded authority)
    /// 3. `[signer, writable]` payer (funds the new account)
    /// 4. `[]` system program
    ///
    /// Data: [`CreateMetadataArgs`]
    CreateMetadata,
    /// Replace name/symbol/uri/royalty/creators on existing, mutable metadata. Accounts:
    /// 0. `[writable]` metadata PDA
    /// 1. `[]` mint
    /// 2. `[signer]` update authority
    ///
    /// Data: [`UpdateMetadataArgs`]
    UpdateMetadata,
    /// Transfer or renounce the update authority. Accounts:
    /// 0. `[writable]` metadata PDA
    /// 1. `[signer]` current update authority
    ///
    /// Data: [`SetUpdateAuthorityArgs`]
    SetUpdateAuthority,
    /// One-way flip `is_mutable` from `true` to `false`. Accounts:
    /// 0. `[writable]` metadata PDA
    /// 1. `[signer]` update authority
    ///
    /// Data: none
    SetImmutable,
    /// A listed creator signs to flip their own `verified` to `true`. Accounts:
    /// 0. `[writable]` metadata PDA
    /// 1. `[signer]` creator
    ///
    /// Data: none
    VerifyCreator,
    /// A listed creator signs to flip their own `verified` to `false`. Accounts:
    /// 0. `[writable]` metadata PDA
    /// 1. `[signer]` creator
    ///
    /// Data: none
    UnverifyCreator,
    /// Close the metadata account and reclaim its rent; only while mutable. Accounts:
    /// 0. `[writable]` metadata PDA
    /// 1. `[]` mint
    /// 2. `[signer]` update authority
    /// 3. `[writable]` rent-reclaim destination
    ///
    /// Data: none
    CloseMetadata,
}

impl TokenMetadataInstruction {
    pub fn try_from_discriminator(tag: &[u8; 8]) -> Result<Self, ProgramError> {
        Ok(match *tag {
            discriminator::CREATE_METADATA => Self::CreateMetadata,
            discriminator::UPDATE_METADATA => Self::UpdateMetadata,
            discriminator::SET_UPDATE_AUTHORITY => Self::SetUpdateAuthority,
            discriminator::SET_IMMUTABLE => Self::SetImmutable,
            discriminator::VERIFY_CREATOR => Self::VerifyCreator,
            discriminator::UNVERIFY_CREATOR => Self::UnverifyCreator,
            discriminator::CLOSE_METADATA => Self::CloseMetadata,
            _ => return Err(TokenMetadataError::UnknownInstruction.into()),
        })
    }
}

/// Split raw instruction data into its 8-byte discriminator and the remaining payload.
pub fn split_discriminator(data: &[u8]) -> Result<(TokenMetadataInstruction, &[u8]), ProgramError> {
    if data.len() < 8 {
        return Err(TokenMetadataError::InvalidInstructionData.into());
    }
    let (tag, rest) = data.split_at(8);
    let tag: [u8; 8] = tag.try_into().unwrap();
    Ok((TokenMetadataInstruction::try_from_discriminator(&tag)?, rest))
}

/// A single creator supplied in instruction data: an address and its attribution share. Unlike
/// [`crate::state::CreatorEntry`], there is no `verified` field — creators are always written as
/// unverified by `Create`/`Update`; verification only ever happens via the dedicated
/// `VerifyCreator`/`UnverifyCreator` instructions, using the creator's own signature.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, bytemuck::Pod, bytemuck::Zeroable)]
pub struct CreatorInput {
    pub address: Address,
    pub share: u8,
}

const _: () = assert!(size_of::<CreatorInput>() == 33);

/// Shared, fixed-layout content payload for `CreateMetadata` and `UpdateMetadata`.
#[repr(C)]
#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct MetadataContentArgs {
    pub royalty_bps: [u8; 2],
    pub name_len: u8,
    pub name: [u8; MAX_NAME_LEN],
    pub symbol_len: u8,
    pub symbol: [u8; MAX_SYMBOL_LEN],
    pub uri_len: u8,
    pub uri: [u8; MAX_URI_LEN],
    pub creator_count: u8,
    pub creators: [CreatorInput; MAX_CREATORS],
}

const _: () = assert!(size_of::<MetadataContentArgs>() == 413);

impl MetadataContentArgs {
    pub const LEN: usize = size_of::<Self>();

    #[inline(always)]
    pub fn royalty_bps(&self) -> u16 {
        u16::from_le_bytes(self.royalty_bps)
    }

    #[inline(always)]
    pub fn name(&self) -> &[u8] {
        &self.name[..self.name_len as usize]
    }

    #[inline(always)]
    pub fn symbol(&self) -> &[u8] {
        &self.symbol[..self.symbol_len as usize]
    }

    #[inline(always)]
    pub fn uri(&self) -> &[u8] {
        &self.uri[..self.uri_len as usize]
    }

    #[inline(always)]
    pub fn creators(&self) -> Result<&[CreatorInput], ProgramError> {
        if self.creator_count as usize > MAX_CREATORS {
            return Err(TokenMetadataError::TooManyCreators.into());
        }
        Ok(&self.creators[..self.creator_count as usize])
    }

    /// # Safety
    /// `bytes` must be exactly `Self::LEN` long.
    unsafe fn from_bytes_unchecked(bytes: &[u8]) -> &Self {
        &*(bytes.as_ptr() as *const Self)
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<&Self, ProgramError> {
        if bytes.len() != Self::LEN {
            return Err(TokenMetadataError::InvalidInstructionData.into());
        }
        // Every field is a byte array, `u8`, or `Address` (itself `repr(transparent)` over
        // `[u8; 32]`), so every bit pattern is a valid `MetadataContentArgs` — the length check
        // above is sufficient to make this cast sound.
        Ok(unsafe { Self::from_bytes_unchecked(bytes) })
    }

    /// Bounds-check `name_len`/`symbol_len`/`uri_len` against the buffer capacities the on-chain
    /// caller controls (these are `u8` lengths into fixed-size arrays that are themselves already
    /// exactly `MAX_*_LEN` long, so a length in range is always safe to slice — but content
    /// validation, e.g. UTF-8 well-formedness, still happens in `checks::validate_metadata_content`).
    pub fn validate_lengths(&self) -> Result<(), ProgramError> {
        if self.name_len as usize > MAX_NAME_LEN {
            return Err(TokenMetadataError::NameTooLong.into());
        }
        if self.symbol_len as usize > MAX_SYMBOL_LEN {
            return Err(TokenMetadataError::SymbolTooLong.into());
        }
        if self.uri_len as usize > MAX_URI_LEN {
            return Err(TokenMetadataError::UriTooLong.into());
        }
        Ok(())
    }
}

/// `CreateMetadata` instruction data.
#[repr(C)]
#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct CreateMetadataArgs {
    /// Bump for the `["metadata", mint, bump]` PDA, computed off-chain (e.g. via
    /// `findProgramAddressSync`). See `pda::derive_metadata_address` for why a non-canonical
    /// value here poses no cross-account security risk.
    pub bump: u8,
    /// Update authority to record on the new metadata. Does not need to match the mint
    /// authority that signs for creation (e.g. a token creator may want a multisig or DAO as the
    /// update authority from the start) and does not itself need to sign this instruction.
    pub initial_update_authority: Address,
    pub content: MetadataContentArgs,
}

const _: () = assert!(size_of::<CreateMetadataArgs>() == 446);

impl CreateMetadataArgs {
    pub const LEN: usize = size_of::<Self>();

    pub fn from_bytes(bytes: &[u8]) -> Result<&Self, ProgramError> {
        if bytes.len() != Self::LEN {
            return Err(TokenMetadataError::InvalidInstructionData.into());
        }
        Ok(unsafe { &*(bytes.as_ptr() as *const Self) })
    }
}

/// `UpdateMetadata` instruction data.
pub type UpdateMetadataArgs = MetadataContentArgs;

/// `SetUpdateAuthority` instruction data.
#[repr(C)]
#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct SetUpdateAuthorityArgs {
    /// `1` to renounce (set update authority to the all-zero address, permanently), `0` to
    /// transfer to `new_update_authority`. Kept as an explicit flag rather than overloading the
    /// all-zero address as "renounce" so the on-chain instruction data can never be ambiguous
    /// about caller intent.
    pub renounce: u8,
    pub new_update_authority: Address,
}

const _: () = assert!(size_of::<SetUpdateAuthorityArgs>() == 33);

impl SetUpdateAuthorityArgs {
    pub const LEN: usize = size_of::<Self>();

    pub fn from_bytes(bytes: &[u8]) -> Result<&Self, ProgramError> {
        if bytes.len() != Self::LEN {
            return Err(TokenMetadataError::InvalidInstructionData.into());
        }
        Ok(unsafe { &*(bytes.as_ptr() as *const Self) })
    }
}
