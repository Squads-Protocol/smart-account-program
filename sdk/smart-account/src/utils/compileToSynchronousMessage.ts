import {
    AccountMeta,
    PublicKey,
    TransactionInstruction
} from "@solana/web3.js";
import { CompiledKeys } from "./compiled-keys";


export function compileToSynchronousMessageAndAccounts({
    vaultPda,
    members,
    instructions,
}: {
    vaultPda: PublicKey,
    members: PublicKey[],
    instructions: TransactionInstruction[];
}): {
    instructions: Uint8Array;
    accounts: AccountMeta[];
} {

    // Compile instructions to get account mapping
    const compiledKeys = CompiledKeys.compileWithoutPayer(
        instructions
    );

    // Get message components
    const [header, staticAccountKeys] = compiledKeys.getMessageComponents();

    // Mark the vault as non-signer if it exists
    // Create remaining accounts array starting with vault
    const remainingAccounts: AccountMeta[] = [
        ...members.map((member, index) => ({
            pubkey: member,
            isSigner: true,
            isWritable: false,
        })),
    ];

    // Add all other accounts
    staticAccountKeys.forEach((key, index) => {
        remainingAccounts.push({
            pubkey: key,
            isSigner: index < header.numRequiredSignatures,
            isWritable: index < (header.numRequiredSignatures - header.numReadonlySignedAccounts) ||
                (index >= header.numRequiredSignatures &&
                    index < (staticAccountKeys.length - header.numReadonlyUnsignedAccounts))
        });
    });

    // Mark the vault as non-signer if it exists
    if (remainingAccounts.find(acc => acc.pubkey.equals(vaultPda))) {
        remainingAccounts.find(acc => acc.pubkey.equals(vaultPda))!.isSigner = false;
    }
    // Compile instructions with updated indices
    let args_buffer = Buffer.alloc(0);

    // Insert instruction length as u8 as first byte
    args_buffer = Buffer.concat([
        Buffer.from([instructions.length])
    ]);

    // Serialize each instruction
    instructions.forEach(ix => {
        const accounts = ix.keys.map(key => {
            const index = remainingAccounts.findIndex(acc => acc.pubkey.equals(key.pubkey));
            if (index === -1) {
                throw new Error(`Account ${key.pubkey.toBase58()} not found in remaining accounts`);
            }
            return index;
        });

        const programIdIndex = remainingAccounts.findIndex(id => id.pubkey.equals(ix.programId));
        if (programIdIndex === -1) {
            throw new Error(`ProgramId ${ix.programId.toBase58()} not found in remaining accounts`);
        }

        const serialized_ix = serializeCompiledInstruction({
            programIdIndex,
            accountIndexes: accounts,
            data: ix.data,
        });

        // Concatenate the serialized instruction to the buffer
        args_buffer = Buffer.concat([args_buffer, serialized_ix]);
    });

    return {
        instructions: args_buffer,
        accounts: remainingAccounts,
    };
}


function serializeCompiledInstruction(ix: {
    programIdIndex: number,
    accountIndexes: number[],
    data: Buffer | Uint8Array,
}) {
    // Create a buffer to hold the serialized instruction
    let buffer = Buffer.alloc(0);

    // Add program id index (u8)
    buffer = Buffer.concat([buffer, Buffer.from([ix.programIdIndex])]);

    // SmallVec<u8, u8> for account indexes
    buffer = Buffer.concat([
        buffer,
        // Length prefix as u8
        Buffer.from([ix.accountIndexes.length]),
        // The account indexes
        Buffer.from(ix.accountIndexes)
    ]);

    // SmallVec<u16, u8> for data
    buffer = Buffer.concat([
        buffer,
        // Length prefix as u16 (little endian)
        Buffer.from(new Uint16Array([ix.data.length]).buffer),
        // The actual data bytes
        ix.data
    ]);

    return Uint8Array.from(buffer);
}