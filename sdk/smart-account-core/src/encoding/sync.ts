import { type Address, type Instruction, isSignerRole, isWritableRole } from '@solana/kit';
import { SmartAccountError } from '../errors.ts';
import { compareBytes, concat, pubkey, u16, u8 } from './bytes.ts';

export interface SyncKey {
  address: Address;
  signer: boolean;
  writable: boolean;
}

export interface CompiledSync {
  /** Remaining accounts of `executeTransactionSync`: members first, then the inner instructions' accounts. */
  accounts: SyncKey[];
  /** `SmallVec<u8, CompiledInstruction>` */
  message: Uint8Array;
}

const tooLarge = (msg: string) => new SmartAccountError('TX_TOO_LARGE', msg);

function upsert(keys: SyncKey[], k: SyncKey): void {
  const existing = keys.find((e) => e.address === k.address);
  if (existing) {
    existing.signer ||= k.signer;
    existing.writable ||= k.writable;
  } else keys.push({ ...k });
}

/**
 * Inner keys sorted signer → writable → raw pubkey bytes, with the vault sorted as a signer and then
 * demoted; members are placed first and flags are OR-merged.
 */
export function compileSync(vault: Address, members: readonly Address[], instructions: readonly Instruction[]): CompiledSync {
  if (instructions.length > 0xff) throw tooLarge(`${instructions.length} inner instructions exceed 255`);
  const inner: SyncKey[] = [];
  for (const ix of instructions) {
    if (!inner.some((k) => k.address === ix.programAddress)) {
      inner.push({ address: ix.programAddress, signer: false, writable: false });
    }
    for (const meta of ix.accounts ?? []) {
      if ('lookupTableAddress' in meta) {
        throw new SmartAccountError('UNSUPPORTED_TRANSACTION', 'address lookup table accounts are not supported');
      }
      upsert(inner, { address: meta.address, signer: isSignerRole(meta.role), writable: isWritableRole(meta.role) });
    }
  }
  const bytes = new Map(inner.map((k) => [k.address, pubkey(k.address)]));
  inner.sort((a, b) => {
    if (a.signer !== b.signer) return a.signer ? -1 : 1;
    if (a.writable !== b.writable) return a.writable ? -1 : 1;
    return compareBytes(bytes.get(a.address) ?? new Uint8Array(), bytes.get(b.address) ?? new Uint8Array());
  });

  const accounts: SyncKey[] = members.map((m) => ({ address: m, signer: true, writable: false }));
  for (const k of inner) upsert(accounts, k);
  const vaultKey = accounts.find((k) => k.address === vault);
  if (vaultKey) vaultKey.signer = false;
  if (accounts.length > 0x100) throw tooLarge(`${accounts.length} accounts exceed the u8 account index`);

  const indexOf = (a: Address): number => accounts.findIndex((k) => k.address === a);
  const compiled = instructions.map((ix) => {
    const data = ix.data ?? new Uint8Array();
    if (data.length > 0xffff) throw tooLarge(`instruction data of ${data.length} bytes exceeds 65535`);
    const accountIndexes = (ix.accounts ?? []).map((m) => indexOf(m.address));
    if (accountIndexes.length > 0xff) throw tooLarge('an inner instruction has more than 255 accounts');
    return concat(u8(indexOf(ix.programAddress)), u8(accountIndexes.length), Uint8Array.from(accountIndexes), u16(data.length), data);
  });
  return { accounts, message: concat(u8(compiled.length), ...compiled) };
}
