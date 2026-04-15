import { PublicKey } from "@solana/web3.js";
import { createRevokeSessionKeyInstruction, PROGRAM_ID } from "../generated";
import { patchInstructionEvd } from "../utils";

export function revokeSessionKey({
  settingsPda,
  authority,
  signerKey,
  extraVerificationData,
  programId = PROGRAM_ID,
}: {
  settingsPda: PublicKey;
  /** The authority revoking — either the external signer's key_id or the session key holder */
  authority: PublicKey;
  /** The key_id of the external signer whose session key to revoke */
  signerKey: PublicKey;
  /** Required when the external signer is revoking (precompile/syscall). Omit when session key holder revokes. */
  extraVerificationData?: Uint8Array | null;
  programId?: PublicKey;
}) {
  const ix = createRevokeSessionKeyInstruction(
    {
      settings: settingsPda,
      authority,
      program: programId,
    },
    {
      args: { signerKey },
      extraVerificationData: null,
    },
    programId
  );
  if (extraVerificationData && extraVerificationData.length > 0) {
    // External signer revoking — verified via precompile/syscall, NOT a native tx signer
    patchInstructionEvd(ix, extraVerificationData);
  } else {
    // Session key holder self-revoking — must be a native tx signer
    const authorityMeta = ix.keys.find((k) => k.pubkey.equals(authority));
    if (authorityMeta) authorityMeta.isSigner = true;
  }
  return ix;
}
