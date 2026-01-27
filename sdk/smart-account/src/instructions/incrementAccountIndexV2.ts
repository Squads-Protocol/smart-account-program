import { PublicKey } from "@solana/web3.js";
import {
  ClientDataJsonReconstructionParams,
  createIncrementAccountIndexV2Instruction,
  PROGRAM_ID,
} from "../generated";

export function incrementAccountIndexV2({
  settingsPda,
  signerKey,
  clientDataParams = null,
  anchorRemainingAccounts,
  programId = PROGRAM_ID,
}: {
  settingsPda: PublicKey;
  signerKey: PublicKey;
  clientDataParams?: ClientDataJsonReconstructionParams | null;
  anchorRemainingAccounts?: { pubkey: PublicKey; isSigner: boolean; isWritable: boolean }[];
  programId?: PublicKey;
}) {
  return createIncrementAccountIndexV2Instruction(
    {
      settings: settingsPda,
      anchorRemainingAccounts,
    },
    {
      args: {
        signerKey,
        clientDataParams,
      },
    },
    programId
  );
}
