use pinocchio::error::ProgramError;

/// Program-specific errors, surfaced to clients as `ProgramError::Custom(n)`.
#[repr(u32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TokenMetadataError {
    /// Instruction data was too short or malformed for the requested instruction.
    InvalidInstructionData = 0,
    /// The instruction discriminator did not match any known instruction.
    UnknownInstruction,
    /// The mint account is not owned by the SPL Token or Token-2022 program.
    IncorrectMintOwner,
    /// The mint account failed basic layout/initialization checks.
    InvalidMint,
    /// The mint has no mint authority set, so metadata can never be attached to it.
    MintHasNoAuthority,
    /// The provided mint authority account does not match the mint's recorded authority.
    InvalidMintAuthority,
    /// The mint authority account was not a signer.
    MintAuthorityNotSigner,
    /// The metadata account is not owned by this program.
    IncorrectMetadataOwner,
    /// The metadata PDA does not match `["metadata", mint]` for the given mint/program.
    InvalidMetadataAddress,
    /// The metadata account's `mint` field does not match the supplied mint account.
    MintMismatch,
    /// The provided update authority does not match the metadata's recorded update authority.
    UpdateAuthorityIncorrect,
    /// The update authority account was not a signer.
    UpdateAuthorityNotSigner,
    /// `name` exceeds the maximum allowed length.
    NameTooLong,
    /// `symbol` exceeds the maximum allowed length.
    SymbolTooLong,
    /// `uri` exceeds the maximum allowed length.
    UriTooLong,
    /// `royalty_bps` exceeds 10_000 (100%).
    InvalidRoyaltyBasisPoints,
    /// More than the maximum number of creators (5) were supplied.
    TooManyCreators,
    /// The same creator address appears more than once.
    DuplicateCreatorAddress,
    /// Creator shares do not sum to exactly 100.
    CreatorSharesMustSumTo100,
    /// The metadata is immutable; no further changes are permitted.
    DataIsImmutable,
    /// `SetImmutable` was called on metadata that is already immutable.
    AlreadyImmutable,
    /// The signer is not one of the metadata's listed creators.
    SignerNotACreator,
    /// The metadata account is already initialized.
    AlreadyInitialized,
    /// The metadata account is not initialized.
    UninitializedMetadata,
    /// A Token-2022 mint declares a `MetadataPointer` extension that does not point at this
    /// metadata account.
    InvalidMetadataPointer,
    /// A required signer account was not a signer.
    MissingRequiredSignature,
    /// An account passed as a "known" program account did not match its expected program ID.
    IncorrectProgramId,
    /// A numeric operation would have overflowed.
    NumericalOverflow,
    /// The new update authority cannot be the default (all-zero) address; use the explicit
    /// `renounce` path instead so intent is unambiguous on-chain.
    InvalidNewUpdateAuthority,
}

impl From<TokenMetadataError> for ProgramError {
    fn from(e: TokenMetadataError) -> Self {
        ProgramError::Custom(e as u32)
    }
}
