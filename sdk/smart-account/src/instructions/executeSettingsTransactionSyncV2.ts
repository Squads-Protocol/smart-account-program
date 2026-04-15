import { AccountMeta, PublicKey, SystemProgram } from "@solana/web3.js";
import { SettingsAction, createExecuteSettingsTransactionSyncV2Instruction, PROGRAM_ID } from "../generated";
import { patchInstructionEvd } from "../utils";

export function executeSettingsTransactionSyncV2({
    settingsPda,
    signers,
    actions,
    feePayer,
    memo,
    extraVerificationData,
    externalSigners,
    remainingAccounts,
    programId = PROGRAM_ID,
}: {
    settingsPda: PublicKey;
    signers: PublicKey[];
    actions: SettingsAction[];
    feePayer: PublicKey;
    remainingAccounts?: AccountMeta[];
    memo?: string;
    extraVerificationData?: Uint8Array | null;
    /** Set of signer pubkeys (or indices) that are external (precompile-verified, not native tx signers). */
    externalSigners?: PublicKey[];
    programId?: PublicKey;
}) {
    const ix = createExecuteSettingsTransactionSyncV2Instruction(
        {
            consensusAccount: settingsPda,
            rentPayer: feePayer,
            systemProgram: SystemProgram.programId,
            program: programId,
        },
        {
            args: {
                numSigners: signers.length,
                actions: actions,
                memo: memo ? memo : null,
            },
            extraVerificationData: null,
        },
        programId
    );
    if (extraVerificationData && extraVerificationData.length > 0) {
        patchInstructionEvd(ix, extraVerificationData);
    }
    const externalSet = new Set(externalSigners?.map(k => k.toBase58()) ?? []);
    ix.keys.push(...signers.map(signer => ({
        pubkey: signer,
        isSigner: !externalSet.has(signer.toBase58()),
        isWritable: false,
    })));
    if (remainingAccounts) {
        ix.keys.push(...remainingAccounts);
    }
    return ix;
}
