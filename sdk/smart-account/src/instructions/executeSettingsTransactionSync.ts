import { AccountMeta, PublicKey, SystemProgram } from "@solana/web3.js";
import { SettingsAction, createExecuteSettingsTransactionSyncInstruction, PROGRAM_ID } from "../generated";

export function executeSettingsTransactionSync({
    settingsPda,
    signers,
    actions,
    feePayer,
    memo,
    remainingAccounts,
    programId = PROGRAM_ID,
}: {
    settingsPda: PublicKey;
    signers: PublicKey[];
    actions: SettingsAction[];
    feePayer: PublicKey;
    remainingAccounts?: AccountMeta[];
    memo?: string;
    programId?: PublicKey;
}) {
    const ix = createExecuteSettingsTransactionSyncInstruction(
        {
            consensusAccount: settingsPda,
            rentPayer: feePayer ?? undefined,
            systemProgram: SystemProgram.programId,
            program: programId,
        },
        {
            args: {
                numSigners: signers.length,
                actions: actions,
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
    if (remainingAccounts) {
        ix.keys.push(...remainingAccounts);
    }
    return ix;
}
