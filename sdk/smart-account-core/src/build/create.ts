import { AccountRole, type Address, type Instruction, type ReadonlyUint8Array } from '@solana/kit';
import { deriveProgramConfig, deriveSettingsWithBump } from '../derive.ts';
import { SmartAccountError } from '../errors.ts';
import { decodeProgramConfig } from '../encoding/accounts.ts';
import { SMART_ACCOUNT_PROGRAM_ADDRESS, SYSTEM_PROGRAM_ADDRESS } from '../encoding/constants.ts';
import { encodeCreateSmartAccountData } from '../encoding/instructions.ts';
import type { SettingsAddress, SignerEntry, SignerInput } from '../types.ts';
import { checkSignerSet, meta, signerMeta } from './common.ts';

export const MAX_CANDIDATES = 59;

export interface CreateAccountInput {
  /** Decoded ProgramConfig, or the raw ProgramConfig account (e.g. from kit's `fetchEncodedAccount`). */
  programConfig: { smartAccountIndex: bigint; treasury: Address } | { data: ReadonlyUint8Array };
  /** Signs and pays for the creation. */
  creator: SignerInput;
  signers: readonly SignerEntry[];
  threshold: number;
  timeLock?: number;
  /** null (default) = autonomous */
  settingsAuthority?: Address | null;
  rentCollector?: Address | null;
  memo?: string | null;
  /** Candidate settings addresses appended so the call survives concurrent creations. Default 25, max 59. */
  candidateCount?: number;
}

export async function buildCreateAccount(
  input: CreateAccountInput,
): Promise<{ instruction: Instruction; candidates: SettingsAddress[]; computeUnitLimit: number }> {
  const masks = checkSignerSet(input.signers, input.threshold);
  const count = input.candidateCount ?? 25;
  if (!Number.isInteger(count) || count < 1 || count > MAX_CANDIDATES) {
    throw new SmartAccountError('INVALID_ARGUMENT', `candidateCount must be an integer in [1, ${MAX_CANDIDATES}]`);
  }
  let config: { smartAccountIndex: bigint; treasury: Address };
  if ('data' in input.programConfig) {
    try {
      config = decodeProgramConfig(new Uint8Array(input.programConfig.data));
    } catch (e) {
      throw new SmartAccountError('INVALID_ARGUMENT', 'programConfig is not a ProgramConfig account', e);
    }
  } else config = input.programConfig;

  const derived = await Promise.all(
    Array.from({ length: count }, (_, i) => deriveSettingsWithBump(config.smartAccountIndex + 1n + BigInt(i))),
  );
  const candidates = derived.map(([a]) => a);
  const minBump = Math.min(...derived.map(([, bump]) => bump));
  const data = encodeCreateSmartAccountData({
    settingsAuthority: input.settingsAuthority ?? null,
    threshold: input.threshold,
    signers: input.signers.map((s, i) => ({ key: s.key, mask: masks[i] ?? 0 })),
    timeLock: input.timeLock ?? 0,
    rentCollector: input.rentCollector ?? null,
    memo: input.memo ?? null,
  });
  const instruction: Instruction = {
    programAddress: SMART_ACCOUNT_PROGRAM_ADDRESS,
    accounts: [
      meta(await deriveProgramConfig(), AccountRole.WRITABLE),
      meta(config.treasury, AccountRole.WRITABLE),
      signerMeta(input.creator, true),
      meta(SYSTEM_PROGRAM_ADDRESS, AccountRole.READONLY),
      meta(SMART_ACCOUNT_PROGRAM_ADDRESS, AccountRole.READONLY),
      ...candidates.map((c) => meta(c, AccountRole.WRITABLE)),
    ],
    data,
  };
  return { instruction, candidates, computeUnitLimit: 30_000 + (255 - minBump) * 1_600 };
}
