# Mainnet IDL vs fork IDL vs mainnet binary

Inputs:

- **Mainnet IDL**: `idl.json`, read from the Anchor IDL account `6smoqe8YGsnBkPAEGDmcnxZe9ErCxDwNTJqUio1DcTDm`
  (authority `sqdcVVoTcKZjXU8yPUwKFbGx1Hig1rhbWJQtMRXp2E1`). See `PINNED.json` for hashes and slots.
- **Fork IDL**: `idl/squads_smart_account_program.json` in this repo.
- **Mainnet binary**: `smart_account_program.so`, last deployed at slot 443245754
  (upgrade authority `HT3JknwuufXdtVJggz5Z9JcnYtanPpLzTCqLWsVX1Vu2`).
- **Grid's Rust client**: `grid/libs/smart_account_program/src/{accounts,typedefs,instructions}.rs`.

## Headline

1. **The mainnet IDL account matches the fork IDL.** Both use the legacy Anchor IDL format (`version: 0.1.0`,
   no `address`/`metadata`). The two files have the same instructions (37), accounts (9), types (39, in the same order) and errors (54).
   The only differences are 15 instruction `docs[0]` strings, where the fork still says "multisig" and mainnet says
   "smart account". No byte layout differs.
2. **The mainnet IDL account is stale. The deployed binary is a newer build with policies.** Evidence from the LiteSVM probe
   in `test/spike/litesvm-v1.test.ts`:
   - The Anchor dispatcher accepts `execute_transaction_sync_v2` (`sha256("global:execute_transaction_sync_v2")[0..8]`),
     which appears in neither IDL. All 37 IDL instructions still dispatch. Bogus discriminators return `InstructionFallbackNotFound`.
   - `closeTransactionBuffer`, `executeTransaction` and `closeTransaction` fail on an account named **`consensus_account`**.
     The IDL names that account `settings`. In v2 a consensus account can be a Settings account or a Policy account.
   - The binary contains policy error names and strings (`PolicyNotActiveYet`, `ProgramInteraction*`, `SpendingLimitPolicy*`,
     `SettingsChangePolicy*`, `InternalFundTransferPolicy*`) and the log string `ExecuteTransactionSyncV2Instruction`.
   - `createSmartAccount` with 2 signers allocates **201 bytes** for Settings. The fork/IDL `Settings::size(2)` is 193.
     The 8-byte difference is the `reserved1: u8` → `policy_seed: Option<u64>` change (1 byte → 9 bytes). See below.
3. **Use Grid's Rust client as the layout reference, not the on-chain IDL**, for anything policy-related or anything that
   reads Settings past `account_utilization`. In the paths the spike exercised, the IDL and the binary agree.

## Instructions

| | Fork IDL | Mainnet IDL | Mainnet binary (probe) / Grid client |
|---|---|---|---|
| Added | – | – | `executeTransactionSyncV2` (`SyncTransactionArgsV2`, discriminator `5a51bb512746804e`) |
| Removed | – | – | none (all 37 dispatch) |
| Changed (docs only) | 15 `docs` strings | | |
| Account rename | `settings` | `settings` | `consensus_account` in `closeTransactionBuffer`, `executeTransaction`, `closeTransaction` (account order not verified) |

### Layouts called out in the brief

- **`createSmartAccount`**: the same in all three sources, and verified on the binary by the spike.
  - Discriminator: `c566fde74d543211`.
  - Args, in order: `settingsAuthority: Option<Pubkey>`, `threshold: u16`, `signers: Vec<{key: Pubkey, permissions: {mask: u8}}>`,
    `timeLock: u32`, `rentCollector: Option<Pubkey>`, `memo: Option<String>`.
  - Accounts, in order: `programConfig (w)`, `treasury (w)`, `creator (w,s)`, `systemProgram`, `program`.
  - Remaining accounts: the candidate Settings PDAs (writable), with seeds `["smart_account","settings", u128 LE index]`
    starting at `programConfig.smartAccountIndex + 1`.
- **`executeTransactionSync`**: the same in all three sources, and verified by the spike.
  - Discriminator: `2b66f859e7616886`.
  - `SyncTransactionArgs { accountIndex: u8, numSigners: u8, instructions: bytes }`, where `instructions` is a borsh
    `Vec<u8>` wrapping `SmallVec<u8, {programIdIndex: u8, accountIndexes: SmallVec<u8,u8>, data: SmallVec<u16,u8>}>`.
  - Accounts: `settings (r)`, `program`, then remaining `[signers… (s), instruction accounts…]`.
  - Compiled indexes point into the **whole** remaining-accounts array, signers included.
  - Vault PDA: `["smart_account", settings, "smart_account", u8 accountIndex]`.
- **`SyncTransactionArgs` vs `SyncTransactionArgsV2`**: V2 exists only in the binary and in Grid's client.
  - Layout: `{ accountIndex: u8, numSigners: u8, payload: SyncPayload }`, where `SyncPayload = Transaction(Vec<u8>) | Policy(PolicyPayload)`.
  - `PolicyPayload = InternalFundTransfer | ProgramInteraction | SpendingLimit | SettingsChange`.
  - Grid's account order for V2: `[consensus/policy (w), program, signers…, preHook…, postHook…, ixAccounts…, policy (r)]`.
    See `grid/libs/solana-adapter/src/provider/smart_account.rs`.
- **`executeSettingsTransactionSync`**: the IDL and Grid agree.
  - Discriminator: `8ad140a34f43e94c`.
  - `SyncSettingsTransactionArgs { numSigners: u8, actions: Vec<SettingsAction>, memo: Option<String> }`.
  - Accounts: `settings (w)`, `rentPayer (w,s, optional)`, `systemProgram (optional)`, `program`, then remaining
    `[signers…, new spending-limit/policy PDAs…]`.
- **`SettingsAction` variant order**: variants 0–6 are the same in every source: `AddSigner`, `RemoveSigner`, `ChangeThreshold`,
  `SetTimeLock`, `AddSpendingLimit`, `RemoveSpendingLimit`, `SetArchivalAuthority`. Grid's client adds
  **7 `PolicyCreate`, 8 `PolicyUpdate`, 9 `PolicyRemove`**. Tags 0–6 are therefore wire-compatible.
- **`RemoveSpendingLimit`**: `SettingsAction::RemoveSpendingLimit { spendingLimit: Pubkey }` and
  `RemoveSpendingLimitArgs { memo: Option<String> }` are the same in all sources.
  - `removeSpendingLimitAsAuthority` accounts: `settings`, `settingsAuthority (s)`, `spendingLimit (w)`, `rentCollector (w)`, `program`.
- **`CreateTransactionArgs`** (not in the brief, but it affects wire compatibility): the IDL has a **struct**
  `{ accountIndex, ephemeralSigners, transactionMessage, memo }`. Grid's client has an **enum**
  `TransactionPayload(that struct) | PolicyPayload{…}`, which adds a leading tag byte. The probe confirms that
  `createTransaction` dispatches, but the spike did **not** verify which layout the binary expects. Treat Grid's enum as authoritative until a vector proves otherwise.

## Accounts

- **`Settings`**: Grid (`accounts.rs:218-232`) has `policy_seed: Option<u64>` where the fork and IDL have `reserved1: u8`.
  The other fields are the same:
  `seed u128, settingsAuthority, threshold u16, timeLock u32, transactionIndex u64, staleTransactionIndex u64,
  archivalAuthority Option<Pubkey>, archivableAfter u64, bump u8, signers Vec<SmartAccountSigner>, accountUtilization u8,
  {reserved1 u8 | policySeed Option<u64>}, reserved2 u8`.
  - When `policySeed` is `None` it serializes as a single `0`, the same bytes as `reserved1 = 0`. Decoders written from
    the IDL only break once a policy has been created.
  - Allocation is 201 bytes on the binary vs 193 per the IDL. A deserializer must not assume
    `data.length == Settings::size(n)`.
  - `Settings` discriminator: `dfb3a3beb1e043ad`.
- **`ProgramConfig`**: the same in all sources: `smartAccountIndex u128, authority, smartAccountCreationFee u64, treasury, reserved [u8;64]`
  (160 bytes). The mainnet values are in `program_config.json`: index 745853, fee 0, and authority = treasury =
  `init9xckLHfofCRp5SCisRK4f6eDehGRtFSAw5mLhE8`.
- **`Policy`**: exists only in Grid's client and the binary:
  `settings, seed u64, bump, transactionIndex u64, staleTransactionIndex u64, signers, threshold u16, timeLock u32,
  policyState, start i64, expiration Option<PolicyExpiration>`.
- `Transaction`, `Proposal`, `SettingsTransaction`, `SpendingLimit`, `TransactionBuffer`, `Batch`, `BatchTransaction`:
  the IDL and Grid's client agree. The spike did not check them against the binary.

## Types and errors

- Types: the mainnet and fork IDLs are the same (39). Grid's client adds about 40 policy types, including `SyncPayload`, `PolicyPayload`,
  `PolicyCreationPayload`, `PeriodV2`, `SpendingLimitV2`, `ProgramInteractionPolicy`, `InstructionConstraint`,
  `DataConstraint`, `AccountConstraint`, `Hook`, `PolicyExpiration[Args]` and `PolicyState`.
- Errors: the mainnet and fork IDLs are the same (54 codes). The binary has many more (the policy errors above). The codes past 54
  are **not** pinned anywhere yet.

## Follow-ups

- Get the program team to upload a current IDL (Anchor ≥0.30 format) to the IDL account, or pin a source commit
  whose `anchor build` reproduces `soSha256`.
- Add golden vectors for `CreateTransactionArgs` (struct vs enum) and `executeTransactionSyncV2` before the SDK
  encodes either.
