#![no_std]
// `target_os = "solana"` is a real, valid cfg value on the actual SBF/SBPF build (set by the
// platform tools), but isn't in rustc's built-in list of known `target_os` values, so host-target
// checks (`cargo check`/`clippy` without the Solana toolchain) flag every `target_os = "solana"`
// guard as an "unexpected cfg value" — including ones inside pinocchio's own macros. Harmless.
#![allow(unexpected_cfgs)]

pub mod checks;
mod entrypoint;
pub mod error;
pub mod instruction;
pub mod pda;
mod processor;
pub mod state;

// Placeholder program ID — replace with the real deployed program's keypair-derived address
// before mainnet/devnet deployment. Generated only to be a syntactically valid, non-colliding
// 32-byte address for local development.
solana_address::declare_id!("ArBZw7qnKJhMycbcf7UoNke8GALbGvQrEqvQFampB8wa");

#[cfg(not(feature = "no-entrypoint"))]
pinocchio::program_entrypoint!(entrypoint::process_instruction);
#[cfg(not(feature = "no-entrypoint"))]
pinocchio::no_allocator!();
#[cfg(not(feature = "no-entrypoint"))]
pinocchio::nostd_panic_handler!();
