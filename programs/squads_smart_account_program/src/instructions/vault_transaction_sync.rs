use anchor_lang::prelude::*;

use crate::{
    errors::*,
    state::*,
    utils::{ExecutableTransactionMessage, SynchronousTransactionMessage},
    SmallVec,
};

use super::CompiledInstruction;

#[derive(AnchorSerialize, AnchorDeserialize)]
pub struct VaultTransactionSyncArgs {
    pub vault_index: u8,
    /// The number of signers to reach threshold and adequate permissions
    pub num_signers: u8,
    /// Expected to be serialized as a SmallVec<u8, CompiledInstruction>
    pub instructions: Vec<u8>,
}

#[derive(Accounts)]
pub struct VaultTransactionSync<'info> {
    #[account(
        seeds = [SEED_PREFIX, SEED_MULTISIG, multisig.create_key.as_ref()],
        bump = multisig.bump,
    )]
    pub multisig: Box<Account<'info, Multisig>>,
    // `remaining_accounts` must include the following accounts in the exact order:
    // 1. The exact amount of signers required to reach the threshold
    // 2. Any remaining accounts associated with the instructions
}

impl VaultTransactionSync<'_> {
    fn validate(&self, num_signers: u8, remaining_accounts: &[AccountInfo]) -> Result<()> {
        let Self { multisig } = self;

        // Multisig must not be time locked
        require_eq!(multisig.time_lock, 0, MultisigError::TimeLockNotZero);

        // Get signers from remaining accounts using threshold
        let required_signer_count = multisig.threshold as usize;
        let signer_count = num_signers as usize;
        require!(signer_count >= required_signer_count, MultisigError::InvalidSignerCount);

        let signers = remaining_accounts
            .get(..signer_count)
            .ok_or(MultisigError::InvalidSignerCount)?;

        // Setup the aggregated permissions and the vote permission count
        let mut aggregated_permissions = Permissions { mask: 0 };
        let mut vote_permission_count = 0;
        let mut seen_members = Vec::with_capacity(signer_count);

        // Check permissions for all signers
        for signer in signers.iter() {
            if let Some(member_index) = multisig.is_member(signer.key()) {
                // Check that the signer is indeed a signer
                if !signer.is_signer {
                    return err!(MultisigError::MissingSignature);
                }
                // Check for duplicate signer
                if seen_members.contains(&signer.key()) {
                    return err!(MultisigError::DuplicateMember);
                }
                seen_members.push(signer.key());

                let member_permissions = multisig.members[member_index].permissions;
                // Add to the aggregated permissions mask
                aggregated_permissions.mask |= member_permissions.mask;

                // Count the vote permissions
                if member_permissions.has(Permission::Vote) {
                    vote_permission_count += 1;
                }
            } else {
                return err!(MultisigError::NotAMember);
            }
        }

        // Check if we have all required permissions (Initiate | Vote | Execute
        // = 7)
        require!(
            aggregated_permissions.mask == 7,
            MultisigError::InsufficientAggregatePermissions
        );

        // Verify threshold is met across all voting permissions
        require!(
            vote_permission_count >= multisig.threshold as usize,
            MultisigError::InsufficientVotePermissions
        );

        Ok(())
    }

    #[access_control(ctx.accounts.validate(args.num_signers, &ctx.remaining_accounts))]
    pub fn vault_transaction_sync(
        ctx: Context<Self>,
        args: VaultTransactionSyncArgs,
    ) -> Result<()> {
        // Readonly Accounts
        let multisig = &ctx.accounts.multisig;

        let multisig_key = multisig.key();
        // Deserialize the instructions
        let compiled_instructions =
            SmallVec::<u8, CompiledInstruction>::try_from_slice(&args.instructions)
                .map_err(|_| MultisigError::InvalidInstructionArgs)?;
        // Convert to MultisigCompiledInstruction
        let multisig_compiled_instructions: Vec<MultisigCompiledInstruction> =
            Vec::from(compiled_instructions)
                .into_iter()
                .map(MultisigCompiledInstruction::from)
                .collect();

        let vault_seeds = &[
            SEED_PREFIX,
            multisig_key.as_ref(),
            SEED_VAULT,
            &args.vault_index.to_le_bytes(),
        ];

        let (vault_pubkey, vault_bump) = Pubkey::find_program_address(vault_seeds, ctx.program_id);

        // Get the signer seeds for the vault
        let vault_signer_seeds = &[
            vault_seeds[0],
            vault_seeds[1],
            vault_seeds[2],
            vault_seeds[3],
            &[vault_bump],
        ];

        let executable_message = SynchronousTransactionMessage::new_validated(
            &multisig.key(),
            &multisig,
            &vault_pubkey,
            multisig_compiled_instructions,
            &ctx.remaining_accounts,
        )?;

        // Execute the transaction message instructions one-by-one.
        // NOTE: `execute_message()` calls `self.to_instructions_and_accounts()`
        // which in turn calls `take()` on
        // `self.message.instructions`, therefore after this point no more
        // references or usages of `self.message` should be made to avoid
        // faulty behavior.
        executable_message.execute(vault_signer_seeds)?;

        Ok(())
    }
}
