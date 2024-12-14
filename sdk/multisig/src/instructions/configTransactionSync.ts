import { PublicKey, SystemProgram } from "@solana/web3.js";
import { ConfigAction, createConfigTransactionSyncInstruction, PROGRAM_ID } from "../generated";

export function configTransactionSync({
    multisigPda,
    signers,
    configActions,
    feePayer,
    memo,
    programId = PROGRAM_ID,
}: {
    multisigPda: PublicKey;
    signers: PublicKey[];
    configActions: ConfigAction[];
    feePayer: PublicKey;
    memo?: string;
    programId?: PublicKey;
}) {
    const ix = createConfigTransactionSyncInstruction(
        {
            multisig: multisigPda,
            rentPayer: feePayer ?? undefined,
            systemProgram: SystemProgram.programId,
        },
        {
            args: {
                numSigners: signers.length,
                actions: configActions,
                memo: memo ? memo : null,

            },
        },
        programId
    );
    // Append each of the signers into ix.keys
    ix.keys.push(...signers.map(signer => ({
        pubkey: signer,
        isSigner: true,
        isWritable: false,
    })));
    return ix;
}
