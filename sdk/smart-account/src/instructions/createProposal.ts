import { PublicKey } from "@solana/web3.js";
import { createCreateProposalInstruction, PROGRAM_ID } from "../generated";
import { getProposalPda } from "../pda";

export function createProposal({
  settingsPda,
  creator,
  rentPayer,
  transactionIndex,
  isDraft = false,
  programId = PROGRAM_ID,
}: {
  settingsPda: PublicKey;
  /** Member of the multisig that is creating the proposal. */
  creator: PublicKey;
  /** Payer for the proposal account rent. If not provided, `creator` is used. */
  rentPayer?: PublicKey;
  transactionIndex: bigint;
  isDraft?: boolean;
  programId?: PublicKey;
}) {
  const [proposalPda] = getProposalPda({
    settingsPda,
    transactionIndex,
    programId,
  });

  if (transactionIndex > Number.MAX_SAFE_INTEGER) {
    throw new Error("transactionIndex is too large");
  }

  const ix = createCreateProposalInstruction(
    {
      creator,
      rentPayer: rentPayer ?? creator,
      consensusAccount: settingsPda,
      proposal: proposalPda,
      program: programId,
    },
    { args: { transactionIndex: Number(transactionIndex), draft: isDraft } },
    programId
  );
  const consensusMeta = ix.keys.find((k) => k.pubkey.equals(settingsPda));
  if (consensusMeta) consensusMeta.isWritable = true;
  const creatorMeta = ix.keys.find((k) => k.pubkey.equals(creator));
  if (creatorMeta) creatorMeta.isSigner = true;
  return ix;
}
