use pinocchio::{error::ProgramError, Address};
use solana_address::PDA_MARKER;

use crate::error::TokenMetadataError;

/// Seed prefix for the `["metadata", mint, bump]` PDA.
pub const METADATA_SEED: &[u8] = b"metadata";

/// Derive the metadata address for `mint` using `bump`, without the ed25519 on-curve exclusion
/// check that `create_program_address`/`find_program_address` normally perform.
///
/// A program-derived address is defined as `sha256(seeds || bump || program_id ||
/// "ProgramDerivedAddress")`, additionally required by the Solana runtime's syscalls to fall
/// *off* the ed25519 curve (i.e. not be a valid public key with a known private key), which is
/// what makes it safe for a program to sign for it via CPI. Skipping that extra check here is
/// safe for our purposes: we only ever use the output of this function to *compare* against an
/// account key that was independently supplied and already constrained elsewhere (owned by this
/// program, correct discriminator, etc.) — we never use it to sign a CPI. Even in the
/// astronomically unlikely case the hash lands on-curve, nobody can derive a matching private key
/// for it (that would require solving the ed25519 discrete log problem, an unrelated and
/// intractable problem from inverting a SHA256 hash). This is the same optimization used by
/// `pinocchio-pubkey::derive_address` and other production Solana frameworks to avoid the
/// ~1,000-1,500 CU cost of the syscall-based derivation on every instruction.
///
/// # Bump trust
/// The caller must source `bump` from a trusted place:
/// - For an *existing* metadata account, from `TokenMetadata::bump`, persisted at creation.
/// - For `CreateMetadata`, from the instruction's caller-supplied bump. A non-canonical bump
///   there does not create a cross-account security issue: creation is already gated on the
///   signer being the mint's current authority (see `processor::create`), so at worst a mint
///   authority picks a non-standard address for *their own* token's metadata, which only affects
///   discoverability by conventional indexers, not correctness or authorization.
pub fn derive_metadata_address(mint: &Address, bump: u8, program_id: &Address) -> Address {
    let bump_seed = [bump];
    // `sol_sha256` hashes the concatenation of each `(ptr, len)` pair it is given, treating the
    // input as an array of such pairs — which is exactly the in-memory layout of `&[&[u8]]` on
    // this target.
    let segments: [&[u8]; 5] = [
        METADATA_SEED,
        mint.as_ref(),
        &bump_seed,
        program_id.as_ref(),
        PDA_MARKER.as_ref(),
    ];

    #[cfg(any(target_os = "solana", target_arch = "bpf"))]
    {
        let mut out = core::mem::MaybeUninit::<[u8; 32]>::uninit();
        unsafe {
            pinocchio::syscalls::sol_sha256(
                segments.as_ptr() as *const u8,
                segments.len() as u64,
                out.as_mut_ptr() as *mut u8,
            );
            Address::from(out.assume_init())
        }
    }

    #[cfg(not(any(target_os = "solana", target_arch = "bpf")))]
    {
        // `sol_sha256` is only available when actually running on-chain; off-chain callers
        // (tests, tooling) should derive addresses using a standard SHA256 implementation
        // instead, mirroring `pinocchio_pubkey::derive_address`'s own host-target behavior.
        let _ = segments;
        unreachable!("derive_metadata_address is only available when compiled for target `solana`")
    }
}

/// Verify that `candidate` is the metadata address for `mint` derived with `bump`.
///
/// See [`derive_metadata_address`] for the derivation and its safety/trust requirements.
pub fn verify_metadata_address(
    mint: &Address,
    bump: u8,
    program_id: &Address,
    candidate: &Address,
) -> Result<(), ProgramError> {
    if &derive_metadata_address(mint, bump, program_id) != candidate {
        return Err(TokenMetadataError::InvalidMetadataAddress.into());
    }
    Ok(())
}
