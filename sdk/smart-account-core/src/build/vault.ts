import { AccountRole, type Address, type Instruction, isSignerRole } from '@solana/kit';
import { deriveAssociatedTokenAccount, deriveVault } from '../derive.ts';
import { SmartAccountError } from '../errors.ts';
import { reader } from '../encoding/bytes.ts';
import {
  ASSOCIATED_TOKEN_PROGRAM_ADDRESS,
  COMPUTE_BUDGET_PROGRAM_ADDRESS,
  SMART_ACCOUNT_PROGRAM_ADDRESS,
  SYSTEM_PROGRAM_ADDRESS,
  TOKEN_PROGRAM_ADDRESS,
} from '../encoding/constants.ts';
import {
  ATA_CREATE_IDEMPOTENT_DATA,
  encodeSystemTransferData,
  encodeTransferCheckedData,
  encodeVaultSyncData,
} from '../encoding/instructions.ts';
import { compileSync } from '../encoding/sync.ts';
import type { AccountSnapshot, SignerInput } from '../types.ts';
import { checkTransactionSigners, keyOf, meta, requireSync, signerMeta } from './common.ts';

const MAX_ADDRESSES = 64;

export interface VaultTransactionInput {
  account: AccountSnapshot;
  vaultIndex: number;
  /** Signers of the outer transaction, in the order they are passed to the program. */
  signers: readonly SignerInput[];
}

/** One `executeTransactionSync` that has the vault execute `instructions`. */
export async function buildVaultTransaction(
  input: VaultTransactionInput & { instructions: readonly Instruction[] },
): Promise<Instruction> {
  const { account, vaultIndex, signers, instructions } = input;
  requireSync(account);
  checkTransactionSigners(account, signers);
  if (instructions.length === 0) throw new SmartAccountError('INVALID_ARGUMENT', 'no instructions to execute');
  const vault = await deriveVault(account.settings, vaultIndex);
  const members = signers.map(keyOf);
  for (const ix of instructions) {
    for (const m of ix.accounts ?? []) {
      if (isSignerRole(m.role) && m.address !== vault && !members.includes(m.address)) {
        throw new SmartAccountError(
          'UNSUPPORTED_TRANSACTION',
          `inner instruction for ${ix.programAddress} requires a signature from ${m.address}, which is neither the vault nor a transaction signer`,
        );
      }
    }
  }
  const compiled = compileSync(vault, members, instructions);
  const unique = new Set([account.settings, SMART_ACCOUNT_PROGRAM_ADDRESS, ...compiled.accounts.map((k) => k.address)]);
  if (unique.size > MAX_ADDRESSES) {
    throw new SmartAccountError('TX_TOO_LARGE', `${unique.size} addresses exceed the v1 limit of ${MAX_ADDRESSES}`);
  }
  const bySigner = new Map(signers.map((s) => [keyOf(s), s]));
  return {
    programAddress: SMART_ACCOUNT_PROGRAM_ADDRESS,
    accounts: [
      meta(account.settings, AccountRole.READONLY),
      meta(SMART_ACCOUNT_PROGRAM_ADDRESS, AccountRole.READONLY),
      ...compiled.accounts.map((k) => {
        const s = bySigner.get(k.address);
        if (s !== undefined) return signerMeta(s, k.writable);
        return meta(k.address, k.writable ? AccountRole.WRITABLE : AccountRole.READONLY);
      }),
    ],
    data: encodeVaultSyncData(vaultIndex, signers.length, compiled.message),
  };
}

export interface StrippedComputeBudget {
  unitLimit?: number;
  /** micro-lamports per compute unit */
  unitPrice?: bigint;
  /** ceil(unitPrice × unitLimit / 1,000,000), set when both are present: the total to pass as `send`'s `priorityFeeLamports`. */
  priorityFeeLamports?: bigint;
}

export type WrapInput = VaultTransactionInput &
  (
    | { message: { readonly instructions: readonly Instruction[] } }
    | { instructions: readonly Instruction[] }
  );

/**
 * Removes compute-budget instructions (reporting their values) and wraps the rest with `buildVaultTransaction`.
 * Heap-frame and loaded-accounts-data-size instructions are dropped without being reported.
 * A message's fee payer is not carried over; `send` sets its own.
 */
export async function wrapTransaction(input: WrapInput): Promise<{ instruction: Instruction; stripped: StrippedComputeBudget }> {
  const all = 'message' in input ? input.message.instructions : input.instructions;
  const stripped: StrippedComputeBudget = {};
  const kept: Instruction[] = [];
  for (const ix of all) {
    if (ix.programAddress !== COMPUTE_BUDGET_PROGRAM_ADDRESS) {
      kept.push(ix);
      continue;
    }
    const data = new Uint8Array(ix.data ?? []);
    const bad = () => new SmartAccountError('UNSUPPORTED_TRANSACTION', 'malformed or duplicate compute-budget instruction');
    if (data[0] === 2) {
      if (data.length !== 5 || stripped.unitLimit !== undefined) throw bad();
      stripped.unitLimit = reader(data, 1).u32();
    } else if (data[0] === 3) {
      if (data.length !== 9 || stripped.unitPrice !== undefined) throw bad();
      stripped.unitPrice = reader(data, 1).u64();
    } else if (data[0] !== 1 && data[0] !== 4) throw bad();
  }
  if (stripped.unitLimit !== undefined && stripped.unitPrice !== undefined) {
    stripped.priorityFeeLamports = (stripped.unitPrice * BigInt(stripped.unitLimit) + 999_999n) / 1_000_000n;
  }
  const instruction = await buildVaultTransaction({
    account: input.account,
    vaultIndex: input.vaultIndex,
    signers: input.signers,
    instructions: kept,
  });
  return { instruction, stripped };
}

export interface TransferInput extends VaultTransactionInput {
  mint: Address | 'SOL';
  /** Recipient wallet; for SPL tokens the tokens go to its associated token account. */
  to: Address;
  amount: bigint;
  /** Required for SPL tokens. */
  decimals?: number;
  /** Defaults to SPL Token. */
  tokenProgram?: Address;
  /** Create the recipient's associated token account (idempotently), paid by the vault. */
  createRecipientAta?: boolean;
}

export async function buildTransfer(input: TransferInput): Promise<Instruction> {
  const { amount, to, mint } = input;
  if (amount <= 0n) throw new SmartAccountError('INVALID_ARGUMENT', 'amount must be positive');
  const vault = await deriveVault(input.account.settings, input.vaultIndex);
  const base = { account: input.account, vaultIndex: input.vaultIndex, signers: input.signers };
  if (mint === 'SOL') {
    return buildVaultTransaction({
      ...base,
      instructions: [
        {
          programAddress: SYSTEM_PROGRAM_ADDRESS,
          accounts: [meta(vault, AccountRole.WRITABLE_SIGNER), meta(to, AccountRole.WRITABLE)],
          data: encodeSystemTransferData(amount),
        },
      ],
    });
  }
  if (input.decimals === undefined) throw new SmartAccountError('INVALID_ARGUMENT', 'decimals is required for SPL transfers');
  const tokenProgram = input.tokenProgram ?? TOKEN_PROGRAM_ADDRESS;
  const source = await deriveAssociatedTokenAccount(vault, mint, tokenProgram);
  const destination = await deriveAssociatedTokenAccount(to, mint, tokenProgram);
  const instructions: Instruction[] = [];
  if (input.createRecipientAta) {
    instructions.push({
      programAddress: ASSOCIATED_TOKEN_PROGRAM_ADDRESS,
      accounts: [
        meta(vault, AccountRole.WRITABLE_SIGNER),
        meta(destination, AccountRole.WRITABLE),
        meta(to, AccountRole.READONLY),
        meta(mint, AccountRole.READONLY),
        meta(SYSTEM_PROGRAM_ADDRESS, AccountRole.READONLY),
        meta(tokenProgram, AccountRole.READONLY),
      ],
      data: ATA_CREATE_IDEMPOTENT_DATA,
    });
  }
  instructions.push({
    programAddress: tokenProgram,
    accounts: [
      meta(source, AccountRole.WRITABLE),
      meta(mint, AccountRole.READONLY),
      meta(destination, AccountRole.WRITABLE),
      meta(vault, AccountRole.READONLY_SIGNER),
    ],
    data: encodeTransferCheckedData(amount, input.decimals),
  });
  return buildVaultTransaction({ ...base, instructions });
}
