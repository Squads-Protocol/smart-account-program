import {
  type Address,
  type GetAccountInfoApi,
  type GetBalanceApi,
  type GetMultipleAccountsApi,
  type GetProgramAccountsApi,
  type GetTokenAccountsByOwnerApi,
  type GetTransactionApi,
  getCompiledTransactionMessageDecoder,
  getInstructionsFromCompiledTransactionMessage,
  getTransactionDecoder,
  type Rpc,
  type Signature,
} from '@solana/kit';
import { deriveSettings } from './derive.ts';
import { SmartAccountError } from './errors.ts';
import { decodeMintDecimals, decodeSettings, decodeSpendingLimit, decodeTokenAccount } from './encoding/accounts.ts';
import { fromBase64, pubkey, toBase64Bytes } from './encoding/bytes.ts';
import { decodeCreateSmartAccountData } from './encoding/instructions.ts';
import {
  asSettings,
  asSpendingLimit,
  DISC,
  SMART_ACCOUNT_PROGRAM_ADDRESS,
  SYSTEM_PROGRAM_ADDRESS,
  TOKEN_2022_PROGRAM_ADDRESS,
  TOKEN_PROGRAM_ADDRESS,
} from './encoding/constants.ts';
import type { AccountSnapshot, Balances, SettingsAddress, SpendingLimit, TokenBalance, VaultAddress } from './types.ts';

/** Reads and decodes a Settings account (mainnet layout). */
export async function fetchAccount(rpc: Rpc<GetAccountInfoApi>, settings: Address): Promise<AccountSnapshot> {
  const { context, value } = await rpc.getAccountInfo(settings, { encoding: 'base64', commitment: 'confirmed' }).send();
  if (value === null) throw new SmartAccountError('ACCOUNT_NOT_FOUND', `no account at ${settings}`);
  const notSmartAccount = (why: string, cause?: unknown) =>
    new SmartAccountError('NOT_A_SMART_ACCOUNT', `${settings}: ${why}`, cause);
  if (value.owner !== SMART_ACCOUNT_PROGRAM_ADDRESS) throw notSmartAccount(`owned by ${value.owner}`);
  let decoded;
  try {
    decoded = decodeSettings(fromBase64(value.data[0]));
  } catch (e) {
    throw notSmartAccount('not a decodable Settings account', e);
  }
  if ((await deriveSettings(decoded.seed)) !== settings) throw notSmartAccount('address does not match its seed');
  return {
    settings: asSettings(settings),
    slot: context.slot,
    threshold: decoded.threshold,
    timeLock: decoded.timeLock,
    settingsAuthority: decoded.settingsAuthority,
    signers: decoded.signers,
    transactionIndex: decoded.transactionIndex,
  };
}

/**
 * Lamports plus every SPL Token and Token-2022 account owned by `vault`; raw amounts. Token or mint data that does not
 * decode can only come from a non-conforming `rpc` (the token programs enforce their layouts), so it is INVALID_ARGUMENT.
 */
export async function fetchBalances(
  rpc: Rpc<GetBalanceApi & GetTokenAccountsByOwnerApi & GetMultipleAccountsApi>,
  vault: VaultAddress,
): Promise<Balances> {
  const decode = <T>(what: string, run: () => T): T => {
    try {
      return run();
    } catch (e) {
      throw new SmartAccountError('INVALID_ARGUMENT', `the RPC returned ${what} that does not decode`, e);
    }
  };
  const [balance, ...tokenLists] = await Promise.all([
    rpc.getBalance(vault, { commitment: 'confirmed' }).send(),
    ...[TOKEN_PROGRAM_ADDRESS, TOKEN_2022_PROGRAM_ADDRESS].map(async (programId) => {
      const res = await rpc.getTokenAccountsByOwner(vault, { programId }, { encoding: 'base64', commitment: 'confirmed' }).send();
      return res.value.map((a) => ({
        address: a.pubkey,
        tokenProgram: programId,
        ...decode(`token account ${a.pubkey}`, () => decodeTokenAccount(fromBase64(a.account.data[0]))),
      }));
    }),
  ]);
  const accounts = tokenLists.flat();
  const mints = [...new Set(accounts.map((a) => a.mint))];
  const decimals = new Map<Address, number>();
  for (let i = 0; i < mints.length; i += 100) {
    const chunk = mints.slice(i, i + 100);
    const res = await rpc.getMultipleAccounts(chunk, { encoding: 'base64', commitment: 'confirmed' }).send();
    res.value.forEach((acc, j) => {
      const mint = chunk[j];
      if (acc === null || mint === undefined) throw new SmartAccountError('ACCOUNT_NOT_FOUND', `mint ${mint} not found`);
      decimals.set(mint, decode(`mint ${mint}`, () => decodeMintDecimals(fromBase64(acc.data[0]))));
    });
  }
  const tokens: TokenBalance[] = accounts.map((a) => {
    const d = decimals.get(a.mint);
    if (d === undefined) throw new SmartAccountError('ACCOUNT_NOT_FOUND', `no decimals for mint ${a.mint}`);
    return { address: a.address, mint: a.mint, tokenProgram: a.tokenProgram, amount: a.amount, decimals: d };
  });
  return { lamports: balance.value, tokens };
}

/** Every SpendingLimit account whose `settings` field equals `settings`. A program account that does not decode is NOT_A_SMART_ACCOUNT, as in `fetchAccount`. */
export async function fetchSpendingLimits(rpc: Rpc<GetProgramAccountsApi>, settings: Address): Promise<SpendingLimit[]> {
  const accounts = await rpc
    .getProgramAccounts(SMART_ACCOUNT_PROGRAM_ADDRESS, {
      encoding: 'base64',
      commitment: 'confirmed',
      filters: [
        { memcmp: { offset: 0n, encoding: 'base64', bytes: toBase64Bytes(DISC.spendingLimit) } },
        { memcmp: { offset: 8n, encoding: 'base64', bytes: toBase64Bytes(pubkey(settings)) } },
      ],
    })
    .send();
  return accounts.map((a) => {
    let d;
    try {
      d = decodeSpendingLimit(fromBase64(a.account.data[0]));
    } catch (e) {
      throw new SmartAccountError('NOT_A_SMART_ACCOUNT', `${a.pubkey} is not a decodable SpendingLimit account`, e);
    }
    if (d.settings !== settings) throw new SmartAccountError('NOT_A_SMART_ACCOUNT', `${a.pubkey} belongs to ${d.settings}`);
    return { ...d, address: asSpendingLimit(a.pubkey), settings: asSettings(d.settings) };
  });
}

/**
 * The candidate settings address that `signature` created. In order: the one candidate it funded (zero lamports before,
 * positive after); else the one whose balance rose in it (a griefer pre-funded the address below rent); else, reading
 * the candidates, the one Settings account whose seed derives its address and whose signers, threshold, time lock and
 * settings authority equal this transaction's `createSmartAccount` arguments (the address was pre-funded above rent).
 * Anything else, including two matches, is ACCOUNT_NOT_FOUND.
 */
export async function resolveCreatedAccount(
  rpc: Rpc<GetTransactionApi & GetMultipleAccountsApi>,
  signature: Signature,
  candidates: readonly SettingsAddress[],
): Promise<SettingsAddress> {
  const tx = await rpc
    .getTransaction(signature, { encoding: 'base64', maxSupportedTransactionVersion: 1, commitment: 'confirmed' })
    .send();
  if (tx === null || tx.meta === null) throw new SmartAccountError('ACCOUNT_NOT_FOUND', `transaction ${signature} not found`);
  if (tx.meta.err !== null) {
    throw new SmartAccountError('ACCOUNT_NOT_FOUND', `transaction ${signature} failed; no account was created`, tx.meta.err);
  }
  const { messageBytes } = getTransactionDecoder().decode(fromBase64(tx.transaction[0]));
  const message = getCompiledTransactionMessageDecoder().decode(messageBytes);
  const loaded = tx.meta.loadedAddresses;
  const keys = [...message.staticAccounts, ...(loaded?.writable ?? []), ...(loaded?.readonly ?? [])];
  const { preBalances, postBalances } = tx.meta;
  const balances = (c: SettingsAddress) => {
    const i = keys.indexOf(c);
    return i < 0 ? null : { pre: preBalances[i] ?? 0n, post: postBalances[i] ?? 0n };
  };
  const inTx = candidates.filter((c) => balances(c) !== null);
  const funded = inTx.filter((c) => balances(c)?.pre === 0n && (balances(c)?.post ?? 0n) > 0n);
  const rose = inTx.filter((c) => (balances(c)?.post ?? 0n) > (balances(c)?.pre ?? 0n));
  for (const set of [funded, rose]) if (set.length === 1 && set[0] !== undefined) return set[0];
  if (funded.length > 1 || rose.length > 1) {
    throw new SmartAccountError('ACCOUNT_NOT_FOUND', `transaction ${signature} funded ${Math.max(funded.length, rose.length)} of the candidates`);
  }
  const args = getInstructionsFromCompiledTransactionMessage(message, loaded)
    .filter((ix) => ix.programAddress === SMART_ACCOUNT_PROGRAM_ADDRESS)
    .map((ix) => decodeCreateSmartAccountData(new Uint8Array(ix.data ?? [])))
    .filter((a) => a !== null);
  const [want] = args;
  if (want === undefined || args.length !== 1 || inTx.length === 0) {
    throw new SmartAccountError('ACCOUNT_NOT_FOUND', `transaction ${signature} funded none of the candidates`);
  }
  const signerSet = (s: readonly { key: Address; mask: number }[]) => s.map((x) => `${x.key}:${x.mask}`).sort().join();
  const { value } = await rpc.getMultipleAccounts(inTx, { encoding: 'base64', commitment: 'confirmed' }).send();
  const matches: SettingsAddress[] = [];
  for (const [i, acc] of value.entries()) {
    const c = inTx[i];
    if (c === undefined || acc === null || acc.owner !== SMART_ACCOUNT_PROGRAM_ADDRESS) continue;
    let d;
    try {
      d = decodeSettings(fromBase64(acc.data[0]));
    } catch {
      continue;
    }
    const same =
      d.threshold === want.threshold &&
      d.timeLock === want.timeLock &&
      d.settingsAuthority === (want.settingsAuthority === SYSTEM_PROGRAM_ADDRESS ? null : want.settingsAuthority) &&
      signerSet(d.signers.map((x, j) => ({ key: x.key, mask: d.signerMasks[j] ?? 0 }))) === signerSet(want.signers);
    if (same && (await deriveSettings(d.seed)) === c) matches.push(c);
  }
  const [created] = matches;
  if (created === undefined || matches.length !== 1) {
    throw new SmartAccountError('ACCOUNT_NOT_FOUND', `transaction ${signature}: ${matches.length} candidates match its creation`);
  }
  return created;
}
