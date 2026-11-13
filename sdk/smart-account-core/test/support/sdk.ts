// The public API of @sqds/smart-account-core as spec v0.4 §3/§5 (and plan §5.1) describe it.
// Tests call the implementation only through this facade, so the contract the tests assume is
// written down in one place. Shapes the spec leaves open are marked "assumed" (see README of the
// test suite in the final report); builders are always awaited because PDA derivation in kit is
// async, so a builder that derives a vault/ATA/candidate address may return a Promise.
import type { Address, Instruction, Signature, TransactionMessage, TransactionSigner } from '@solana/kit';

import * as impl from '../../src/index.ts';

export type Permission = 'initiate' | 'vote' | 'execute';
export interface SignerEntry {
  key: Address;
  permissions: readonly Permission[];
}
export interface AccountSnapshot {
  settings: Address;
  slot: bigint;
  threshold: number;
  timeLock: number;
  settingsAuthority: Address | null;
  signers: readonly SignerEntry[];
  transactionIndex: bigint;
}
export type SendOutcome =
  | { status: 'confirmed'; signature: Signature; slot: bigint }
  | { status: 'failed'; signature: Signature; error: unknown }
  | { status: 'unknown'; signature: Signature; lastValidBlockHeight: bigint; cause?: unknown };
export type CheckedOutcome = SendOutcome | { status: 'expired' };

/** assumed: ProgramConfig is passed as its decoded fields plus its address */
export interface ProgramConfigInput {
  address: Address;
  smartAccountIndex: bigint;
  treasury: Address;
  authority: Address;
  smartAccountCreationFee: bigint;
}
/** assumed field names for a decoded SpendingLimit (mainnet layout) plus its own address */
export interface SpendingLimitView {
  address: Address;
  settings: Address;
  seed: Address;
  accountIndex: number;
  mint: Address;
  amount: bigint;
  period: unknown;
  remainingAmount: bigint;
  lastReset: bigint;
  bump: number;
  signers: readonly Address[];
  destinations: readonly Address[];
  expiration: bigint;
}
type MaybePromise<T> = T | Promise<T>;
/** derive* may return the address or kit's [address, bump] tuple */
type Pda = Address | readonly [Address, number];

export interface SpecApi {
  fetchAccount(rpc: unknown, settings: Address): Promise<AccountSnapshot>;
  fetchBalances(
    rpc: unknown,
    vault: Address,
  ): Promise<{ lamports: bigint; tokens: { mint: Address; tokenProgram: Address; amount: bigint; decimals: number }[] }>;
  fetchSpendingLimits(rpc: unknown, settings: Address): Promise<SpendingLimitView[]>;
  deriveSettings(accountIndex: bigint): MaybePromise<Pda>;
  deriveVault(settings: Address, vaultIndex: number): MaybePromise<Pda>;
  deriveSpendingLimit: (settings: Address, seed: Address) => MaybePromise<Pda>;
  buildVaultTransaction(args: {
    account: AccountSnapshot;
    vaultIndex: number;
    signers: readonly TransactionSigner[];
    instructions: readonly Instruction[];
  }): MaybePromise<Instruction>;
  wrapTransaction(
    args: { account: AccountSnapshot; vaultIndex: number; signers: readonly TransactionSigner[] } & (
      | { message: TransactionMessage }
      | { instructions: readonly Instruction[] }
    ),
  ): MaybePromise<{ instruction: Instruction; stripped: { unitLimit?: number | bigint; unitPrice?: number | bigint; priorityFeeLamports?: bigint } }>;
  buildTransfer(args: {
    account: AccountSnapshot;
    vaultIndex: number;
    signers: readonly TransactionSigner[];
    mint: Address | 'SOL';
    to: Address;
    amount: bigint;
    decimals: number;
    tokenProgram?: Address;
    createRecipientAta: boolean;
  }): MaybePromise<Instruction>;
  buildSetSigners(args: {
    account: AccountSnapshot;
    signers: readonly TransactionSigner[];
    newSigners: readonly SignerEntry[];
    newThreshold: number;
    rentPayer: TransactionSigner;
  }): MaybePromise<Instruction>;
  buildRemoveSpendingLimit(args: {
    account: AccountSnapshot;
    signers: readonly TransactionSigner[];
    spendingLimit: Address;
    rentPayer: TransactionSigner;
  }): MaybePromise<Instruction>;
  buildCreateAccount(args: {
    programConfig: ProgramConfigInput;
    creator: TransactionSigner;
    signers: readonly SignerEntry[];
    threshold: number;
    timeLock: number;
    settingsAuthority: Address | null;
    rentCollector: Address | null;
    memo: string | null;
    candidateCount?: number;
  }): MaybePromise<{ instruction: Instruction; candidates: Address[]; computeUnitLimit: number | bigint }>;
  resolveCreatedAccount(rpc: unknown, signature: Signature, candidates: readonly Address[]): Promise<Address>;
  send(
    rpc: unknown,
    args: {
      instructions: readonly Instruction[];
      feePayer: TransactionSigner;
      signers: readonly TransactionSigner[];
      computeUnitLimit?: number;
      priorityFeeLamports?: bigint;
    },
  ): Promise<SendOutcome>;
  checkOutcome(rpc: unknown, signature: Signature, lastValidBlockHeight: bigint): Promise<CheckedOutcome>;
  MigrationSdkError?: abstract new (...args: never[]) => Error;
}

export const sdk = impl as unknown as SpecApi;

/** Error codes of spec §4, plus SIGNER_MODIFIED_TRANSACTION (review fix 4: wallet signers in `send`). */
export const ERROR_CODES = [
  'ACCOUNT_NOT_FOUND',
  'NOT_A_SMART_ACCOUNT',
  'INSUFFICIENT_SIGNERS',
  'SYNC_NOT_ALLOWED',
  'INVALID_SIGNER_SET',
  'SIMULATION_FAILED',
  'UNSUPPORTED_TRANSACTION',
  'TX_TOO_LARGE',
  'INVALID_ARGUMENT',
  'SIGNER_MODIFIED_TRANSACTION',
] as const;
export type ErrorCode = (typeof ERROR_CODES)[number];

export const pdaAddress = (p: Pda): Address => (typeof p === 'string' ? p : p[0]);
