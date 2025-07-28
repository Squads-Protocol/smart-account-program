# Event Logging System Overview

The smart account program includes a custom event logging mechanism that allows program-controlled accounts to emit structured events.

## Core Components

### LogEvent Instruction

**File**: `programs/squads_smart_account_program/src/instructions/log_event.rs`

```rust
pub struct LogEventArgsV2 {
    pub event: Vec<u8>,
}

#[derive(Accounts)]
pub struct LogEvent<'info> {
    #[account(
        constraint = Self::validate_log_authority(&log_authority).is_ok(),
        owner = crate::id(),
    )]
    pub log_authority: Signer<'info>,
}
```

The instruction itself is minimal - it accepts event data and validates the log authority but doesn't process the data.

### Log Authority Validation

Only accounts owned by the smart account program with non-empty, non-zero data can log events:

```rust
pub fn validate_log_authority(log_authority: &Signer<'info>) -> Result<()> {
    let data_len = log_authority.data_len();
    require!(data_len > 0, SmartAccountError::ProtectedInstruction);

    let uninit_data = vec![0; data_len];
    let data = log_authority.try_borrow_data()?;
    require!(
        &**data != &uninit_data,
        SmartAccountError::ProtectedInstruction
    );
    Ok(())
}
```

This prevents unauthorized logging by ensuring only legitimate program accounts (with actual state) can emit events.

## Event Types

**File**: `programs/squads_smart_account_program/src/events/account_events.rs`

The system defines structured events for major operations:

- **CreateSmartAccountEvent**: New smart account creation
- **SynchronousTransactionEvent**: Sync transaction execution
- **SynchronousSettingsTransactionEvent**: Sync settings changes
- **AddSpendingLimitEvent**: Spending limit creation
- **RemoveSpendingLimitEvent**: Spending limit removal
- **UseSpendingLimitEvent**: Spending limit usage
- **AuthoritySettingsEvent**: Authority-based settings changes
- **AuthorityChangeEvent**: Authority transfers

## Event Emission

**File**: `programs/squads_smart_account_program/src/events/mod.rs`

Events are emitted by constructing a cross-program invocation to the LogEvent instruction:

```rust
impl SmartAccountEvent {
    pub fn log<'info>(&self, authority_info: &LogAuthorityInfo<'info>) -> Result<()> {
        let data = LogEventArgsV2 {
            event: self.try_to_vec()?,
        };

        let ix = solana_program::instruction::Instruction {
            program_id: authority_info.program.key(),
            accounts: vec![AccountMeta::new_readonly(
                authority_info.authority.key(),
                true,
            )],
            data: instruction_data,
        };

        invoke_signed(&ix, &[authority_account_info], &[signer_seeds.as_slice()])?;
        Ok(())
    }
}
```
## File Locations

- **Instruction**: `programs/squads_smart_account_program/src/instructions/log_event.rs`
- **Event Types**: `programs/squads_smart_account_program/src/events/account_events.rs`
- **Event Logic**: `programs/squads_smart_account_program/src/events/mod.rs`
- **Entry Point**: `programs/squads_smart_account_program/src/lib.rs:337-342`