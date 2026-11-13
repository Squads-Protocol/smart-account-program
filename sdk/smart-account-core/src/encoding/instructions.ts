import type { Address } from '@solana/kit';
import { bytesVec, concat, option, pubkey, reader, string, u16, u32, u64, u8, vec } from './bytes.ts';
import { hasDiscriminator } from './accounts.ts';
import { DISC } from './constants.ts';

export type SettingsAction =
  | { type: 'addSigner'; key: Address; mask: number }
  | { type: 'removeSigner'; key: Address }
  | { type: 'changeThreshold'; threshold: number }
  | { type: 'removeSpendingLimit'; spendingLimit: Address };

export function encodeSettingsAction(a: SettingsAction): Uint8Array {
  switch (a.type) {
    case 'addSigner':
      return concat(u8(0), pubkey(a.key), u8(a.mask));
    case 'removeSigner':
      return concat(u8(1), pubkey(a.key));
    case 'changeThreshold':
      return concat(u8(2), u16(a.threshold, 'threshold'));
    case 'removeSpendingLimit':
      return concat(u8(5), pubkey(a.spendingLimit));
  }
}

/** `SyncSettingsTransactionArgs { numSigners: u8, actions: Vec<SettingsAction>, memo: Option<String> }` */
export const encodeSettingsSyncData = (numSigners: number, actions: readonly SettingsAction[], memo: string | null) =>
  concat(
    DISC.executeSettingsTransactionSync,
    u8(numSigners, 'number of signers'),
    vec(actions.map(encodeSettingsAction)),
    option(memo, string),
  );

/** `SyncTransactionArgs { accountIndex: u8, numSigners: u8, instructions: Vec<u8> }` */
export const encodeVaultSyncData = (vaultIndex: number, numSigners: number, compiled: Uint8Array) =>
  concat(DISC.executeTransactionSync, u8(vaultIndex, 'vaultIndex'), u8(numSigners, 'number of signers'), bytesVec(compiled));

export interface CreateSmartAccountArgs {
  settingsAuthority: Address | null;
  threshold: number;
  signers: readonly { key: Address; mask: number }[];
  timeLock: number;
  rentCollector: Address | null;
  memo: string | null;
}

export const encodeCreateSmartAccountData = (a: CreateSmartAccountArgs) =>
  concat(
    DISC.createSmartAccount,
    option(a.settingsAuthority, pubkey),
    u16(a.threshold, 'threshold'),
    vec(a.signers.map((s) => concat(pubkey(s.key), u8(s.mask)))),
    u32(a.timeLock, 'timeLock'),
    option(a.rentCollector, pubkey),
    option(a.memo, string),
  );

/** Decodes the fields of `createSmartAccount` data that end up in the Settings account; null if it is not that instruction. */
export function decodeCreateSmartAccountData(data: Uint8Array): Omit<CreateSmartAccountArgs, 'rentCollector' | 'memo'> | null {
  if (!hasDiscriminator(data, DISC.createSmartAccount)) return null;
  const r = reader(data, 8);
  const settingsAuthority = r.option(r.pubkey);
  const threshold = r.u16();
  const signers = r.vec(() => {
    const key = r.pubkey();
    const mask = r.u8();
    return { key, mask };
  });
  const timeLock = r.u32();
  return { settingsAuthority, threshold, signers, timeLock };
}

export const encodeSystemTransferData = (lamports: bigint) => concat(u32(2), u64(lamports, 'amount'));
export const encodeTransferCheckedData = (amount: bigint, decimals: number) =>
  concat(u8(12), u64(amount, 'amount'), u8(decimals, 'decimals'));
export const ATA_CREATE_IDEMPOTENT_DATA = Uint8Array.of(1);
