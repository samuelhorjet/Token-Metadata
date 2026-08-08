use pinocchio::{error::ProgramError, AccountView, Address};

use crate::{
    error::TokenMetadataError,
    state::{
        CreatorEntry, TokenMetadata, MAX_CREATORS, MAX_CREATOR_SHARE_TOTAL, MAX_NAME_LEN,
        MAX_ROYALTY_BASIS_POINTS, MAX_SYMBOL_LEN, MAX_URI_LEN,
    },
};

/// Which token program owns a validated mint.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TokenProgramKind {
    Token,
    Token2022,
}

/// The subset of mint fields relevant to metadata creation/validation, read without a full
/// `spl-token`-style unpack (works uniformly for classic SPL Token mints and Token-2022 mints,
/// including ones with extensions, since both share an identical base layout).
pub struct MintInfo {
    pub mint_authority: Option<Address>,
    pub decimals: u8,
    pub program: TokenProgramKind,
}

/// Validate that `mint_account` is owned by either the SPL Token or Token-2022 program, is a
/// properly laid out, initialized mint, and return its authority/decimals/program kind.
///
/// This is the dual-token-program gate every instruction that touches a mint relies on: it is the
/// only place that decides whether a given account is treated as a legitimate mint at all.
pub fn read_mint(mint_account: &AccountView) -> Result<MintInfo, ProgramError> {
    if mint_account.owner() == &pinocchio_token::ID {
        let mint = pinocchio_token::state::Mint::from_account_view(mint_account)
            .map_err(|_| TokenMetadataError::InvalidMint)?;
        Ok(MintInfo {
            mint_authority: mint.mint_authority().copied(),
            decimals: mint.decimals(),
            program: TokenProgramKind::Token,
        })
    } else if mint_account.owner() == &pinocchio_token_2022::ID {
        let mint = pinocchio_token_2022::state::Mint::from_account_view(mint_account)
            .map_err(|_| TokenMetadataError::InvalidMint)?;
        Ok(MintInfo {
            mint_authority: mint.mint_authority().copied(),
            decimals: mint.decimals(),
            program: TokenProgramKind::Token2022,
        })
    } else {
        Err(TokenMetadataError::IncorrectMintOwner.into())
    }
}

/// Require `account` to be a transaction signer.
#[inline(always)]
pub fn assert_signer(account: &AccountView) -> Result<(), ProgramError> {
    if !account.is_signer() {
        return Err(TokenMetadataError::MissingRequiredSignature.into());
    }
    Ok(())
}

/// Require `account` to be writable.
#[inline(always)]
pub fn assert_writable(account: &AccountView) -> Result<(), ProgramError> {
    if !account.is_writable() {
        return Err(ProgramError::InvalidAccountData);
    }
    Ok(())
}

/// Require `account.address()` to equal `expected`.
#[inline(always)]
pub fn assert_address(account: &AccountView, expected: &Address) -> Result<(), ProgramError> {
    if account.address() != expected {
        return Err(TokenMetadataError::IncorrectProgramId.into());
    }
    Ok(())
}

/// Require that `authority_account` is both the mint's recorded mint authority and a signer.
///
/// Mirrors `mpl-token-metadata`'s `assert_mint_authority_matches_mint`, minus its
/// Metaplex-Foundation "seed authority" escape hatch, which this program does not replicate.
pub fn assert_mint_authority_matches(
    mint_authority: Option<&Address>,
    authority_account: &AccountView,
) -> Result<(), ProgramError> {
    let mint_authority = mint_authority.ok_or(TokenMetadataError::MintHasNoAuthority)?;

    if mint_authority != authority_account.address() {
        return Err(TokenMetadataError::InvalidMintAuthority.into());
    }
    if !authority_account.is_signer() {
        return Err(TokenMetadataError::MintAuthorityNotSigner.into());
    }
    Ok(())
}

/// Require that `authority_account` is both the metadata's recorded update authority and a
/// signer.
pub fn assert_update_authority_is_correct(
    metadata: &TokenMetadata,
    authority_account: &AccountView,
) -> Result<(), ProgramError> {
    if &metadata.update_authority != authority_account.address() {
        return Err(TokenMetadataError::UpdateAuthorityIncorrect.into());
    }
    if !authority_account.is_signer() {
        return Err(TokenMetadataError::UpdateAuthorityNotSigner.into());
    }
    Ok(())
}

/// Require that the metadata is still mutable.
#[inline(always)]
pub fn assert_mutable(metadata: &TokenMetadata) -> Result<(), ProgramError> {
    if !metadata.is_mutable() {
        return Err(TokenMetadataError::DataIsImmutable.into());
    }
    Ok(())
}

/// Validate `name`/`symbol`/`uri` length and UTF-8 well-formedness, `royalty_bps` range, and the
/// creators list (count, no duplicate addresses, shares summing to exactly 100 when non-empty).
///
/// `verified` is intentionally not part of this check: creator verification can only ever be set
/// via the dedicated `VerifyCreator`/`UnverifyCreator` instructions (see [`CreatorEntry`]), so
/// `Create`/`Update` always write creators with `verified = false` regardless of caller input.
pub fn validate_metadata_content(
    name: &[u8],
    symbol: &[u8],
    uri: &[u8],
    royalty_bps: u16,
    creators: &[CreatorEntry],
) -> Result<(), ProgramError> {
    if name.len() > MAX_NAME_LEN {
        return Err(TokenMetadataError::NameTooLong.into());
    }
    if symbol.len() > MAX_SYMBOL_LEN {
        return Err(TokenMetadataError::SymbolTooLong.into());
    }
    if uri.len() > MAX_URI_LEN {
        return Err(TokenMetadataError::UriTooLong.into());
    }
    core::str::from_utf8(name).map_err(|_| TokenMetadataError::InvalidInstructionData)?;
    core::str::from_utf8(symbol).map_err(|_| TokenMetadataError::InvalidInstructionData)?;
    core::str::from_utf8(uri).map_err(|_| TokenMetadataError::InvalidInstructionData)?;

    if royalty_bps > MAX_ROYALTY_BASIS_POINTS {
        return Err(TokenMetadataError::InvalidRoyaltyBasisPoints.into());
    }

    if creators.len() > MAX_CREATORS {
        return Err(TokenMetadataError::TooManyCreators.into());
    }

    if !creators.is_empty() {
        for i in 0..creators.len() {
            for j in (i + 1)..creators.len() {
                if creators[i].address == creators[j].address {
                    return Err(TokenMetadataError::DuplicateCreatorAddress.into());
                }
            }
        }

        let mut share_total: u16 = 0;
        for creator in creators {
            share_total = share_total
                .checked_add(creator.share as u16)
                .ok_or(TokenMetadataError::NumericalOverflow)?;
        }
        if share_total != MAX_CREATOR_SHARE_TOTAL {
            return Err(TokenMetadataError::CreatorSharesMustSumTo100.into());
        }
    }

    Ok(())
}
