import { PublicKey } from "@solana/web3.js";
import { ConfigAction, createConfigTransactionSyncInstruction, PROGRAM_ID } from "../generated";

export function configTransactionSync({
    multisigPda,
    numSigners,
    configActions,
    memo,
    programId = PROGRAM_ID,
}: {
    multisigPda: PublicKey;
    numSigners: number;
    configActions: ConfigAction[];
    memo?: string,
    programId?: PublicKey;
}) {
    const ix = createConfigTransactionSyncInstruction(
        {
            multisig: multisigPda,
        },
        {
            args: {
                numSigners,
                actions: configActions,
                memo: memo ? memo : null,
            },
        },
        programId
    );
    return ix;
}
