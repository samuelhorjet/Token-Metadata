use pinocchio::{Address, AccountView, ProgramResult};

use crate::{
    instruction::{split_discriminator, TokenMetadataInstruction},
    processor,
};

pub fn process_instruction(
    program_id: &Address,
    accounts: &mut [AccountView],
    instruction_data: &[u8],
) -> ProgramResult {
    let (instruction, data) = split_discriminator(instruction_data)?;

    match instruction {
        TokenMetadataInstruction::CreateMetadata => processor::process_create(program_id, accounts, data),
        TokenMetadataInstruction::UpdateMetadata => processor::process_update(program_id, accounts, data),
        TokenMetadataInstruction::SetUpdateAuthority => {
            processor::process_set_update_authority(program_id, accounts, data)
        }
        TokenMetadataInstruction::SetImmutable => processor::process_set_immutable(program_id, accounts),
        TokenMetadataInstruction::VerifyCreator => processor::process_verify_creator(program_id, accounts),
        TokenMetadataInstruction::UnverifyCreator => {
            processor::process_unverify_creator(program_id, accounts)
        }
        TokenMetadataInstruction::CloseMetadata => processor::process_close(program_id, accounts),
    }
}
