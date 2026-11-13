import { type Address, getProgramDerivedAddress } from '@solana/kit';
import { SmartAccountError } from './errors.ts';
import { pubkey, u128, u8 } from './encoding/bytes.ts';
import {
  ASSOCIATED_TOKEN_PROGRAM_ADDRESS,
  SEED,
  SMART_ACCOUNT_PROGRAM_ADDRESS,
  asSettings,
  asSpendingLimit,
  asVault,
} from './encoding/constants.ts';
import type { SettingsAddress, SpendingLimitAddress, VaultAddress } from './types.ts';

const pda = (seeds: (string | Uint8Array)[], programAddress: Address = SMART_ACCOUNT_PROGRAM_ADDRESS) =>
  getProgramDerivedAddress({ programAddress, seeds });

export async function deriveSettingsWithBump(accountIndex: bigint): Promise<[SettingsAddress, number]> {
  if (accountIndex < 0n || accountIndex >= 1n << 128n) {
    throw new SmartAccountError('INVALID_ARGUMENT', 'accountIndex must be a u128');
  }
  const [a, bump] = await pda([SEED.prefix, SEED.settings, u128(accountIndex)]);
  return [asSettings(a), bump];
}

/** Settings PDA: ["smart_account", "settings", u128 LE index]. */
export const deriveSettings = async (accountIndex: bigint): Promise<SettingsAddress> =>
  (await deriveSettingsWithBump(accountIndex))[0];

/** Vault PDA: ["smart_account", settings, "smart_account", u8 vaultIndex]. */
export async function deriveVault(settings: Address, vaultIndex: number): Promise<VaultAddress> {
  const [a] = await pda([SEED.prefix, pubkey(settings), SEED.prefix, u8(vaultIndex, 'vaultIndex')]);
  return asVault(a);
}

/** SpendingLimit PDA: ["smart_account", settings, "spending_limit", seed]. */
export async function deriveSpendingLimit(settings: Address, seed: Address): Promise<SpendingLimitAddress> {
  const [a] = await pda([SEED.prefix, pubkey(settings), SEED.spendingLimit, pubkey(seed)]);
  return asSpendingLimit(a);
}

/** ProgramConfig PDA: ["smart_account", "program_config"]. */
export const deriveProgramConfig = async (): Promise<Address> => (await pda([SEED.prefix, SEED.programConfig]))[0];

export const deriveAssociatedTokenAccount = async (owner: Address, mint: Address, tokenProgram: Address) =>
  (await pda([pubkey(owner), pubkey(tokenProgram), pubkey(mint)], ASSOCIATED_TOKEN_PROGRAM_ADDRESS))[0];
