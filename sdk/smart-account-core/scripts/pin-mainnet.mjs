#!/usr/bin/env node
// Pins the mainnet Squads Smart Account Program binary, Anchor IDL and ProgramConfig account.
//
//   node scripts/pin-mainnet.mjs           # (re)write test/fixtures/mainnet/*
//   node scripts/pin-mainnet.mjs --check   # re-fetch and compare to PINNED.json; exit 1 on drift
//
// RPC: $SOLANA_RPC_URL, falling back to the public mainnet-beta endpoint.
import { createHash } from 'node:crypto';
import { mkdir, readFile, writeFile } from 'node:fs/promises';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { inflateSync } from 'node:zlib';
import {
  address,
  createAddressWithSeed,
  createSolanaRpc,
  getAddressDecoder,
  getProgramDerivedAddress,
} from '@solana/kit';

const PROGRAM_ID = address('SMRTzfY6DfH5ik3TKiyLFfXexV8uSG3d2UksSCYdunG');
const BPF_LOADER_UPGRADEABLE = 'BPFLoaderUpgradeab1e11111111111111111111111';
const PROGRAMDATA_HEADER_LEN = 45; // u32 tag(3) + u64 slot + Option<Pubkey> (1 + 32)
const OUT_DIR = join(dirname(fileURLToPath(import.meta.url)), '..', 'test', 'fixtures', 'mainnet');
const RPC_URL = process.env.SOLANA_RPC_URL || 'https://api.mainnet-beta.solana.com';
const CHECK = process.argv.includes('--check');

const rpc = createSolanaRpc(RPC_URL);
const addrDecoder = getAddressDecoder();
const sha256 = (buf) => createHash('sha256').update(buf).digest('hex');
const u32 = (b, o) => new DataView(b.buffer, b.byteOffset + o, 4).getUint32(0, true);
const u64 = (b, o) => new DataView(b.buffer, b.byteOffset + o, 8).getBigUint64(0, true);
const u128 = (b, o) => u64(b, o) | (u64(b, o + 8) << 64n);

async function getAccount(addr, minContextSlot) {
  const { context, value } = await rpc
    .getAccountInfo(addr, { encoding: 'base64', commitment: 'confirmed', ...(minContextSlot ? { minContextSlot } : {}) })
    .send();
  if (!value) return { slot: context.slot, account: null };
  return { slot: context.slot, account: { ...value, data: Buffer.from(value.data[0], 'base64') } };
}

async function fetchProgram() {
  const { slot, account: program } = await getAccount(PROGRAM_ID);
  if (!program) throw new Error(`program ${PROGRAM_ID} not found`);
  if (program.owner !== BPF_LOADER_UPGRADEABLE) throw new Error(`unexpected program owner ${program.owner}`);
  if (u32(program.data, 0) !== 2) throw new Error('program account is not UpgradeableLoaderState::Program');
  const programDataAddress = addrDecoder.decode(program.data.subarray(4, 36));

  const { account: pd } = await getAccount(programDataAddress, slot);
  if (!pd || u32(pd.data, 0) !== 3) throw new Error('programdata account missing or wrong tag');
  const lastDeploySlot = u64(pd.data, 4);
  const upgradeAuthority = pd.data[12] === 1 ? addrDecoder.decode(pd.data.subarray(13, 45)) : null;
  // The account is padded with zeros after the ELF; strip trailing zeros back to the ELF end
  // using the section header table (e_shoff + e_shnum * e_shentsize), which is the last thing in an sBPF ELF.
  const raw = pd.data.subarray(PROGRAMDATA_HEADER_LEN);
  if (raw.readUInt32BE(0) !== 0x7f454c46) throw new Error('programdata does not contain an ELF');
  const shoff = Number(raw.readBigUInt64LE(0x28));
  const shentsize = raw.readUInt16LE(0x3a);
  const shnum = raw.readUInt16LE(0x3c);
  const elfLen = shoff + shentsize * shnum;
  // Only trim if everything after the computed ELF end is zero padding; otherwise keep the full payload.
  const trimmable = elfLen <= raw.length && raw.subarray(elfLen).every((b) => b === 0);
  const elf = Buffer.from(trimmable ? raw.subarray(0, elfLen) : raw);
  return { fetchedAtSlot: slot, programDataAddress, lastDeploySlot, upgradeAuthority, elf, programDataLen: pd.data.length };
}

async function fetchIdl(minSlot) {
  const [base] = await getProgramDerivedAddress({ programAddress: PROGRAM_ID, seeds: [] });
  const idlAddress = await createAddressWithSeed({ baseAddress: base, programAddress: PROGRAM_ID, seed: 'anchor:idl' });
  const { account } = await getAccount(idlAddress, minSlot);
  if (!account) return { idlAddress, idl: null };
  const d = account.data;
  const authority = addrDecoder.decode(d.subarray(8, 40));
  const len = u32(d, 40);
  const json = inflateSync(d.subarray(44, 44 + len));
  return { idlAddress, idlAuthority: authority, idlJson: JSON.parse(json.toString('utf8')) };
}

function decodeProgramConfig(d) {
  // Layout per programs/squads_smart_account_program/src/state/program_config.rs
  return {
    discriminatorHex: Buffer.from(d.subarray(0, 8)).toString('hex'),
    smartAccountIndex: u128(d, 8).toString(),
    authority: addrDecoder.decode(d.subarray(24, 56)),
    smartAccountCreationFee: u64(d, 56).toString(),
    treasury: addrDecoder.decode(d.subarray(64, 96)),
    reservedLen: d.length - 96,
  };
}

async function fetchProgramConfig(minSlot) {
  const [pc] = await getProgramDerivedAddress({
    programAddress: PROGRAM_ID,
    seeds: ['smart_account', 'program_config'],
  });
  const { slot, account } = await getAccount(pc, minSlot);
  if (!account) throw new Error(`program config ${pc} not found`);
  return {
    address: pc,
    fetchedAtSlot: slot.toString(),
    owner: account.owner,
    lamports: account.lamports.toString(),
    dataLen: account.data.length,
    dataBase64: account.data.toString('base64'),
    decoded: decodeProgramConfig(account.data),
  };
}

const stableJson = (v) => JSON.stringify(v, null, 2) + '\n';

async function main() {
  const prog = await fetchProgram();
  const idl = await fetchIdl(prog.fetchedAtSlot);
  const pinned = {
    programId: PROGRAM_ID,
    programDataAddress: prog.programDataAddress,
    lastDeploySlot: prog.lastDeploySlot.toString(),
    fetchedAtSlot: prog.fetchedAtSlot.toString(),
    upgradeAuthority: prog.upgradeAuthority,
    soSha256: sha256(prog.elf),
    soLength: prog.elf.length,
    idlAddress: idl.idlAddress,
    idlSha256: idl.idlJson ? sha256(stableJson(idl.idlJson)) : null,
    idlAuthority: idl.idlAuthority ?? null,
    rpc: RPC_URL.startsWith('https://api.mainnet-beta.solana.com') ? RPC_URL : '(SOLANA_RPC_URL)',
  };

  if (CHECK) {
    const prev = JSON.parse(await readFile(join(OUT_DIR, 'PINNED.json'), 'utf8'));
    const keys = ['programId', 'programDataAddress', 'lastDeploySlot', 'soSha256', 'idlSha256', 'idlAuthority', 'upgradeAuthority'];
    const drift = keys.filter((k) => prev[k] !== pinned[k]);
    const soOnDisk = sha256(await readFile(join(OUT_DIR, 'smart_account_program.so')));
    if (soOnDisk !== prev.soSha256) drift.push('smart_account_program.so (on disk vs PINNED.soSha256)');
    if (drift.length) {
      console.error('Mainnet drift detected:');
      for (const k of drift) console.error(`  ${k}: pinned=${prev[k]} live=${pinned[k]}`);
      process.exit(1);
    }
    console.log(`OK: mainnet matches PINNED.json (lastDeploySlot ${pinned.lastDeploySlot}, checked at slot ${pinned.fetchedAtSlot})`);
    return;
  }

  const programConfig = await fetchProgramConfig(prog.fetchedAtSlot);
  await mkdir(OUT_DIR, { recursive: true });
  await writeFile(join(OUT_DIR, 'smart_account_program.so'), prog.elf);
  if (idl.idlJson) await writeFile(join(OUT_DIR, 'idl.json'), stableJson(idl.idlJson));
  else console.warn(`No Anchor IDL account at ${idl.idlAddress}`);
  await writeFile(join(OUT_DIR, 'program_config.json'), stableJson(programConfig));
  await writeFile(join(OUT_DIR, 'PINNED.json'), stableJson(pinned));
  console.log(stableJson(pinned));
}

main().catch((e) => {
  console.error(e);
  process.exit(2);
});
