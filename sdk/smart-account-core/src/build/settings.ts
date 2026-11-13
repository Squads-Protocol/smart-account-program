import { AccountRole, type Address, type Instruction } from '@solana/kit';
import { maskOf } from '../encoding/accounts.ts';
import { SMART_ACCOUNT_PROGRAM_ADDRESS, SYSTEM_PROGRAM_ADDRESS } from '../encoding/constants.ts';
import { encodeSettingsSyncData, type SettingsAction } from '../encoding/instructions.ts';
import type { AccountSnapshot, SignerEntry, SignerInput } from '../types.ts';
import { checkSignerSet, checkTransactionSigners, keyOf, meta, requireAutonomous, requireSync, signerMeta } from './common.ts';

interface SettingsInput {
  account: AccountSnapshot;
  /** Current signers that sign this change. */
  signers: readonly SignerInput[];
  /** Pays for any reallocation and receives closed accounts' rent. Must sign. */
  rentPayer: SignerInput;
}

function settingsSyncInstruction(
  input: SettingsInput,
  actions: readonly SettingsAction[],
  extra: readonly Address[] = [],
): Instruction {
  requireAutonomous(input.account);
  requireSync(input.account);
  checkTransactionSigners(input.account, input.signers);
  return {
    programAddress: SMART_ACCOUNT_PROGRAM_ADDRESS,
    accounts: [
      meta(input.account.settings, AccountRole.WRITABLE),
      signerMeta(input.rentPayer, true),
      meta(SYSTEM_PROGRAM_ADDRESS, AccountRole.READONLY),
      meta(SMART_ACCOUNT_PROGRAM_ADDRESS, AccountRole.READONLY),
      ...input.signers.map((s) => signerMeta(s)),
      ...extra.map((a) => meta(a, AccountRole.WRITABLE)),
    ],
    data: encodeSettingsSyncData(input.signers.length, actions, null),
  };
}

/**
 * Actions: ChangeThreshold(1); AddSigner for new keys (in `newSigners` order); RemoveSigner + AddSigner for keys
 * whose permissions changed; RemoveSigner for dropped keys (on-chain order); ChangeThreshold(newThreshold).
 */
export function setSignersActions(current: readonly SignerEntry[], newSigners: readonly SignerEntry[], newThreshold: number) {
  const masks = checkSignerSet(newSigners, newThreshold);
  const wanted = newSigners.map((s, i) => ({ key: s.key, mask: masks[i] ?? 0 }));
  const currentMask = new Map(current.map((s) => [s.key, maskOf(s.permissions) ?? 0]));
  const actions: SettingsAction[] = [{ type: 'changeThreshold', threshold: 1 }];
  for (const w of wanted) if (!currentMask.has(w.key)) actions.push({ type: 'addSigner', ...w });
  for (const w of wanted) {
    const old = currentMask.get(w.key);
    if (old !== undefined && old !== w.mask) {
      actions.push({ type: 'removeSigner', key: w.key }, { type: 'addSigner', ...w });
    }
  }
  for (const c of current) {
    if (!wanted.some((w) => w.key === c.key)) actions.push({ type: 'removeSigner', key: c.key });
  }
  actions.push({ type: 'changeThreshold', threshold: newThreshold });
  return actions;
}

/** One `executeSettingsTransactionSync` after which the on-chain signer set equals `newSigners`. */
export function buildSetSigners(
  input: SettingsInput & { newSigners: readonly SignerEntry[]; newThreshold: number },
): Instruction {
  return settingsSyncInstruction(input, setSignersActions(input.account.signers, input.newSigners, input.newThreshold));
}

/** One `executeSettingsTransactionSync` with `RemoveSpendingLimit`; the closed account's rent goes to `rentPayer`. */
export function buildRemoveSpendingLimit(input: SettingsInput & { spendingLimit: Address }): Instruction {
  return settingsSyncInstruction(input, [{ type: 'removeSpendingLimit', spendingLimit: input.spendingLimit }], [input.spendingLimit]);
}
