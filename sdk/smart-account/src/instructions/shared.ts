import {
  AccountMeta,
  SYSVAR_INSTRUCTIONS_PUBKEY,
  TransactionInstruction,
} from "@solana/web3.js";

const INSTRUCTIONS_SYSVAR_META: AccountMeta = {
  pubkey: SYSVAR_INSTRUCTIONS_PUBKEY,
  isWritable: false,
  isSigner: false,
};

export function appendInstructionsSysvar(
  instruction: TransactionInstruction
): TransactionInstruction {
  instruction.keys = instruction.keys.filter(
    (account) => !account.pubkey.equals(SYSVAR_INSTRUCTIONS_PUBKEY)
  );
  instruction.keys.push(INSTRUCTIONS_SYSVAR_META);
  return instruction;
}
