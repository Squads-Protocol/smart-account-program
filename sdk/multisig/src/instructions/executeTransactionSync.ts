import { PublicKey } from "@solana/web3.js";
import { createExecuteTransactionSyncInstruction, PROGRAM_ID } from "../generated";

export function executeTransactionSync({
    settingsPda,
    accountIndex,
    numSigners,
    instructions,
    instruction_accounts,
    programId = PROGRAM_ID,
}: {
    settingsPda: PublicKey;
    accountIndex: number;
    numSigners: number;
    instructions: Uint8Array;
    instruction_accounts: { pubkey: PublicKey; isWritable: boolean; isSigner: boolean }[];
    programId?: PublicKey;
}) {
    const ix = createExecuteTransactionSyncInstruction(
        {
            settings: settingsPda,
            anchorRemainingAccounts: instruction_accounts,
        },
        {
            args: {
                accountIndex,
                numSigners,
                instructions,
            },
        },
        programId
    );
    return ix;
}
