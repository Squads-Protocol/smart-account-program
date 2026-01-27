import { PublicKey } from "@solana/web3.js";
import { createIncrementAccountIndexInstruction, PROGRAM_ID } from "../generated";

export function incrementAccountIndex({
  settingsPda,
  signer,
  programId = PROGRAM_ID,
}: {
  settingsPda: PublicKey;
  signer: PublicKey;
  programId?: PublicKey;
}) {
  return createIncrementAccountIndexInstruction(
    {
      settings: settingsPda,
      signer,
    },
    programId
  );
}
