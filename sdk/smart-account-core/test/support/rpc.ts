// A kit-shaped RPC object (`rpc.method(...args).send()`) backed by plain handler functions, with a
// call log. Unknown methods fail loudly so a missing handler is obvious, not a hang.
import { type ReadonlyUint8Array, getBase58Decoder, getBase58Encoder, getBase64Encoder } from '@solana/kit';

export interface Call {
  method: string;
  args: unknown[];
}
export type Handlers = Record<string, (...args: never[]) => unknown>;

export function makeRpc(handlers: Handlers) {
  const calls: Call[] = [];
  const rpc = new Proxy(
    {},
    {
      get(_t, prop) {
        if (typeof prop !== 'string' || prop === 'then') return undefined;
        return (...args: unknown[]) => ({
          send: async () => {
            calls.push({ method: prop, args });
            const h = handlers[prop];
            if (h === undefined) throw new Error(`test RPC: method ${prop} is not implemented`);
            return (h as (...a: unknown[]) => unknown)(...args);
          },
        });
      },
    },
  );
  const count = (m: string) => calls.filter((c) => c.method === m).length;
  return { rpc, calls, count };
}

/** Decodes a wire transaction argument of sendTransaction/simulateTransaction. */
export function wireBytes(wire: unknown, config: unknown): Uint8Array {
  const enc =
    typeof config === 'object' && config !== null && 'encoding' in config ? String(config.encoding) : 'base58';
  if (typeof wire !== 'string') throw new Error('wire transaction must be a string');
  return new Uint8Array(enc === 'base64' ? getBase64Encoder().encode(wire) : getBase58Encoder().encode(wire));
}

export const cfg = (c: unknown): Record<string, unknown> =>
  typeof c === 'object' && c !== null ? (c as Record<string, unknown>) : {};

/** getProgramAccounts filter semantics (memcmp base58/base64, dataSize). */
export function matchesFilters(data: ReadonlyUint8Array, filters: unknown): boolean {
  return (Array.isArray(filters) ? filters : []).every((f: unknown) => {
    const flt = cfg(f);
    if (flt.dataSize !== undefined) return BigInt(data.length) === BigInt(flt.dataSize as bigint);
    const m = cfg(flt.memcmp);
    const bytes = Uint8Array.from(
      m.encoding === 'base64' ? getBase64Encoder().encode(String(m.bytes)) : getBase58Encoder().encode(String(m.bytes)),
    );
    const off = Number(m.offset ?? 0);
    return Buffer.compare(data.subarray(off, off + bytes.length), bytes) === 0;
  });
}

export interface RawAccount {
  data: Uint8Array;
  owner: string;
  lamports?: bigint;
}
/** Read-only fake RPC over a fixed account map (for fetch* decoding tests). */
export function accountsRpc(accounts: ReadonlyMap<string, RawAccount>, slot = 4242n) {
  const encode = (a: RawAccount, c: unknown) => {
    const enc = cfg(c).encoding;
    return {
      data: enc === 'base64' || enc === 'jsonParsed' ? [Buffer.from(a.data).toString('base64'), 'base64'] : getBase58Decoder().decode(a.data),
      executable: false,
      lamports: a.lamports ?? 1_000_000n,
      owner: a.owner,
      space: BigInt(a.data.length),
      rentEpoch: 0n,
    };
  };
  const one = (addr: string, c: unknown) => {
    const a = accounts.get(addr);
    return a === undefined ? null : encode(a, c);
  };
  return makeRpc({
    getAccountInfo: (addr: string, c: unknown) => ({ context: { slot }, value: one(addr, c) }),
    getMultipleAccounts: (addrs: string[], c: unknown) => ({ context: { slot }, value: addrs.map((x) => one(x, c)) }),
    getProgramAccounts: (program: string, c: unknown) => {
      const list = [...accounts]
        .filter(([, a]) => a.owner === program && matchesFilters(a.data, cfg(c).filters))
        .map(([pubkey, a]) => ({ pubkey, account: encode(a, c) }));
      return cfg(c).withContext === true ? { context: { slot }, value: list } : list;
    },
  } as never);
}
