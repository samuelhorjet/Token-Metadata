use core::mem::size_of;

use pinocchio::{
    account::{Ref, RefMut},
    error::ProgramError,
    AccountView, Address,
};

use crate::error::TokenMetadataError;

/// Maximum length of `name`, in bytes (UTF-8).
pub const MAX_NAME_LEN: usize = 32;
/// Maximum length of `symbol`, in bytes (UTF-8).
pub const MAX_SYMBOL_LEN: usize = 10;
/// Maximum length of `uri`, in bytes (UTF-8).
pub const MAX_URI_LEN: usize = 200;
/// Maximum number of creators.
pub const MAX_CREATORS: usize = 5;
/// Maximum total of creator `share` values.
pub const MAX_CREATOR_SHARE_TOTAL: u16 = 100;
/// Maximum `royalty_bps` value (100%).
pub const MAX_ROYALTY_BASIS_POINTS: u16 = 10_000;

/// Discriminator value for an account that has not been initialized yet
/// (i.e. a freshly allocated, zeroed PDA).
pub const DISCRIMINATOR_UNINITIALIZED: [u8; 8] = [0; 8];
/// Discriminator value for an initialized [`TokenMetadata`] account: the first 8 bytes of
/// `sha256("account:TokenMetadata")` — the same Anchor-style account-sighash convention used for
/// instruction discriminators, see `instruction::discriminator`.
pub const DISCRIMINATOR_TOKEN_METADATA_V1: [u8; 8] = [0xed, 0xd7, 0x84, 0xb6, 0x18, 0x7f, 0xaf, 0xad];

/// Kind of token this metadata describes, inferred from the mint's decimals at creation time.
#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TokenKind {
    /// A mint with `decimals > 0`, used as a divisible currency-like token.
    Fungible = 0,
    /// A mint with `decimals == 0`, used as a whole-unit countable token (e.g. game items).
    FungibleAsset = 1,
}

impl TokenKind {
    #[inline(always)]
    pub const fn from_decimals(decimals: u8) -> Self {
        if decimals == 0 {
            TokenKind::FungibleAsset
        } else {
            TokenKind::Fungible
        }
    }
}

/// A single creator entry: an address, whether that address has verified its inclusion, and its
/// share (in whole percentage points) of attribution.
///
/// `verified` can only be flipped by the creator's own signature (see the `VerifyCreator` and
/// `UnverifyCreator` instructions) — no other authority, including the update authority, may set
/// it on a creator's behalf. This is a deliberate simplification over `mpl-token-metadata`, which
/// allows the update authority to self-verify only when it is itself one of the listed creators;
/// here that ambiguity is removed entirely.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, bytemuck::Pod, bytemuck::Zeroable)]
pub struct CreatorEntry {
    pub address: Address,
    pub verified: u8,
    pub share: u8,
}

impl CreatorEntry {
    pub const LEN: usize = size_of::<Self>();

    #[inline(always)]
    pub const fn is_verified(&self) -> bool {
        self.verified != 0
    }
}

const _: () = assert!(CreatorEntry::LEN == 34);

/// On-chain metadata record for a single SPL Token / Token-2022 mint.
///
/// This struct is fully packed (every field has alignment 1) so it can be read and written
/// directly from raw account bytes via pointer casts — no serialization/deserialization step,
/// no heap allocation. All variable-length fields (`name`, `symbol`, `uri`, `creators`) use a
/// fixed maximum capacity plus an explicit length prefix rather than a dynamically sized
/// encoding, so the account never needs to be resized after creation.
#[repr(C)]
#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct TokenMetadata {
    /// Account discriminator; see `DISCRIMINATOR_*` constants.
    pub discriminator: [u8; 8],
    /// Bump seed of the `["metadata", mint]` PDA.
    pub bump: u8,
    /// `1` if the metadata can still be updated/closed, `0` if permanently immutable.
    pub is_mutable: u8,
    /// See [`TokenKind`].
    pub token_kind: u8,
    /// Authority allowed to update or close this metadata. All-zero once renounced.
    pub update_authority: Address,
    /// The mint this metadata describes.
    pub mint: Address,
    /// Royalty basis points (0-10_000). Informational only; not enforced by this program.
    pub royalty_bps: [u8; 2],
    pub name_len: u8,
    pub name: [u8; MAX_NAME_LEN],
    pub symbol_len: u8,
    pub symbol: [u8; MAX_SYMBOL_LEN],
    pub uri_len: u8,
    pub uri: [u8; MAX_URI_LEN],
    pub creator_count: u8,
    pub creators: [CreatorEntry; MAX_CREATORS],
}

impl TokenMetadata {
    pub const LEN: usize = size_of::<Self>();

    /// Borrow an already-initialized metadata account immutably, verifying ownership, size and
    /// discriminator.
    pub fn from_account_view<'a>(
        account: &'a AccountView,
        program_id: &Address,
    ) -> Result<Ref<'a, TokenMetadata>, ProgramError> {
        if !account.owned_by(program_id) {
            return Err(TokenMetadataError::IncorrectMetadataOwner.into());
        }
        if account.data_len() != Self::LEN {
            return Err(ProgramError::InvalidAccountData);
        }

        let data = account.try_borrow()?;
        if data[0..8] != DISCRIMINATOR_TOKEN_METADATA_V1 {
            return Err(TokenMetadataError::UninitializedMetadata.into());
        }

        Ok(Ref::map(data, |bytes| unsafe { Self::from_bytes_unchecked(bytes) }))
    }

    /// Borrow an already-initialized metadata account mutably, verifying ownership, size and
    /// discriminator.
    pub fn from_account_view_mut<'a>(
        account: &'a mut AccountView,
        program_id: &Address,
    ) -> Result<RefMut<'a, TokenMetadata>, ProgramError> {
        if !account.owned_by(program_id) {
            return Err(TokenMetadataError::IncorrectMetadataOwner.into());
        }
        if account.data_len() != Self::LEN {
            return Err(ProgramError::InvalidAccountData);
        }

        let data = account.try_borrow_mut()?;
        if data[0..8] != DISCRIMINATOR_TOKEN_METADATA_V1 {
            return Err(TokenMetadataError::UninitializedMetadata.into());
        }

        Ok(RefMut::map(data, |bytes| unsafe {
            Self::from_bytes_unchecked_mut(bytes)
        }))
    }

    /// Borrow a freshly allocated (all-zero), not-yet-initialized metadata account mutably.
    ///
    /// Verifies ownership and size, but requires the discriminator to be `0` (uninitialized)
    /// rather than `DISCRIMINATOR_TOKEN_METADATA_V1`, to prevent re-initializing (and thereby
    /// resetting authority checks on) an existing metadata account.
    pub fn from_uninitialized_account_view_mut<'a>(
        account: &'a mut AccountView,
        program_id: &Address,
    ) -> Result<RefMut<'a, TokenMetadata>, ProgramError> {
        if !account.owned_by(program_id) {
            return Err(TokenMetadataError::IncorrectMetadataOwner.into());
        }
        if account.data_len() != Self::LEN {
            return Err(ProgramError::InvalidAccountData);
        }

        let data = account.try_borrow_mut()?;
        if data[0..8] != DISCRIMINATOR_UNINITIALIZED {
            return Err(TokenMetadataError::AlreadyInitialized.into());
        }

        Ok(RefMut::map(data, |bytes| unsafe {
            Self::from_bytes_unchecked_mut(bytes)
        }))
    }

    /// # Safety
    /// `bytes` must be at least `Self::LEN` long and valid for reads for the lifetime of the
    /// returned reference (every bit pattern is a valid `TokenMetadata`, since all fields are
    /// byte arrays or `Address`, so this is safe as long as the length precondition holds).
    #[inline(always)]
    unsafe fn from_bytes_unchecked(bytes: &[u8]) -> &Self {
        &*(bytes.as_ptr() as *const TokenMetadata)
    }

    /// # Safety
    /// See [`Self::from_bytes_unchecked`]; additionally requires unique (mutable) access to
    /// `bytes` for the lifetime of the returned reference.
    #[inline(always)]
    unsafe fn from_bytes_unchecked_mut(bytes: &mut [u8]) -> &mut Self {
        &mut *(bytes.as_mut_ptr() as *mut TokenMetadata)
    }

    #[inline(always)]
    pub const fn is_mutable(&self) -> bool {
        self.is_mutable != 0
    }

    #[inline(always)]
    pub fn set_mutable(&mut self, mutable: bool) {
        self.is_mutable = mutable as u8;
    }

    #[inline(always)]
    pub fn royalty_bps(&self) -> u16 {
        u16::from_le_bytes(self.royalty_bps)
    }

    #[inline(always)]
    pub fn set_royalty_bps(&mut self, bps: u16) {
        self.royalty_bps = bps.to_le_bytes();
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
    pub fn creators(&self) -> &[CreatorEntry] {
        &self.creators[..self.creator_count as usize]
    }

    #[inline(always)]
    pub fn creators_mut(&mut self) -> &mut [CreatorEntry] {
        let count = self.creator_count as usize;
        &mut self.creators[..count]
    }

    /// Overwrite `name`/`symbol`/`uri`, zeroing unused tail capacity so no stale bytes from a
    /// previous, longer value remain readable.
    pub fn set_content(&mut self, name: &[u8], symbol: &[u8], uri: &[u8]) {
        self.name = [0u8; MAX_NAME_LEN];
        self.name[..name.len()].copy_from_slice(name);
        self.name_len = name.len() as u8;

        self.symbol = [0u8; MAX_SYMBOL_LEN];
        self.symbol[..symbol.len()].copy_from_slice(symbol);
        self.symbol_len = symbol.len() as u8;

        self.uri = [0u8; MAX_URI_LEN];
        self.uri[..uri.len()].copy_from_slice(uri);
        self.uri_len = uri.len() as u8;
    }

    /// Overwrite the creators list, zeroing unused tail capacity.
    pub fn set_creators(&mut self, creators: &[CreatorEntry]) {
        self.creators = [CreatorEntry {
            address: Address::default(),
            verified: 0,
            share: 0,
        }; MAX_CREATORS];
        self.creators[..creators.len()].copy_from_slice(creators);
        self.creator_count = creators.len() as u8;
    }
}

const _: () = assert!(TokenMetadata::LEN == 493);
