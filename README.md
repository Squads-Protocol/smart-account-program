# Squads Smart Account Program

<img width="2500" alt="Frame 13" src="./assets/title_image.png">

![license][license-image]
![version][version-image]

[version-image]: https://img.shields.io/badge/version-0.1.0-blue.svg?style=flat
[license-image]: https://img.shields.io/badge/license-AGPL_3.0-blue.svg?style=flat

High-performance smart wallet infrastructure for Solana. Built for stablecoin systems, programmable wallets, and financial applications at scale.

## Table of Contents

- [Overview](#overview)
- [Grid: The Easiest Way to Build](#grid-the-easiest-way-to-build)
- [Quick Reference](#quick-reference)
- [Core Concepts](#core-concepts)
  - [Settings Account](#settings-account)
  - [Sub-Accounts (Vaults)](#sub-accounts-vaults)
  - [Signers & Permissions](#signers--permissions)
  - [Time Lock](#time-lock)
  - [Stale Transaction Protection](#stale-transaction-protection)
  - [Account Types](#account-types)
- [Execution Modes](#execution-modes)
  - [Consensus Transactions (Async)](#consensus-transactions-async)
  - [Synchronous Transactions](#synchronous-transactions)
- [Policy Framework](#policy-framework)
  - [Program Interaction Policy (Smart Transactions)](#program-interaction-policy-smart-transactions)
  - [Spending Limit Policy](#spending-limit-policy)
  - [Internal Fund Transfer Policy](#internal-fund-transfer-policy)
  - [Settings Change Policy](#settings-change-policy)
- [Legacy Spending Limits](#legacy-spending-limits)
- [Developers](#developers)
- [Building](#building)
- [Testing](#testing)
- [Verifying the Program](#verifying-the-program)
- [Responsibility](#responsibility)
- [Security](#security)
- [License](#license)

## Overview

Smart contract wallets have traditionally forced developers to choose between programmability, cost, and security. The Smart Account Program eliminates this trilemma:

- **Programmability**: Multi-signature governance, granular permissions, and flexible policies (spending limits, time locks, program interaction rules)
- **Cost**: Atomic policy enforcement and transaction execution in a single operation. Rent-free account creation allows deploying wallets for as little as 0.0000025 SOL, with rent deferred until the account executes transactions
- **Security**: Audited by OtterSec and formally verified by Certora. Supports native keypairs, MPC/TEE signing, with passkeys and alternative signature schemes coming soon

For the full announcement, see [Smart Account Program: Live on Mainnet](https://squads.xyz/blog/squads-smart-account-program-live-on-mainnet).

## Grid: The Easiest Way to Build

**Don't want to integrate directly with the program?** [Grid](https://squads.xyz/grid) is our developer platform that abstracts the Smart Account Program into a simple API.

With Grid you get:
- **Stablecoin accounts** - policy-ready, fault-tolerant, proven in production
- **Payments** - stablecoins + ACH, Wire, SEPA rails
- **Card infrastructure** - programmable spend controls
- **Yield integrations** - RWA and DeFi strategies
- **Gas abstraction** - seamless UX without native tokens
- **Reconciliation** - unified ledger across all rails

Grid is trusted by 400+ teams securing $15B+. If you're building fintech, neobanks, or consumer apps, start with [Grid's documentation](https://grid.squads.xyz/welcome).

For the full vision, see [Grid: A Stablecoin API for Accounts, Payments, Cards and Yield](https://squads.xyz/blog/grid-a-stablecoin-api-for-accounts-payments-cards-and-yield).

---

*The rest of this README documents the underlying Smart Account Program for developers who need direct protocol access.*

## Quick Reference

| Network | Address |
|---------|---------|
| Mainnet | `SMRTzfY6DfH5ik3TKiyLFfXexV8uSG3d2UksSCYdunG` |
| Devnet | `SMRTzfY6DfH5ik3TKiyLFfXexV8uSG3d2UksSCYdunG` |

## Core Concepts

### Settings Account

The Settings account is the central configuration for a Smart Account.

**PDA Derivation:**
```
seeds = ["smart_account", "settings", seed]
```

**Account Structure:**
```rust
Settings {
    seed: u128,                         // Unique identifier assigned by program config
    settings_authority: Pubkey,         // Controls settings (default = autonomous)
    threshold: u16,                     // Required votes for approval
    time_lock: u32,                     // Seconds between approval and execution
    transaction_index: u64,             // Latest transaction number
    stale_transaction_index: u64,       // Staleness boundary
    archival_authority: Option<Pubkey>, // Reserved for compression feature
    archivable_after: u64,              // Timestamp for archival eligibility
    bump: u8,                           // PDA bump
    signers: Vec<SmartAccountSigner>,   // Signers with permissions
    account_utilization: u8,            // Number of sub-accounts in use
    policy_seed: Option<u64>,           // Counter for deterministic policy creation
}

SmartAccountSigner {
    key: Pubkey,
    permissions: Permissions { mask: u8 },
}
```

### Sub-Accounts (Vaults)

Assets are held in sub-accounts derived from the Settings account. Each sub-account is identified by an `account_index` (u8), allowing up to 256 vaults per Smart Account.

**PDA Derivation:**
```
seeds = ["smart_account", settings_key, "smart_account", account_index]
```

Sub-account 0 is typically the primary vault.

### Signers & Permissions

Each signer has a pubkey and a permission bitmask controlling what actions they can perform:

| Permission | Bit | Description |
|------------|-----|-------------|
| Initiate | `0b001` | Create transactions and proposals |
| Vote | `0b010` | Approve or reject proposals |
| Execute | `0b100` | Execute approved transactions |

Permissions combine freely. Examples:
- `0b111` (7) - Full access: can initiate, vote, and execute
- `0b011` (3) - Can initiate and vote, but not execute
- `0b110` (6) - Can vote and execute, but not initiate

**Threshold** defines how many Vote permissions are required for approval. A 2-of-3 multisig means threshold=2 with at least 3 signers having Vote permission.

**Invariants enforced by the program:**
- At least one signer must have Initiate permission
- At least one signer must have Vote permission
- At least one signer must have Execute permission
- Threshold must be > 0 and <= number of voters

### Time Lock

Configurable delay (in seconds) between proposal approval and execution. Range: 0 to 90 days.

- Time lock of 0 enables synchronous transactions (immediate execution)
- Non-zero time lock requires waiting period after approval

### Stale Transaction Protection

When signers, threshold, or time lock change, the program updates `stale_transaction_index` to the current `transaction_index`. This invalidates all pending settings transactions, preventing outdated proposals from executing under new governance rules.

### Account Types

**Autonomous:** `settings_authority = Pubkey::default()`
- All configuration changes go through proposal voting
- Fully self-governed

**Controlled:** `settings_authority = <pubkey>`
- The settings authority can modify configuration directly
- Useful for managed/custodial setups

## Execution Modes

### Consensus Transactions (Async)

Full governance flow with voting and time lock. Use when signers aren't available simultaneously or when you need a deliberation period.

**Flow:**
```
Create Transaction → Create Proposal → Vote (until threshold) → [Time Lock] → Execute
```

**Accounts involved:**

```rust
Transaction {
    settings: Pubkey,               // Parent settings account
    creator: Pubkey,                // Who created it
    index: u64,                     // Transaction index
    bump: u8,
    account_index: u8,              // Which vault executes this
    ephemeral_signer_bumps: Vec<u8>,
    message: SmartAccountMessage,   // The actual instruction(s)
}

Proposal {
    settings: Pubkey,
    transaction_index: u64,
    status: ProposalStatus,         // Active, Approved, Rejected, Cancelled, Executed
    bump: u8,
    approved: Vec<Pubkey>,          // Signers who approved
    rejected: Vec<Pubkey>,          // Signers who rejected
    cancelled: Vec<Pubkey>,         // Signers who cancelled
    activation_timestamp: i64,      // When time lock ends
}
```

**Lifecycle:**
1. **Create Transaction**: Store the instruction(s) to execute
2. **Create Proposal**: Initialize voting state (status = Active)
3. **Vote**: Signers approve/reject. Once threshold reached, status = Approved and `activation_timestamp` is set
4. **Time Lock**: Wait until `current_time >= activation_timestamp`
5. **Execute**: Run the transaction. Status = Executed

**Settings Transactions** follow the same flow but modify the Settings account itself (add/remove signers, change threshold, etc.). They have additional staleness checks: if the settings changed after proposal creation, execution fails.

### Synchronous Transactions

Atomic execution when all required signers are present. No intermediate accounts, no waiting.

**Requirements:**
- `time_lock = 0` on the Settings account
- All signing keys present in the transaction
- Combined permissions satisfy Initiate + Vote + Execute
- Number of voters meets threshold

**Flow:**
```
Single transaction with all signatures → Validate → Execute
```

This is the most gas-efficient path when coordination is possible. Ideal for automated systems, MPC wallets, or situations where all parties are online.

## Policy Framework

Policies are scoped permission sets with their own governance. Each policy has independent signers, threshold, and time lock from the main Settings account.

**PDA Derivation:**
```
seeds = ["smart_account", "policy", settings_key, policy_seed]
```

**Account Structure:**
```rust
Policy {
    settings: Pubkey,                   // Parent settings account
    seed: u64,                          // Unique seed (auto-incremented)
    bump: u8,
    transaction_index: u64,             // For stale protection
    stale_transaction_index: u64,
    signers: Vec<SmartAccountSigner>,   // Policy-specific signers
    threshold: u16,
    time_lock: u32,
    policy_state: PolicyState,          // Type-specific configuration
    start: i64,                         // Activation timestamp
    expiration: Option<PolicyExpiration>,
    rent_collector: Pubkey,             // Receives rent on close
}
```

**Policy Types:**

| Type | Purpose |
|------|---------|
| `SpendingLimit` | Token transfers with amount/time constraints |
| `InternalFundTransfer` | Move funds between sub-accounts |
| `ProgramInteraction` | Constrained external program calls (Smart Transactions) |
| `SettingsChange` | Delegated configuration changes |

**Expiration Modes:**
- `Timestamp(i64)` - Expires at Unix timestamp
- `SettingsState([u8; 32])` - Expires when settings hash changes (signers/threshold/time_lock modified)

Policies are created via `SettingsAction::PolicyCreate`, `PolicyUpdate`, and `PolicyRemove`.

### Program Interaction Policy (Smart Transactions)

The Program Interaction Policy enables autonomous, rules-based transaction execution. Instead of deploying custom programs, you can define constraints that are enforced atomically onchain.

**Why onchain rule enforcement matters:** Off-chain validation introduces a trust layer between intent and execution. If checks fail or are bypassed, transactions still execute. Onchain enforcement eliminates this since invalid rules make the transaction itself invalid.

**Constraint Types:**

```rust
ProgramInteractionPolicy {
    account_index: u8,                           // Which vault executes
    instructions_constraints: Vec<InstructionConstraint>,
    pre_hook: Option<Hook>,                      // Called before execution
    post_hook: Option<Hook>,                     // Called after execution
    spending_limits: Vec<SpendingLimitV2>,       // Balance tracking
}

InstructionConstraint {
    program_id: Pubkey,                          // Allowed program
    account_constraints: Vec<AccountConstraint>, // Required accounts
    data_constraints: Vec<DataConstraint>,       // Instruction data rules
}
```

**What you can constrain:**
- **Allowed programs** - Whitelist by program ID
- **Allowed instructions** - Filter by discriminator
- **Required accounts** - Mandate specific accounts in the transaction
- **Account data validation** - Check balances, flags, pubkeys, numeric thresholds
- **Spending limits** - Track and cap balance changes per period

**Data Operators:**
```rust
DataOperator::Equals | NotEquals | GreaterThan | GreaterThanOrEqualTo | LessThan | LessThanOrEqualTo
```

**Use cases:** DCA strategies, yield routing, conditional escrow, agentic trading, automated treasury management.

For the full vision, see [Smart Transactions: A Primitive for Autonomous Finance](https://squads.xyz/blog/grid-smart-transactions-a-primitive-for-autonomous-finance-on-solana).

#### Hooks (Pre & Post Execution)

Hooks allow external programs to be invoked before and/or after the main transaction executes. This enables custom validation, logging, side effects, or integration with external protocols.

```rust
Hook {
    program_id: Pubkey,                      // Program to invoke
    num_extra_accounts: u8,                  // Additional accounts beyond program ID
    account_constraints: Vec<AccountConstraint>, // Constraints on hook accounts
    instruction_data: Vec<u8>,               // Data passed to the hook
    pass_inner_instructions: bool,           // Forward inner tx instructions to hook
}
```

**Execution flow:**
```
[Pre-Hook] → Constraint Validation → Execute Transaction → [Post-Hook]
```

**Key features:**
- **Pre-hooks** run before the main transaction - useful for validation, price checks, or state snapshots
- **Post-hooks** run after execution - useful for logging, notifications, or post-condition verification
- **`pass_inner_instructions`** - when true, the hook receives serialized inner instructions and their accounts, enabling the hook to inspect what's being executed
- Hooks are invoked via CPI with a dedicated **hook authority** PDA as signer

**Use cases:**
- Oracle price validation before swaps
- Compliance checks before transfers
- Event emission for off-chain indexing
- Post-execution accounting or reconciliation

### Spending Limit Policy

Token transfers with periodic limits and optional accumulation.

```rust
SpendingLimitPolicy {
    source_account_index: u8,
    destinations: Vec<Pubkey>,           // Empty = any destination
    spending_limit: SpendingLimitV2 {
        mint: Pubkey,                    // Token mint (default = SOL)
        time_constraints: TimeConstraints {
            start: i64,
            period: Period,              // Daily, Weekly, Monthly, etc.
            expiration: Option<i64>,
            accumulate_unused: bool,     // Roll over unused amounts
        },
        quantity_constraints: QuantityConstraints {
            max_per_period: u64,
            max_per_use: u64,
            enforce_exact_quantity: bool,
        },
        usage: UsageState {
            remaining_in_period: u64,
            last_reset: i64,
        },
    },
}
```

### Internal Fund Transfer Policy

Move assets between sub-accounts within the same Smart Account.

```rust
InternalFundTransferPolicy {
    source_account_mask: [u8; 32],       // Bitmask of allowed source indices
    destination_account_mask: [u8; 32],  // Bitmask of allowed destination indices
    allowed_mints: Vec<Pubkey>,          // Empty = any mint
}
```

Uses bitmasks for efficient storage - supports all 256 possible sub-account indices.

### Settings Change Policy

Delegate specific configuration changes without full governance.

```rust
SettingsChangePolicy {
    allowed_actions: Vec<SettingsChangeAction>,
}
```

Allows policies to modify Settings (add/remove signers, change threshold, etc.) within defined bounds.

## Legacy Spending Limits

Before the Policy Framework, spending limits were standalone accounts managed directly via Settings actions. These remain supported for backwards compatibility.

**PDA Derivation:**
```
seeds = ["smart_account", settings_key, "spending_limit", seed]
```

```rust
SpendingLimit {
    settings: Pubkey,
    seed: Pubkey,                    // User-provided seed for PDA
    account_index: u8,               // Which vault
    mint: Pubkey,                    // Token mint (default = SOL)
    amount: u64,                     // Max per period
    period: Period,                  // OneTime, Day, Week, Month
    remaining_amount: u64,           // Current period remaining
    last_reset: i64,                 // Last reset timestamp
    bump: u8,
    signers: Vec<Pubkey>,            // Any one can use the limit
    destinations: Vec<Pubkey>,       // Allowed recipients (empty = any)
    expiration: i64,                 // Expiration timestamp
}
```

Created via `SettingsAction::AddSpendingLimit`, removed via `SettingsAction::RemoveSpendingLimit`.

**Key differences from Spending Limit Policy:**
- No independent governance (uses Settings signers)
- Simpler structure (no time constraints, no accumulation)
- User-provided seed vs auto-incremented

## Developers

You can interact with the Squads Smart Account Program via our SDKs.

**SDKs:**
- TypeScript: [`@sqds/smart-account`](https://www.npmjs.com/package/@sqds/smart-account)

## Building

This program requires Solana 1.18.16. Install it via Agave:

```bash
agave-install init 1.18.16
```

Compile the program with Anchor:

```bash
anchor build
```

If you don't have Anchor CLI installed, follow [this guide](https://www.anchor-lang.com/docs/installation).

After building the program, rebuild the SDK to generate updated types:

```bash
yarn build
```

## Testing

To deploy the program on a local validator for testing:

```bash
solana-test-validator
```

Install dependencies and run tests:

```bash
yarn
yarn test
```

## Verifying the Program

Clone and build the program:

```bash
git clone https://github.com/Squads-Protocol/smart-account-program.git
anchor build
```

Install [solana-verify](https://crates.io/crates/solana-verify):

```bash
cargo install solana-verify
```

Get the hash of your locally compiled program:

```bash
solana-verify get-executable-hash target/deploy/squads_smart_account_program.so
```

Compare with the on-chain program:

```bash
solana-verify get-program-hash -u <cluster_url> SMRTzfY6DfH5ik3TKiyLFfXexV8uSG3d2UksSCYdunG
```

If the hashes match, the repository code matches the deployed program.

## Responsibility

By interacting with this program, users acknowledge and accept full personal responsibility for any consequences, regardless of their nature. This includes both potential risks inherent to the smart contract, also referred to as program, as well as any losses resulting from user errors or misjudgment.

By using a smart account, it is important to acknowledge certain concepts. Here are some that could be misunderstood by users:

- Loss of Private Keys: If a participant loses their private key, the smart account may not be able to execute transactions if a threshold number of signatures is required.
- Single Point of Failure with Keys: If all keys are stored in the same location or device, a single breach can compromise the smart account.
- Forgetting the Threshold: Misremembering the number of signatures required can result in a deadlock, where funds cannot be accessed.
- No Succession Planning: If keyholders become unavailable (e.g., due to accident, death), without a plan for transition, funds may be locked forever.
- Transfer of funds to wrong address: Funds should always be sent to the smart account account, and not the smart account settingsaddress. Due to the design of the Squads Protocol program, funds deposited to the smart account may not be recoverable.
- If the settings_authority of a smart account is compromised, an attacker can change smart account settings, potentially reducing the required threshold for transaction execution or instantly being able to remove and add new members.
- If the underlying SVM compatible blockchain undergoes a fork and a user had sent funds to the orphaned chain, the state of the blockchain may not interpret the owner of funds to be original one.
- Users might inadvertently set long or permanent time-locks in their smart account, preventing access to their funds for that period of time.
- Smart account participants might not have enough of the native token of the underlying SVM blockchain to pay for transaction and state fees.

## Security

The Squads Smart Account Program has been audited by Ottersec and Certora and additionally formally verified by Certora.

- Certora FV & Audit: [View Full Report](./audits/certora_smart_account_audit+FV.pdf)
- Ottersec Audit: (coming soon)

## License

The primary license for Squads Smart Account Program is the AGPL-3.0 license, see [LICENSE](./LICENSE). The following exceptions are licensed separately as follows:

- The file <https://github.com/Squads-Protocol/smart-account-program/blob/main/programs/squads_smart_account_program/src/utils/system.rs> is derived from code released under the [Apache 2.0 license](https://github.com/coral-xyz/anchor/blob/master/LICENSE) at <https://github.com/coral-xyz/anchor/blob/714d5248636493a3d1db1481f16052836ee59e94/lang/syn/src/codegen/accounts/constraints.rs#L1126-L1179>.
- The file <https://github.com/Squads-Protocol/smart-account-program/blob/main/programs/squads_smart_account_program/src/utils/small_vec.rs> is derived from code released under both the [Apache 2.0 license](https://github.com/near/borsh-rs/blob/master/LICENSE-APACHE) and the [MIT license](https://github.com/near/borsh-rs/blob/master/LICENSE-MIT) at <https://github.com/near/borsh-rs/blob/master/borsh/src/de/hint.rs> and <https://github.com/near/borsh-rs/blob/master/borsh/src/ser/mod.rs>.

To the extent that each such file incorporates code from another source, such code is licensed under its respective open source license as provided above, and the original open source code is copyrighted by its respective owner as provided above.
