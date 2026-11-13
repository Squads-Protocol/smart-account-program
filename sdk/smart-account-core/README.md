# @sqds/smart-account-core

A narrow TypeScript SDK for the Squads Smart Account Program
(mainnet `SMRTzfY6DfH5ik3TKiyLFfXexV8uSG3d2UksSCYdunG`).

**Status: first implementation, under test.** Synchronous execution only, Solana v1 transactions only. The package also contains:

- `test/fixtures/mainnet/`: the pinned mainnet program binary, the on-chain Anchor IDL, the ProgramConfig account, and
  `PINNED.json` (hashes and slots). `IDL-DIFF.md` explains why the on-chain IDL is **stale** compared with the deployed binary.
- `scripts/pin-mainnet.mjs`: re-pins the fixtures. With `--check` it compares against mainnet and exits non-zero on drift. It uses `$SOLANA_RPC_URL`, or the
  public RPC if that is unset.
- `test/spike/litesvm-v1.test.ts`: runs the mainnet binary in LiteSVM with Solana v1 transactions (SIMD-0385).
- `test/impl/`: implementation checks (vector byte-equality, and every builder on the mainnet binary through a LiteSVM-backed RPC).
- `scripts/ci-guards.mjs`: three CI checks. `deps` fails if the package has any runtime dependency. `lines` fails if `src/` has more than 1500 hand-written lines. `casts` fails on an `as` type assertion (other than `as const`) or `any` in `src/` outside `src/encoding/`.
- `examples/check-v1-signer.mjs`: an optional [wallet and key-provider compatibility check](https://github.com/Squads-Protocol/smart-account-program/tree/main/sdk/smart-account-core/examples). It is separate from the SDK tests and CI.

## Install

```sh
npm install @sqds/smart-account-core@^0.1.0 @solana/kit@^8.6.0
```

The package is ESM only and requires Node.js 22.18 or later when used in Node.
CommonJS applications can load it with `await import('@sqds/smart-account-core')`.

## Public API (`src/`)

All amounts are `bigint` in the smallest unit. Builders are pure (no I/O, clock or randomness); they are `async` only
where kit's PDA derivation is. Only `fetch*`, `resolveCreatedAccount`, `send` and `checkOutcome` take an `rpc`
(a kit `Rpc`). Every error the package raises itself is a `SmartAccountError` with a stable `code` (errors from kit,
such as a signer's or address parser's, and RPC transport errors pass through unchanged):
`ACCOUNT_NOT_FOUND`, `NOT_A_SMART_ACCOUNT`, `INSUFFICIENT_SIGNERS`, `SYNC_NOT_ALLOWED`, `INVALID_SIGNER_SET`,
`SIMULATION_FAILED`, `UNSUPPORTED_TRANSACTION`, `TX_TOO_LARGE`, `INVALID_ARGUMENT`, `SIGNER_MODIFIED_TRANSACTION`.

| Export | Signature (abridged) |
|---|---|
| `fetchAccount` | `(rpc, settings) → Promise<AccountSnapshot>` |
| `fetchBalances` | `(rpc, vault: VaultAddress) → Promise<{ lamports; tokens: { address, mint, tokenProgram, amount, decimals }[] }>` |
| `fetchSpendingLimits` | `(rpc, settings) → Promise<SpendingLimit[]>` |
| `deriveSettings` | `(accountIndex: bigint) → Promise<SettingsAddress>` |
| `deriveVault` | `(settings, vaultIndex 0..255) → Promise<VaultAddress>` |
| `deriveSpendingLimit` | `(settings, seed) → Promise<SpendingLimitAddress>` |
| `deriveProgramConfig` | `() → Promise<Address>` (fetch it with kit's `fetchEncodedAccount` for `buildCreateAccount`) |
| `buildVaultTransaction` | `({ account, vaultIndex, signers, instructions }) → Promise<Instruction>` (`executeTransactionSync`) |
| `wrapTransaction` | `({ account, vaultIndex, signers, message \| instructions }) → Promise<{ instruction, stripped: { unitLimit?, unitPrice?, priorityFeeLamports? } }>` |
| `buildTransfer` | `({ account, vaultIndex, signers, mint: Address \| "SOL", to, amount, decimals?, tokenProgram?, createRecipientAta? }) → Promise<Instruction>` |
| `buildSetSigners` | `({ account, signers, newSigners, newThreshold, rentPayer }) → Instruction` (`executeSettingsTransactionSync`) |
| `buildRemoveSpendingLimit` | `({ account, signers, spendingLimit, rentPayer }) → Instruction` |
| `buildCreateAccount` | `({ programConfig, creator, signers, threshold, timeLock?, settingsAuthority?, rentCollector?, memo?, candidateCount = 25 }) → Promise<{ instruction, candidates, computeUnitLimit }>` |
| `resolveCreatedAccount` | `(rpc: Rpc<GetTransactionApi & GetMultipleAccountsApi>, signature, candidates) → Promise<SettingsAddress>` (see below) |
| `send` | `(rpc: SendRpc, { instructions, feePayer: SendSigner, signers?: SendSigner[], computeUnitLimit?, loadedAccountsDataSizeLimit?, priorityFeeLamports? }) → Promise<SendOutcome>`; `SendSigner = TransactionPartialSigner \| TransactionModifyingSigner` |
| `checkOutcome` | `(rpc: Rpc<GetSignatureStatusesApi & GetEpochInfoApi>, signature, lastValidBlockHeight) → Promise<SendOutcome \| { status: "expired"; signature }>` |
| `SmartAccountError`, `SMART_ACCOUNT_PROGRAM_ADDRESS` | |
| `SendOutcome` | `{ status: "confirmed"; signature; slot } \| { status: "failed"; signature; error: ProgramErrorInfo } \| { status: "unknown"; signature; lastValidBlockHeight; cause?: unknown }` (`cause`: the RPC error, if any, that left the outcome unknown; never resend on it) |
| Types | `SettingsAddress`, `VaultAddress`, `SpendingLimitAddress`, `Permission`, `AccountSnapshot`, `SpendingLimit`, `SendOutcome`, `CheckedOutcome`, `ProgramErrorInfo`, `SendInput`, `SendRpc`, `SendSigner`, `StrippedComputeBudget`, ... |

`resolveCreatedAccount` returns the one candidate the transaction funded (zero lamports before, positive after). Anyone
can pre-fund candidate addresses, and the program's init accepts a pre-funded address, so it falls back to the one
candidate whose balance rose in the transaction (pre-funded below rent), then to reading the candidates: the one
Settings account whose seed derives its address and whose signers, threshold, time lock and settings authority equal the
transaction's `createSmartAccount` arguments (pre-funded above rent). No match or several matches is
`ACCOUNT_NOT_FOUND`; it never guesses.

`fetch*` and `resolveCreatedAccount` pass `commitment: "confirmed"` on every RPC read, so a custom transport's default
(often `finalized`) does not apply.

`signers` in builders are keys or kit `TransactionSigner`s; signer objects are embedded in the account metas, and `send`
also attaches any `signers` passed to it. `send` accepts partial signers and modifying signers (browser wallet-standard
wallets are modifying signers). If any signer changes the message it was given (blockhash, instructions, config), `send`
throws `SIGNER_MODIFIED_TRANSACTION` before sending anything. `send` compiles a v1 message, simulates to size the compute-unit and
loaded-accounts-data limits when they are not given, confirms against the transaction's own `lastValidBlockHeight`, and
never rebuilds or re-signs: an `unknown` outcome is resolved with `checkOutcome`. It reads the finalized block height and
slot together (`getEpochInfo`, commitment `finalized`), then the signature status with `searchTransactionHistory: true`,
and reports `expired` (safe to rebuild) only when the height has passed `lastValidBlockHeight`, the status response's
`context.slot` is at or after that finalized slot, and the signature is absent. Otherwise it reports `unknown`.
Resolving an outcome more than ~2 minutes after sending needs an RPC that serves transaction history: a node that has
pruned the signature cannot tell "never landed" from "landed long ago".

Example:

```ts
const account = await fetchAccount(rpc, settings);
const ix = await buildTransfer({ account, vaultIndex: 0, signers: [signer], mint: "SOL", to, amount: 1_000_000n });
const outcome = await send(rpc, { instructions: [ix], feePayer: signer });
```

`wrapTransaction` removes ComputeBudget instructions from the wrapped set. `stripped.unitLimit` is the CU limit and
`stripped.unitPrice` the price in micro-lamports per CU; when both are present, `stripped.priorityFeeLamports`
(= ceil(unitPrice × unitLimit / 1,000,000)) is the total in lamports, which is the value to pass to `send` as
`priorityFeeLamports`. Do not pass `unitPrice` there: `send` takes a total, not a per-CU price. Heap-frame and
loaded-accounts-data-size instructions are dropped and not reported (`send` sizes the loaded-accounts limit itself).

A base64 v0/legacy transaction is decompiled with kit first and passed as `message` to `wrapTransaction`:
`decompileTransactionMessage(getCompiledTransactionMessageDecoder().decode(getTransactionDecoder().decode(bytes).messageBytes))`.

## Constraints

- Zero runtime `dependencies`. The only peer dependency is `@solana/kit` (^8.6). Everything else is a devDependency.
- At most 1500 hand-written lines in `src/`. Files whose first line contains `@generated` are excluded.
- Strict TypeScript: `strict`, `noUncheckedIndexedAccess`, `exactOptionalPropertyTypes`, `noImplicitOverride`.

## Package location and tooling decision

- **Path:** `sdk/smart-account-core`, in the official `Squads-Protocol/smart-account-program` repository.
- **Not a yarn workspace.** The root workspace list contains only the existing `sdk/smart-account` package.
  Core uses its own npm lockfile and tooling so the root's TypeScript and Solana dependency resolutions do not apply to it.
- **Standalone npm package** with its own `package-lock.json` and its own CI (`.github/workflows/sdk-core.yml`).
  Run `npm ci` inside this directory.
- **Test runner: `node:test`.** It is built into Node 22 and runs `.ts` files directly through Node's type stripping (≥22.18), so it adds
  no devDependency and needs no build step.
- **Third-party packages are kept to what is required.** The allowed devDependencies are `litesvm` (required), `@solana/kit`
  (the peer dependency, pinned for tests), and `typescript` + `@types/node` (strict typecheck). `npm run check:deps` enforces
  this list, and changing it needs owner review (CODEOWNERS). No property-testing, mutation-testing or model-checking packages:
  randomized tests use a small seeded generator in the repo instead.

## Commands

```sh
npm ci
npm run typecheck      # tsc --noEmit (TypeScript 7)
npm test               # node --test "test/**/*.test.ts"
npm run check:deps     # fail on any runtime dependency / unexpected peer
npm run check:lines    # fail above 1500 hand-written src lines
npm run check:casts    # fail on `as` casts (other than `as const`) or `any` outside src/encoding/
npm run pin:check      # mainnet drift check (network)
npm run pin:mainnet    # re-pin fixtures (network; review the diff!)
E2E_PAYER_KEYPAIR=key.json npm run e2e:devnet   # live v1 sends on devnet (spends ~0.1 SOL; not in CI)
```

## Publishing

Run from `sdk/smart-account-core` on the reviewed release commit:

```sh
npm ci
npm run typecheck
npm test
npm run check:deps
npm run check:lines
npm run check:casts
npm run pin:check
npm pack --dry-run
npm publish --access public
```

The `prepack` hook builds JavaScript and declarations before packing or publishing.
Only `dist/`, this README and npm's standard package metadata are published; the test fixtures and development scripts stay in the repository.
Publishing requires npm access to the `@sqds` scope. CI validates the package but does not publish it automatically.

`e2e:devnet` creates a fresh account whose only signer is the payer key, then sends real v1 transactions for:
account creation, wrapping a customer-built v0 transaction, an SPL transfer with recipient ATA creation, signer changes
(1-of-1 → 2-of-2 → 1-of-1) and spending-limit removal. Each step checks the on-chain result. `SOLANA_RPC_URL`
overrides the cluster.

## LiteSVM / v1 transaction findings (litesvm 1.6.2, @solana/kit 8.6.0, Linux x86_64)

- The mainnet binary loads through `svm.addProgram`. `createSmartAccount` and `executeTransactionSync` (a SOL transfer from vault 0)
  succeed with both v1 and v0 transactions. The `std::bad_alloc` seen with litesvm 0.8 and web3.js 1.x in
  `sdk/smart-account-client` does **not** happen.
- A v1 transaction carries its compute-unit limit and loaded-accounts-data-size limit in the message config, not in ComputeBudget
  instructions. Set them with kit's `setTransactionMessageComputeUnitLimit` and
  `setTransactionMessageLoadedAccountsDataSizeLimit`.
- **If either config value is left out, LiteSVM treats it as 0.** With no data-size limit, the transaction fails with
  `MaxLoadedAccountsDataSizeExceeded`, because the program alone is 1.6 MB. With no CU limit, it fails with "consumed 0 of 0 compute units".
  SDK transaction builders must always set both. Whether mainnet validators use the same defaults still needs checking against SIMD-0385.
