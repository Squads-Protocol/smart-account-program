import { PublicKey } from "@solana/web3.js";
import { createVaultTransactionSyncInstruction, PROGRAM_ID } from "../generated";

export function vaultTransactionSync({
    multisigPda,
    numSigners,
    vaultIndex,
    instructions,
    instruction_accounts,
    programId = PROGRAM_ID,
}: {
    multisigPda: PublicKey;
    numSigners: number;
    vaultIndex: number;
    instructions: Uint8Array;
    instruction_accounts: { pubkey: PublicKey; isWritable: boolean; isSigner: boolean }[];
    programId?: PublicKey;
}) {
    const ix = createVaultTransactionSyncInstruction(
        {
            multisig: multisigPda,
            anchorRemainingAccounts: instruction_accounts,
        },
        {
            args: {
                vaultIndex,
                numSigners,
                instructions,
            },
        },
        programId
    );
    return ix;
}
