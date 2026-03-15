import { PublicKey } from "@solana/web3.js";
import { bignum } from "@metaplex-foundation/beet";
import { createCreateSessionKeyInstruction, PROGRAM_ID } from "../generated";
import { patchInstructionEvd } from "../utils";

export function createSessionKey({
  settingsPda,
  signer,
  args,
  extraVerificationData,
  programId = PROGRAM_ID,
}: {
  settingsPda: PublicKey;
  signer: PublicKey;
  args: {
    sessionKey: PublicKey;
    sessionKeyExpiration: bigint | number;
  };
  extraVerificationData: Uint8Array | null;
  programId?: PublicKey;
}) {
  const ix = createCreateSessionKeyInstruction(
    {
      settings: settingsPda,
      signer,
      program: programId,
    },
    {
      args: {
        sessionKey: args.sessionKey,
        sessionKeyExpiration: args.sessionKeyExpiration as bignum,
      },
      extraVerificationData: null,
    },
    programId
  );
  if (extraVerificationData && extraVerificationData.length > 0) {
    // External signer — verified via precompile/syscall, NOT a native tx signer
    patchInstructionEvd(ix, extraVerificationData);
  } else {
    // Native signer — must be a native tx signer
    const signerMeta = ix.keys.find((k) => k.pubkey.equals(signer));
    if (signerMeta) signerMeta.isSigner = true;
  }
  return ix;
}
