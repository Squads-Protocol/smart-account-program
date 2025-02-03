use anchor_lang::prelude::*;

#[derive(AnchorSerialize, AnchorDeserialize, Debug)]
pub struct LogEventArgs {
    pub account_seeds: Vec<Vec<u8>>,
    pub bump: u8,
    pub event: Vec<u8>,
}
#[derive(Accounts)]
#[instruction(args: LogEventArgs)]
pub struct LogEvent<'info> {
    #[account(
        // Any account owned by the Smart Account Program, except for individual smart accounts, should be able to
        // log data. Smart accounts are the only accounts owned by the System Program.
        owner = crate::id(),
    )]
    pub log_authority: Signer<'info>,
}
impl<'info> LogEvent<'info> {
    fn validate(&self, args: &LogEventArgs) -> Result<()> {
        let mut collected_seeds: Vec<&[u8]> =
            args.account_seeds.iter().map(|v| v.as_slice()).collect();
        let bump_slice = &[args.bump];
        collected_seeds.push(bump_slice);

        let derived_address =
            Pubkey::create_program_address(collected_seeds.as_slice(), &crate::ID).unwrap();
        assert_eq!(&derived_address, self.log_authority.key);
        Ok(())
    }
    #[access_control(ctx.accounts.validate(&args))]
    pub fn log_event(ctx: Context<'_, '_, 'info, 'info, Self>, args: LogEventArgs) -> Result<()> {
        Ok(())
    }
}
