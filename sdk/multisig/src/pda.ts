import { PublicKey } from "@solana/web3.js";
import invariant from "invariant";
import { PROGRAM_ID } from "./generated";
import { toU32Bytes, toU64Bytes, toU8Bytes, toUtfBytes } from "./utils";

const SEED_PREFIX = toUtfBytes("smart_account");
const SEED_PROGRAM_CONFIG = toUtfBytes("program_config");
const SEED_SETTINGS = toUtfBytes("settings");
const SEED_SMART_ACCOUNT = toUtfBytes("smart_account");
const SEED_TRANSACTION = toUtfBytes("transaction");
const SEED_PROPOSAL = toUtfBytes("proposal");
const SEED_BATCH_TRANSACTION = toUtfBytes("batch_transaction");
const SEED_EPHEMERAL_SIGNER = toUtfBytes("ephemeral_signer");
const SEED_SPENDING_LIMIT = toUtfBytes("spending_limit");

export function getProgramConfigPda({
  programId = PROGRAM_ID,
}: {
  programId?: PublicKey;
}): [PublicKey, number] {
  return PublicKey.findProgramAddressSync(
    [SEED_PREFIX, SEED_PROGRAM_CONFIG],
    programId
  );
}

export function getSettingsPda({
  createKey,
  programId = PROGRAM_ID,
}: {
  createKey: PublicKey;
  programId?: PublicKey;
}): [PublicKey, number] {
  return PublicKey.findProgramAddressSync(
    [SEED_PREFIX, SEED_SETTINGS, createKey.toBytes()],
    programId
  );
}

export function getSmartAccountPda({
  settingsPda,
  /** Authority index. */
  accountIndex,
  programId = PROGRAM_ID,
}: {
  settingsPda: PublicKey;
  accountIndex: number;
  programId?: PublicKey;
}): [PublicKey, number] {
  invariant(accountIndex >= 0 && accountIndex < 256, "Invalid vault index");

  return PublicKey.findProgramAddressSync(
    [
      SEED_PREFIX,
      settingsPda.toBytes(),
      SEED_SMART_ACCOUNT,
      toU8Bytes(accountIndex),
    ],
    programId
  );
}

export function getEphemeralSignerPda({
  transactionPda,
  ephemeralSignerIndex,
  programId = PROGRAM_ID,
}: {
  transactionPda: PublicKey;
  ephemeralSignerIndex: number;
  programId?: PublicKey;
}): [PublicKey, number] {
  return PublicKey.findProgramAddressSync(
    [
      SEED_PREFIX,
      transactionPda.toBytes(),
      SEED_EPHEMERAL_SIGNER,
      toU8Bytes(ephemeralSignerIndex),
    ],
    programId
  );
}

export function getTransactionPda({
  settingsPda,
  transactionIndex,
  programId = PROGRAM_ID,
}: {
  settingsPda: PublicKey;
  /** Transaction index. */
  transactionIndex: bigint;
  programId?: PublicKey;
}): [PublicKey, number] {
  return PublicKey.findProgramAddressSync(
    [
      SEED_PREFIX,
      settingsPda.toBytes(),
      SEED_TRANSACTION,
      toU64Bytes(transactionIndex),
    ],
    programId
  );
}

export function getProposalPda({
  settingsPda,
  transactionIndex,
  programId = PROGRAM_ID,
}: {
  settingsPda: PublicKey;
  /** Transaction index. */
  transactionIndex: bigint;
  programId?: PublicKey;
}): [PublicKey, number] {
  return PublicKey.findProgramAddressSync(
    [
      SEED_PREFIX,
      settingsPda.toBytes(),
      SEED_TRANSACTION,
      toU64Bytes(transactionIndex),
      SEED_PROPOSAL,
    ],
    programId
  );
}

export function getBatchTransactionPda({
  settingsPda,
  batchIndex,
  transactionIndex,
  programId = PROGRAM_ID,
}: {
  settingsPda: PublicKey;
  batchIndex: bigint;
  transactionIndex: number;
  programId?: PublicKey;
}): [PublicKey, number] {
  return PublicKey.findProgramAddressSync(
    [
      SEED_PREFIX,
      settingsPda.toBytes(),
      SEED_TRANSACTION,
      toU64Bytes(batchIndex),
      SEED_BATCH_TRANSACTION,
      toU32Bytes(transactionIndex),
    ],
    programId
  );
}

export function getSpendingLimitPda({
  settingsPda,
  seed,
  programId = PROGRAM_ID,
}: {
  settingsPda: PublicKey;
  seed: PublicKey;
  programId?: PublicKey;
}): [PublicKey, number] {
  return PublicKey.findProgramAddressSync(
    [
      SEED_PREFIX,
      settingsPda.toBytes(),
      SEED_SPENDING_LIMIT,
      seed.toBytes(),
    ],
    programId
  );
}
