//! Shared test fixtures and instruction builders for the `token-metadata` integration suite.
//!
//! All pubkey-shaped types here are `solana_address::Address` — the *same* type as
//! `solana_keypair`/`solana_instruction`/`solana_message`/`litesvm`'s own `Address` (all resolve
//! to `solana-address 2.7.x` in this workspace), and the same type our on-chain program itself
//! uses (`pinocchio::Address` is a re-export of it too). No manual pubkey conversions needed
//! anywhere in these tests.
//!
//! `dead_code` is allowed at the module level: this file is compiled fresh into *each*
//! `tests/*.rs` integration-test binary (Cargo's per-file-is-its-own-crate model), and any given
//! test file only calls a subset of these helpers — genuinely unused-everywhere helpers would
//! still be caught by whichever test file relies on them being correct.
#![allow(dead_code)]

use litesvm::LiteSVM;
use solana_account::Account;
use solana_address::Address;
use solana_instruction::{AccountMeta, Instruction};
use solana_instruction::error::InstructionError;
use solana_keypair::Keypair;
use solana_message::Message;
use solana_signer::Signer;
use solana_transaction::Transaction;
use solana_transaction_error::TransactionError;

use token_metadata::{
    instruction::{discriminator, CreateMetadataArgs, CreatorInput, MetadataContentArgs, SetUpdateAuthorityArgs},
    state::{TokenMetadata, MAX_CREATORS, MAX_NAME_LEN, MAX_SYMBOL_LEN, MAX_URI_LEN},
};

pub fn program_id() -> Address {
    token_metadata::ID
}

/// Boots a fresh `LiteSVM` instance with our compiled program loaded from
/// `target/deploy/token_metadata.so` (built via `cargo build-sbf` beforehand).
pub fn setup() -> LiteSVM {
    let mut svm = LiteSVM::new();

    let mut so_path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    so_path.pop(); // program/ -> workspace root
    so_path.push("target/deploy/token_metadata.so");

    svm.add_program_from_file(program_id(), &so_path)
        .unwrap_or_else(|e| {
            panic!(
                "failed to load {so_path:?}: {e:?}\n\
                 Build it first: cargo build-sbf --manifest-path program/Cargo.toml"
            )
        });

    svm
}

pub fn funded_keypair(svm: &mut LiteSVM) -> Keypair {
    let kp = Keypair::new();
    svm.airdrop(&kp.pubkey(), 10_000_000_000).unwrap();
    kp
}

/// Raw classic SPL Token `Mint` account data (82 bytes, `Pack` layout):
/// `mint_authority: COption<Pubkey>` (36) + `supply: u64` (8) + `decimals: u8` (1) +
/// `is_initialized: bool` (1) + `freeze_authority: COption<Pubkey>` (36).
pub fn mint_account_data(mint_authority: Option<Address>, decimals: u8) -> Vec<u8> {
    let mut data = vec![0u8; 82];
    if let Some(auth) = mint_authority {
        data[0..4].copy_from_slice(&1u32.to_le_bytes());
        data[4..36].copy_from_slice(auth.as_ref());
    }
    data[44] = decimals;
    data[45] = 1; // is_initialized
    data
}

/// Raw Token-2022 mint account data. Same 82-byte base layout as classic SPL Token (identical
/// `Pack` layout), optionally extended with a `MetadataPointer` extension.
///
/// Extension TLV data starts at `Account::BASE_LEN` (165 — *not* `Mint::BASE_LEN` 82; Token-2022
/// aligns the extension-start offset for mints and token accounts to the same fixed value), after
/// a single `AccountType::Mint` (`1`) marker byte, i.e. at absolute offset 166. `MetadataPointer`'s
/// extension type discriminant is `18`, and its value is 64 bytes: `authority` (32, zero = None)
/// followed by `metadata_address` (32, zero = None). See `token_metadata::state::mint_extensions`
/// for the on-chain reader this mirrors.
pub fn mint2022_account_data(
    mint_authority: Option<Address>,
    decimals: u8,
    metadata_pointer: Option<(Option<Address>, Option<Address>)>,
) -> Vec<u8> {
    let mut data = vec![0u8; 82];
    if let Some(auth) = mint_authority {
        data[0..4].copy_from_slice(&1u32.to_le_bytes());
        data[4..36].copy_from_slice(auth.as_ref());
    }
    data[44] = decimals;
    data[45] = 1;

    if let Some((authority, metadata_address)) = metadata_pointer {
        data.resize(165, 0);
        data.push(1); // AccountType::Mint
        data.extend_from_slice(&18u16.to_le_bytes()); // ExtensionType::MetadataPointer
        data.extend_from_slice(&64u16.to_le_bytes()); // value length
        data.extend_from_slice(authority.map(|a| *a.as_array()).unwrap_or([0u8; 32]).as_ref());
        data.extend_from_slice(
            metadata_address
                .map(|a| *a.as_array())
                .unwrap_or([0u8; 32])
                .as_ref(),
        );
    }
    data
}

pub fn set_account(svm: &mut LiteSVM, address: Address, owner: Address, data: Vec<u8>, lamports: u64) {
    svm.set_account(
        address,
        Account {
            lamports,
            data,
            owner,
            executable: false,
            rent_epoch: 0,
        },
    )
    .unwrap();
}

const SYSTEM_PROGRAM_ID: Address = pinocchio_system::ID;

/// Raw `system_program::CreateAccount` instruction data: `[discriminator: u32 LE = 0][lamports:
/// u64 LE][space: u64 LE][owner: 32 bytes]` — verified against `pinocchio_system`'s own
/// `CreateAccount` CPI builder (`program/src/processor/create.rs` uses the same crate on-chain).
fn create_account_ix(from: Address, to: Address, lamports: u64, space: u64, owner: Address) -> Instruction {
    let mut data = Vec::with_capacity(4 + 8 + 8 + 32);
    data.extend_from_slice(&0u32.to_le_bytes());
    data.extend_from_slice(&lamports.to_le_bytes());
    data.extend_from_slice(&space.to_le_bytes());
    data.extend_from_slice(owner.as_ref());

    Instruction {
        program_id: SYSTEM_PROGRAM_ID,
        accounts: vec![AccountMeta::new(from, true), AccountMeta::new(to, true)],
        data,
    }
}

/// Raw classic-SPL-Token/Token-2022 `InitializeMint2` instruction data: `[discriminator: u8 =
/// 20][decimals: u8][mint_authority: 32 bytes][freeze_authority_tag: u8][freeze_authority: 32
/// bytes if tag == 1]`. Verified directly against `spl-token-interface 2.0.0`'s
/// `TokenInstruction::pack()`/`unpack()` (`InitializeMint2` variant, discriminant `20`) — note the
/// 1-byte `Option` tag here is *not* the same encoding as the 4-byte `COption` tag used inside the
/// `Mint` *account's own* on-chain layout (see `mint_account_data` above); instruction-data options
/// and account-state `COption`s are encoded differently by this program.
fn initialize_mint2_ix(
    token_program_id: Address,
    mint: Address,
    mint_authority: Address,
    freeze_authority: Option<Address>,
    decimals: u8,
) -> Instruction {
    let mut data = Vec::with_capacity(2 + 32 + 1 + 32);
    data.push(20);
    data.push(decimals);
    data.extend_from_slice(mint_authority.as_ref());
    match freeze_authority {
        Some(addr) => {
            data.push(1);
            data.extend_from_slice(addr.as_ref());
        }
        None => data.push(0),
    }

    Instruction {
        program_id: token_program_id,
        accounts: vec![AccountMeta::new(mint, false)],
        data,
    }
}

/// Creates a *genuinely* initialized mint by actually CPI-ing into the real SPL Token (or
/// Token-2022) program — `system_program::CreateAccount` followed by `InitializeMint2` — rather
/// than hand-injecting raw account bytes (see `mint_account_data`/`mint2022_account_data`, which
/// test our program's own reading logic against byte-accurate but self-constructed fixtures).
/// This closes that gap: it proves `CreateMetadata` works against a mint the real token program
/// actually produced, not just against our understanding of its wire format.
pub fn create_real_mint(
    svm: &mut LiteSVM,
    payer: &Keypair,
    token_program_id: Address,
    mint_authority: Address,
    decimals: u8,
) -> Address {
    let mint = Keypair::new();
    let space = 82u64;
    let lamports = svm.minimum_balance_for_rent_exemption(space as usize);

    let create_ix = create_account_ix(payer.pubkey(), mint.pubkey(), lamports, space, token_program_id);
    let init_ix = initialize_mint2_ix(token_program_id, mint.pubkey(), mint_authority, None, decimals);

    svm.expire_blockhash();
    let blockhash = svm.latest_blockhash();
    let message = Message::new(&[create_ix, init_ix], Some(&payer.pubkey()));
    let signers: Vec<&Keypair> = vec![payer, &mint];
    let tx = Transaction::new(&signers, message, blockhash);

    let result = svm.send_transaction(tx);
    let meta = result.unwrap_or_else(|failed| {
        panic!("real mint creation should succeed: {:?} (logs: {:?})", failed.err, failed.meta.logs)
    });
    println!(
        "create_real_mint ({}): compute_units_consumed = {}",
        mint.pubkey(),
        meta.compute_units_consumed
    );
    for line in &meta.logs {
        println!("  {line}");
    }

    mint.pubkey()
}

/// Derives the metadata PDA and its canonical bump for `mint`, using the standard off-chain
/// `find_program_address` search — this always lands on the same address/bump our own on-chain
/// `pda::derive_metadata_address` would accept, since both implement the identical
/// `sha256(seeds || bump || program_id || "ProgramDerivedAddress")` formula (ours just skips the
/// runtime's redundant on-curve re-check for cost reasons — see `pda.rs`).
pub fn metadata_pda(mint: &Address) -> (Address, u8) {
    Address::find_program_address(&[b"metadata", mint.as_ref()], &program_id())
}

fn fixed_bytes<const N: usize>(s: &[u8]) -> (u8, [u8; N]) {
    assert!(s.len() <= N);
    let mut buf = [0u8; N];
    buf[..s.len()].copy_from_slice(s);
    (s.len() as u8, buf)
}

pub struct CreatorSpec {
    pub address: Address,
    pub share: u8,
}

/// Builds a `MetadataContentArgs` payload from friendly inputs, zero-padding name/symbol/uri to
/// their fixed on-chain capacities exactly like the real program does.
pub fn content_args(name: &str, symbol: &str, uri: &str, royalty_bps: u16, creators: &[CreatorSpec]) -> MetadataContentArgs {
    assert!(creators.len() <= MAX_CREATORS);

    let (name_len, name) = fixed_bytes::<MAX_NAME_LEN>(name.as_bytes());
    let (symbol_len, symbol) = fixed_bytes::<MAX_SYMBOL_LEN>(symbol.as_bytes());
    let (uri_len, uri) = fixed_bytes::<MAX_URI_LEN>(uri.as_bytes());

    let mut creator_buf = [CreatorInput {
        address: Address::default(),
        share: 0,
    }; MAX_CREATORS];
    for (i, c) in creators.iter().enumerate() {
        creator_buf[i] = CreatorInput {
            address: c.address,
            share: c.share,
        };
    }

    MetadataContentArgs {
        royalty_bps: royalty_bps.to_le_bytes(),
        name_len,
        name,
        symbol_len,
        symbol,
        uri_len,
        uri,
        creator_count: creators.len() as u8,
        creators: creator_buf,
    }
}

fn ix_data<T: bytemuck::Pod>(tag: [u8; 8], args: &T) -> Vec<u8> {
    let mut data = tag.to_vec();
    data.extend_from_slice(bytemuck::bytes_of(args));
    data
}

#[allow(clippy::too_many_arguments)]
pub fn build_create_ix(
    metadata: Address,
    mint: Address,
    mint_authority: Address,
    payer: Address,
    bump: u8,
    initial_update_authority: Address,
    content: MetadataContentArgs,
) -> Instruction {
    let args = CreateMetadataArgs {
        bump,
        initial_update_authority,
        content,
    };
    Instruction {
        program_id: program_id(),
        accounts: vec![
            AccountMeta::new(metadata, false),
            AccountMeta::new_readonly(mint, false),
            AccountMeta::new_readonly(mint_authority, true),
            AccountMeta::new(payer, true),
            AccountMeta::new_readonly(pinocchio_system::ID, false),
        ],
        data: ix_data(discriminator::CREATE_METADATA, &args),
    }
}

pub fn build_update_ix(
    metadata: Address,
    mint: Address,
    update_authority: Address,
    content: MetadataContentArgs,
) -> Instruction {
    Instruction {
        program_id: program_id(),
        accounts: vec![
            AccountMeta::new(metadata, false),
            AccountMeta::new_readonly(mint, false),
            AccountMeta::new_readonly(update_authority, true),
        ],
        data: ix_data(discriminator::UPDATE_METADATA, &content),
    }
}

pub fn build_set_update_authority_ix(
    metadata: Address,
    current_authority: Address,
    renounce: bool,
    new_update_authority: Address,
) -> Instruction {
    let args = SetUpdateAuthorityArgs {
        renounce: renounce as u8,
        new_update_authority,
    };
    Instruction {
        program_id: program_id(),
        accounts: vec![
            AccountMeta::new(metadata, false),
            AccountMeta::new_readonly(current_authority, true),
        ],
        data: ix_data(discriminator::SET_UPDATE_AUTHORITY, &args),
    }
}

pub fn build_set_immutable_ix(metadata: Address, update_authority: Address) -> Instruction {
    Instruction {
        program_id: program_id(),
        accounts: vec![
            AccountMeta::new(metadata, false),
            AccountMeta::new_readonly(update_authority, true),
        ],
        data: discriminator::SET_IMMUTABLE.to_vec(),
    }
}

pub fn build_verify_creator_ix(metadata: Address, creator: Address) -> Instruction {
    Instruction {
        program_id: program_id(),
        accounts: vec![
            AccountMeta::new(metadata, false),
            AccountMeta::new_readonly(creator, true),
        ],
        data: discriminator::VERIFY_CREATOR.to_vec(),
    }
}

pub fn build_unverify_creator_ix(metadata: Address, creator: Address) -> Instruction {
    Instruction {
        program_id: program_id(),
        accounts: vec![
            AccountMeta::new(metadata, false),
            AccountMeta::new_readonly(creator, true),
        ],
        data: discriminator::UNVERIFY_CREATOR.to_vec(),
    }
}

pub fn build_close_ix(
    metadata: Address,
    mint: Address,
    update_authority: Address,
    destination: Address,
) -> Instruction {
    Instruction {
        program_id: program_id(),
        accounts: vec![
            AccountMeta::new(metadata, false),
            AccountMeta::new_readonly(mint, false),
            AccountMeta::new_readonly(update_authority, true),
            AccountMeta::new(destination, false),
        ],
        data: discriminator::CLOSE_METADATA.to_vec(),
    }
}

/// Sends `ix` as a transaction paid and signed by `payer`, plus any additional required signers.
///
/// Always expires the blockhash first: tests frequently send two otherwise-identical
/// instructions in a row (e.g. calling the same one-way-latch instruction twice to confirm it
/// rejects the second call) — without a fresh blockhash, the second submission is byte-identical
/// to the first and LiteSVM treats it as a duplicate (`AlreadyProcessed`) before our program ever
/// runs a second time, masking whatever the test actually meant to exercise.
///
/// Also prints the transaction's logs and compute units consumed (visible with
/// `cargo test -- --show-output`), so CU cost per instruction is easy to eyeball per test.
pub fn send(svm: &mut LiteSVM, payer: &Keypair, extra_signers: &[&Keypair], ix: Instruction) -> litesvm::types::TransactionResult {
    svm.expire_blockhash();
    let blockhash = svm.latest_blockhash();
    let message = Message::new(&[ix], Some(&payer.pubkey()));

    let mut signers: Vec<&Keypair> = vec![payer];
    signers.extend_from_slice(extra_signers);

    let tx = Transaction::new(&signers, message, blockhash);
    let result = svm.send_transaction(tx);

    match &result {
        Ok(meta) => {
            println!("compute_units_consumed = {}", meta.compute_units_consumed);
            for line in &meta.logs {
                println!("  {line}");
            }
        }
        Err(failed) => {
            println!(
                "compute_units_consumed = {} (failed: {:?})",
                failed.meta.compute_units_consumed, failed.err
            );
            for line in &failed.meta.logs {
                println!("  {line}");
            }
        }
    }

    result
}

/// Reads back a `TokenMetadata` account and casts it directly (zero-copy) via `bytemuck` — the
/// exact same POD layout the on-chain program itself reads/writes, so this doubles as an
/// end-to-end check that the account's on-chain byte layout matches the Rust type.
pub fn read_metadata(svm: &LiteSVM, metadata: &Address) -> TokenMetadata {
    let account = svm
        .get_account(metadata)
        .expect("metadata account should exist");
    assert_eq!(account.data.len(), TokenMetadata::LEN, "unexpected metadata account size");
    *bytemuck::from_bytes::<TokenMetadata>(&account.data)
}

/// Asserts `result` failed with exactly the given `Custom` program error code — not just "any
/// error" — matching the on-chain `TokenMetadataError` variant's `as u32` discriminant.
pub fn assert_custom_error(result: litesvm::types::TransactionResult, expected: u32) {
    match result {
        Err(failed) => match failed.err {
            TransactionError::InstructionError(_, InstructionError::Custom(code)) => {
                assert_eq!(code, expected, "wrong custom error code (logs: {:?})", failed.meta.logs);
            }
            other => panic!("expected Custom({expected}), got {other:?} (logs: {:?})", failed.meta.logs),
        },
        Ok(meta) => panic!("expected transaction to fail with Custom({expected}), it succeeded (logs: {:?})", meta.logs),
    }
}
