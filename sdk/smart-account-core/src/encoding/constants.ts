import { type Address, address } from '@solana/kit';
import type { SettingsAddress, SpendingLimitAddress, VaultAddress } from '../types.ts';
import { fromHex } from './bytes.ts';

export const SMART_ACCOUNT_PROGRAM_ADDRESS = address('SMRTzfY6DfH5ik3TKiyLFfXexV8uSG3d2UksSCYdunG');
export const SYSTEM_PROGRAM_ADDRESS = address('11111111111111111111111111111111');
export const TOKEN_PROGRAM_ADDRESS = address('TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA');
export const TOKEN_2022_PROGRAM_ADDRESS = address('TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb');
export const ASSOCIATED_TOKEN_PROGRAM_ADDRESS = address('ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL');
export const COMPUTE_BUDGET_PROGRAM_ADDRESS = address('ComputeBudget111111111111111111111111111111');

/** Anchor discriminators: sha256("global:<ix>")[..8] and sha256("account:<Name>")[..8]. */
export const DISC = {
  createSmartAccount: fromHex('c566fde74d543211'),
  executeTransactionSync: fromHex('2b66f859e7616886'),
  executeSettingsTransactionSync: fromHex('8ad140a34f43e94c'),
  settings: fromHex('dfb3a3beb1e043ad'),
  spendingLimit: fromHex('0ac91ba0dac3de98'),
  programConfig: fromHex('c4d25ae790958c3f'),
} as const;

export const SEED = {
  prefix: 'smart_account',
  settings: 'settings',
  programConfig: 'program_config',
  spendingLimit: 'spending_limit',
} as const;

export const asSettings = (a: Address): SettingsAddress => a as SettingsAddress;
export const asVault = (a: Address): VaultAddress => a as VaultAddress;
export const asSpendingLimit = (a: Address): SpendingLimitAddress => a as SpendingLimitAddress;
