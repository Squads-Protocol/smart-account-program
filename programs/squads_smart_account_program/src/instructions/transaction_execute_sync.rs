use anchor_lang::prelude::*;

use crate::{errors::*, state::*, utils::SynchronousTransactionMessage, SmallVec};

use super::CompiledInstruction;

#[derive(AnchorSerialize, AnchorDeserialize)]
pub struct SyncTransactionArgs {
    pub account_index: u8,
    /// The number of signers to reach threshold and adequate permissions
    pub num_signers: u8,
    /// Expected to be serialized as a SmallVec<u8, CompiledInstruction>
    pub instructions: Vec<u8>,
}

#[derive(Accounts)]
pub struct SyncTransaction<'info> {
    #[account(
        seeds = [SEED_PREFIX, SEED_SETTINGS, settings.seed.as_ref()],
        bump = settings.bump,
    )]
    pub settings: Box<Account<'info, Settings>>,
    // `remaining_accounts` must include the following accounts in the exact order:
    // 1. The exact amount of signers required to reach the threshold
    // 2. Any remaining accounts associated with the instructions
}

impl SyncTransaction<'_> {
    fn validate(&self, num_signers: u8, remaining_accounts: &[AccountInfo]) -> Result<()> {
        let Self { settings } = self;

        // Multisig must not be time locked
        require_eq!(settings.time_lock, 0, SmartAccountError::TimeLockNotZero);

        // Get signers from remaining accounts using threshold
        let required_signer_count = settings.threshold as usize;
        let signer_count = num_signers as usize;
        require!(
            signer_count >= required_signer_count,
            SmartAccountError::InvalidSignerCount
        );

        let signers = remaining_accounts
            .get(..signer_count)
            .ok_or(SmartAccountError::InvalidSignerCount)?;

        // Setup the aggregated permissions and the vote permission count
        let mut aggregated_permissions = Permissions { mask: 0 };
        let mut vote_permission_count = 0;
        let mut seen_members = Vec::with_capacity(signer_count);

        // Check permissions for all signers
        for signer in signers.iter() {
            if let Some(member_index) = settings.is_signer(signer.key()) {
                // Check that the signer is indeed a signer
                if !signer.is_signer {
                    return err!(SmartAccountError::MissingSignature);
                }
                // Check for duplicate signer
                if seen_members.contains(&signer.key()) {
                    return err!(SmartAccountError::DuplicateSigner);
                }
                seen_members.push(signer.key());

                let member_permissions = settings.signers[member_index].permissions;
                // Add to the aggregated permissions mask
                aggregated_permissions.mask |= member_permissions.mask;

                // Count the vote permissions
                if member_permissions.has(Permission::Vote) {
                    vote_permission_count += 1;
                }
            } else {
                return err!(SmartAccountError::NotASigner);
            }
        }

        // Check if we have all required permissions (Initiate | Vote | Execute
        // = 7)
        require!(
            aggregated_permissions.mask == 7,
            SmartAccountError::InsufficientAggregatePermissions
        );

        // Verify threshold is met across all voting permissions
        require!(
            vote_permission_count >= settings.threshold as usize,
            SmartAccountError::InsufficientVotePermissions
        );

        Ok(())
    }

    #[access_control(ctx.accounts.validate(args.num_signers, &ctx.remaining_accounts))]
    pub fn sync_transaction(ctx: Context<Self>, args: SyncTransactionArgs) -> Result<()> {
        // Readonly Accounts
        let settings = &ctx.accounts.settings;

        let settings_key = settings.key();
        // Deserialize the instructions
        let compiled_instructions =
            SmallVec::<u8, CompiledInstruction>::try_from_slice(&args.instructions)
                .map_err(|_| SmartAccountError::InvalidInstructionArgs)?;
        // Convert to MultisigCompiledInstruction
        let settings_compiled_instructions: Vec<SmartAccountCompiledInstruction> =
            Vec::from(compiled_instructions)
                .into_iter()
                .map(SmartAccountCompiledInstruction::from)
                .collect();

        let smart_account_seeds = &[
            SEED_PREFIX,
            settings_key.as_ref(),
            SEED_SMART_ACCOUNT,
            &args.account_index.to_le_bytes(),
        ];

        let (smart_account_pubkey, smart_account_bump) =
            Pubkey::find_program_address(smart_account_seeds, ctx.program_id);

        // Get the signer seeds for the vault
        let vault_signer_seeds = &[
            smart_account_seeds[0],
            smart_account_seeds[1],
            smart_account_seeds[2],
            smart_account_seeds[3],
            &[smart_account_bump],
        ];

        let executable_message = SynchronousTransactionMessage::new_validated(
            &settings.key(),
            &settings,
            &smart_account_pubkey,
            settings_compiled_instructions,
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
