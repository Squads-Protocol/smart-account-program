import type { Address, Signature, TransactionError, TransactionSigner } from '@solana/kit';

export type SettingsAddress = Address & { readonly __brand: 'Settings' };
export type VaultAddress = Address & { readonly __brand: 'Vault' };
export type SpendingLimitAddress = Address & { readonly __brand: 'SpendingLimit' };

export type Permission = 'initiate' | 'vote' | 'execute';

export interface SignerEntry {
  key: Address;
  permissions: readonly Permission[];
}

/** Decoded Settings account at a known slot. */
export interface AccountSnapshot {
  settings: SettingsAddress;
  slot: bigint;
  /** u16 */
  threshold: number;
  /** u32, seconds */
  timeLock: number;
  /** null = autonomous */
  settingsAuthority: Address | null;
  signers: readonly SignerEntry[];
  transactionIndex: bigint;
}

export type SpendingLimitPeriod = 'oneTime' | 'day' | 'week' | 'month';

export interface SpendingLimit {
  address: SpendingLimitAddress;
  settings: SettingsAddress;
  seed: Address;
  accountIndex: number;
  /** System program address (all zeroes) means SOL. */
  mint: Address;
  amount: bigint;
  period: SpendingLimitPeriod;
  remainingAmount: bigint;
  lastReset: bigint;
  bump: number;
  signers: readonly Address[];
  destinations: readonly Address[];
  expiration: bigint;
}

export interface ProgramConfig {
  smartAccountIndex: bigint;
  authority: Address;
  smartAccountCreationFee: bigint;
  treasury: Address;
}

export interface TokenBalance {
  /** The token account. */
  address: Address;
  mint: Address;
  tokenProgram: Address;
  amount: bigint;
  decimals: number;
}

export interface Balances {
  lamports: bigint;
  tokens: TokenBalance[];
}

export interface ProgramErrorInfo {
  err: TransactionError;
  instructionIndex: number | null;
  customCode: number | null;
  logs: readonly string[] | null;
}

export type SendOutcome =
  | { status: 'confirmed'; signature: Signature; slot: bigint }
  | { status: 'failed'; signature: Signature; error: ProgramErrorInfo }
  | {
      status: 'unknown';
      signature: Signature;
      lastValidBlockHeight: bigint;
      /** The RPC error that left the outcome unknown, when there was one. Never a reason to resend: call `checkOutcome`. */
      cause?: unknown;
    };

export type CheckedOutcome = SendOutcome | { status: 'expired'; signature: Signature };

/** A signer key, optionally with the kit signer that will sign for it. */
export type SignerInput = Address | TransactionSigner;
