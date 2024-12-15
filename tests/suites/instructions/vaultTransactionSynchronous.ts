import {
    createInitializeMint2Instruction,
    getMint,
    MINT_SIZE,
    TOKEN_PROGRAM_ID
} from "@solana/spl-token";
import {
    AddressLookupTableProgram,
    Keypair,
    LAMPORTS_PER_SOL,
    SystemProgram,
    TransactionMessage,
    VersionedTransaction
} from "@solana/web3.js";
import * as multisig from "@sqds/multisig";
import assert from "assert";
import {
    createAutonomousMultisigV2,
    createLocalhostConnection,
    generateMultisigMembers,
    getTestProgramId,
    TestMembers
} from "../../utils";


const programId = getTestProgramId();
const connection = createLocalhostConnection();
// const connection = new Connection("https://devnet.helius-rpc.com/?api-key=afd487c1-61b0-4aa2-9f3d-5e5de75fa2c5");
// const programId = new PublicKey("SMRTe6bnZAgJmXt9aJin7XgAzDn1XMHGNy95QATyzpk");
describe("Instructions / vault_transaction_synchronous", () => {
    let members: TestMembers;

    before(async () => {
        members = await generateMultisigMembers(connection);
    });

    it("execute synchronous transfer from vault", async () => {
        // Create multisig
        const createKey = Keypair.generate();
        const [multisigPda] = await createAutonomousMultisigV2({
            connection,
            createKey,
            members,
            threshold: 2,
            timeLock: 0,
            rentCollector: null,
            programId,
        });

        // Get vault PDA
        const [vaultPda] = multisig.getVaultPda({
            multisigPda,
            index: 0,
            programId,
        });

        // Fund vault
        const fundAmount = 2 * LAMPORTS_PER_SOL;
        await connection.requestAirdrop(vaultPda, fundAmount);
        await connection.confirmTransaction(
            await connection.requestAirdrop(vaultPda, fundAmount)
        );

        // Create transfer transaction
        const transferAmount = 1 * LAMPORTS_PER_SOL;
        const receiver = Keypair.generate();
        const transferInstruction = SystemProgram.transfer({
            fromPubkey: vaultPda,
            toPubkey: receiver.publicKey,
            lamports: transferAmount,
        });
        // Compile transaction for synchronous execution
        const { instructions, accounts: instruction_accounts } =
            multisig.utils.instructionsToSynchronousTransactionDetails({
                vaultPda,
                members: [members.proposer.publicKey, members.voter.publicKey, members.almighty.publicKey],
                transaction_instructions: [transferInstruction],
            });
        const synchronousTransactionInstruction = multisig.instructions.vaultTransactionSync({
            multisigPda,
            numSigners: 3,
            vaultIndex: 0,
            instructions,
            instruction_accounts,
            programId,
        });

        const message = new TransactionMessage({
            payerKey: members.almighty.publicKey,
            recentBlockhash: (await connection.getLatestBlockhash()).blockhash,
            instructions: [synchronousTransactionInstruction],
        }).compileToV0Message();

        const transaction = new VersionedTransaction(message);
        transaction.sign([members.proposer, members.voter, members.almighty]);
        // Execute synchronous transaction
        const signature = await connection.sendRawTransaction(transaction.serialize());
        await connection.confirmTransaction(signature);

        // Verify transfer
        const recipientBalance = await connection.getBalance(receiver.publicKey);
        assert.strictEqual(recipientBalance, transferAmount);
    });

    it("execute synchronous transfer from vault with lookup table", async () => {
        // Create multisig
        const createKey = Keypair.generate();
        const [multisigPda] = await createAutonomousMultisigV2({
            connection,
            createKey,
            members,
            threshold: 2,
            timeLock: 0,
            rentCollector: null,
            programId,
        });

        // Get vault PDA
        const [vaultPda] = multisig.getVaultPda({
            multisigPda,
            index: 0,
            programId,
        });

        // Fund vault
        const fundAmount = 2 * LAMPORTS_PER_SOL;
        await connection.requestAirdrop(vaultPda, fundAmount);
        await connection.confirmTransaction(
            await connection.requestAirdrop(vaultPda, fundAmount)
        );

        // Create transfer transaction
        const transferAmount = 1 * LAMPORTS_PER_SOL;
        const receiver = Keypair.generate();
        const transferInstruction = SystemProgram.transfer({
            fromPubkey: vaultPda,
            toPubkey: receiver.publicKey,
            lamports: transferAmount,
        });
        // Compile transaction for synchronous execution
        const { instructions, accounts: instruction_accounts } =
            multisig.utils.instructionsToSynchronousTransactionDetails({
                vaultPda,
                members: [members.proposer.publicKey, members.voter.publicKey, members.almighty.publicKey],
                transaction_instructions: [transferInstruction],
            });
        const synchronousTransactionInstruction = multisig.instructions.vaultTransactionSync({
            multisigPda,
            numSigners: 3,
            vaultIndex: 0,
            instructions,
            instruction_accounts,
            programId,
        });
        const [createLookupTableInstruction, lookupTablePublickey] = AddressLookupTableProgram.createLookupTable({
            authority: members.almighty.publicKey,
            payer: members.almighty.publicKey,
            recentSlot: (await connection.getLatestBlockhashAndContext()).context.slot - 5,
        });
        const extendLookupTableInstruction = AddressLookupTableProgram.extendLookupTable({
            addresses: [vaultPda, receiver.publicKey, programId],
            authority: members.almighty.publicKey,
            lookupTable: lookupTablePublickey,
            payer: members.almighty.publicKey,
        });
        const createTableMessage = new TransactionMessage({
            payerKey: members.almighty.publicKey,
            recentBlockhash: (await connection.getLatestBlockhash()).blockhash,
            instructions: [createLookupTableInstruction, extendLookupTableInstruction],
        }).compileToV0Message();
        const createTableTransaction = new VersionedTransaction(createTableMessage);
        createTableTransaction.sign([members.almighty]);
        const createTableSignature = await connection.sendRawTransaction(createTableTransaction.serialize(), { skipPreflight: true });
        await connection.confirmTransaction(createTableSignature);
        // Wait for lookup table to be fully activated
        await new Promise(resolve => setTimeout(resolve, 1000));


        const lookupTableAccount = (await connection.getAddressLookupTable(lookupTablePublickey)).value;
        assert(lookupTableAccount?.isActive, "Lookup table is not active");

        const message = new TransactionMessage({
            payerKey: members.almighty.publicKey,
            recentBlockhash: (await connection.getLatestBlockhash()).blockhash,
            instructions: [synchronousTransactionInstruction],
        }).compileToV0Message([lookupTableAccount]);

        const transaction = new VersionedTransaction(message);
        transaction.sign([members.proposer, members.voter, members.almighty]);
        // Execute synchronous transaction
        const signature = await connection.sendRawTransaction(transaction.serialize(), { skipPreflight: true });
        await connection.confirmTransaction(signature);

        // Verify transfer
        const recipientBalance = await connection.getBalance(receiver.publicKey);
        assert.strictEqual(recipientBalance, transferAmount);
    });


    it("can create a token mint", async () => {

        // Create multisig
        const createKey = Keypair.generate();
        const [multisigPda] = await createAutonomousMultisigV2({
            connection,
            createKey,
            members,
            threshold: 2,
            timeLock: 0,
            rentCollector: null,
            programId,
        });

        // Get vault PDA
        const [vaultPda] = multisig.getVaultPda({
            multisigPda,
            index: 0,
            programId,
        });

        // Fund vault
        const fundAmount = 2 * LAMPORTS_PER_SOL;
        await connection.requestAirdrop(vaultPda, fundAmount);
        await connection.confirmTransaction(
            await connection.requestAirdrop(vaultPda, fundAmount)
        );

        const decimals = 6;
        const mintAuthority = members.almighty.publicKey;
        const freezeAuthority = members.almighty.publicKey;

        // Create mint instruction
        const mintKeypair = Keypair.generate();
        const mintRent = await connection.getMinimumBalanceForRentExemption(MINT_SIZE);
        const createMintAccountInstruction = SystemProgram.createAccount({
            fromPubkey: vaultPda,
            newAccountPubkey: mintKeypair.publicKey,
            lamports: mintRent,
            space: MINT_SIZE,
            programId: TOKEN_PROGRAM_ID
        });

        const initializeMintInstruction = createInitializeMint2Instruction(
            mintKeypair.publicKey,
            decimals,
            mintAuthority,
            freezeAuthority
        );

        const instructions = [createMintAccountInstruction, initializeMintInstruction];

        // Convert instructions to synchronous transaction format
        const { instructions: instruction_bytes, accounts: instruction_accounts } = multisig.utils.instructionsToSynchronousTransactionDetails({
            vaultPda,
            members: [members.proposer.publicKey, members.voter.publicKey, members.almighty.publicKey],
            transaction_instructions: instructions,
        });

        // Create synchronous transaction instruction
        const synchronousTransactionInstruction = multisig.instructions.vaultTransactionSync({
            multisigPda,
            numSigners: 3,
            vaultIndex: 0,
            instructions: instruction_bytes,
            instruction_accounts,
            programId,
        });

        // Execute synchronous transaction
        const message = new TransactionMessage({
            payerKey: members.almighty.publicKey,
            recentBlockhash: (await connection.getLatestBlockhash()).blockhash,
            instructions: [synchronousTransactionInstruction],
        }).compileToV0Message([]);

        const transaction = new VersionedTransaction(message);
        transaction.sign([members.proposer, members.voter, members.almighty, mintKeypair]);

        const signature = await connection.sendRawTransaction(transaction.serialize(), { skipPreflight: true });
        await connection.confirmTransaction(signature);

        // Verify mint was created correctly
        const mintInfo = await getMint(connection, mintKeypair.publicKey);
        assert.strictEqual(mintInfo.decimals, decimals);
        assert.strictEqual(mintInfo.mintAuthority?.toBase58(), mintAuthority.toBase58());
        assert.strictEqual(mintInfo.freezeAuthority?.toBase58(), freezeAuthority.toBase58());
    });

    it("error: insufficient signers", async () => {
        // Create multisig
        const createKey = Keypair.generate();
        const [multisigPda] = await createAutonomousMultisigV2({
            connection,
            createKey,
            members,
            threshold: 2,
            timeLock: 0,
            rentCollector: null,
            programId,
        });

        // Get vault PDA
        const [vaultPda] = multisig.getVaultPda({
            multisigPda,
            index: 0,
            programId,
        });

        // Fund vault
        const fundAmount = 2 * LAMPORTS_PER_SOL;
        await connection.requestAirdrop(vaultPda, fundAmount);
        await connection.confirmTransaction(
            await connection.requestAirdrop(vaultPda, fundAmount)
        );

        // Create transfer transaction
        const transferAmount = 1 * LAMPORTS_PER_SOL;
        const receiver = Keypair.generate();
        const transferInstruction = SystemProgram.transfer({
            fromPubkey: vaultPda,
            toPubkey: receiver.publicKey,
            lamports: transferAmount,
        });
        // Compile transaction for synchronous execution
        const { instructions, accounts: instruction_accounts } =
            multisig.utils.instructionsToSynchronousTransactionDetails({
                vaultPda,
                members: [members.almighty.publicKey],
                transaction_instructions: [transferInstruction],
            });
        const synchronousTransactionInstruction = multisig.instructions.vaultTransactionSync({
            multisigPda,
            numSigners: 1,
            vaultIndex: 0,
            instructions,
            instruction_accounts,
            programId,
        });

        const message = new TransactionMessage({
            payerKey: members.almighty.publicKey,
            recentBlockhash: (await connection.getLatestBlockhash()).blockhash,
            instructions: [synchronousTransactionInstruction],
        }).compileToV0Message();

        const transaction = new VersionedTransaction(message);
        transaction.sign([members.almighty]);
        // Execute synchronous transaction
        assert.rejects(async () => {
            const signature = await connection.sendRawTransaction(transaction.serialize());
        }, /InvalidSignerCount/);

    });

    it("error: insufficient aggregate vote permissions", async () => {
        // Create multisig
        const createKey = Keypair.generate();
        const [multisigPda] = await createAutonomousMultisigV2({
            connection,
            createKey,
            members,
            threshold: 2,
            timeLock: 0,
            rentCollector: null,
            programId,
        });

        // Get vault PDA
        const [vaultPda] = multisig.getVaultPda({
            multisigPda,
            index: 0,
            programId,
        });

        // Fund vault
        const fundAmount = 2 * LAMPORTS_PER_SOL;
        await connection.requestAirdrop(vaultPda, fundAmount);
        await connection.confirmTransaction(
            await connection.requestAirdrop(vaultPda, fundAmount)
        );

        // Create transfer transaction
        const transferAmount = 1 * LAMPORTS_PER_SOL;
        const receiver = Keypair.generate();
        const transferInstruction = SystemProgram.transfer({
            fromPubkey: vaultPda,
            toPubkey: receiver.publicKey,
            lamports: transferAmount,
        });
        // Compile transaction for synchronous execution
        const { instructions, accounts: instruction_accounts } =
            multisig.utils.instructionsToSynchronousTransactionDetails({
                vaultPda,
                members: [members.proposer.publicKey, members.voter.publicKey, members.executor.publicKey],
                transaction_instructions: [transferInstruction],
            });
        const synchronousTransactionInstruction = multisig.instructions.vaultTransactionSync({
            multisigPda,
            numSigners: 3,
            vaultIndex: 0,
            instructions,
            instruction_accounts,
            programId,
        });

        const message = new TransactionMessage({
            payerKey: members.almighty.publicKey,
            recentBlockhash: (await connection.getLatestBlockhash()).blockhash,
            instructions: [synchronousTransactionInstruction],
        }).compileToV0Message();

        const transaction = new VersionedTransaction(message);
        transaction.sign([members.proposer, members.voter, members.executor]);
        // Execute synchronous transaction
        assert.rejects(async () => {
            const signature = await connection.sendRawTransaction(transaction.serialize());
        }, /InsufficientVotePermissions/);

    });

    it("error: insufficient aggregate permissions", async () => {
        // Create multisig
        const createKey = Keypair.generate();
        const [multisigPda] = await createAutonomousMultisigV2({
            connection,
            createKey,
            members,
            threshold: 2,
            timeLock: 0,
            rentCollector: null,
            programId,
        });

        // Get vault PDA
        const [vaultPda] = multisig.getVaultPda({
            multisigPda,
            index: 0,
            programId,
        });

        // Fund vault
        const fundAmount = 2 * LAMPORTS_PER_SOL;
        await connection.requestAirdrop(vaultPda, fundAmount);
        await connection.confirmTransaction(
            await connection.requestAirdrop(vaultPda, fundAmount)
        );

        // Create transfer transaction
        const transferAmount = 1 * LAMPORTS_PER_SOL;
        const receiver = Keypair.generate();
        const transferInstruction = SystemProgram.transfer({
            fromPubkey: vaultPda,
            toPubkey: receiver.publicKey,
            lamports: transferAmount,
        });
        // Compile transaction for synchronous execution
        const { instructions, accounts: instruction_accounts } =
            multisig.utils.instructionsToSynchronousTransactionDetails({
                vaultPda,
                members: [members.proposer.publicKey, members.voter.publicKey],
                transaction_instructions: [transferInstruction],
            });
        const synchronousTransactionInstruction = multisig.instructions.vaultTransactionSync({
            multisigPda,
            numSigners: 2,
            vaultIndex: 0,
            instructions,
            instruction_accounts,
            programId,
        });

        const message = new TransactionMessage({
            payerKey: members.almighty.publicKey,
            recentBlockhash: (await connection.getLatestBlockhash()).blockhash,
            instructions: [synchronousTransactionInstruction],
        }).compileToV0Message();

        const transaction = new VersionedTransaction(message);
        transaction.sign([members.proposer, members.voter]);
        // Execute synchronous transaction
        assert.rejects(async () => {
            const signature = await connection.sendRawTransaction(transaction.serialize());
        }, /InsufficientAggregatePermissions/);

    });

    it("error: not allowed with time lock", async () => {
        // Create multisig
        const createKey = Keypair.generate();
        const [multisigPda] = await createAutonomousMultisigV2({
            connection,
            createKey,
            members,
            threshold: 2,
            // Adding a 20s time lock
            timeLock: 20,
            rentCollector: null,
            programId,
        });

        // Get vault PDA
        const [vaultPda] = multisig.getVaultPda({
            multisigPda,
            index: 0,
            programId,
        });

        // Fund vault
        const fundAmount = 2 * LAMPORTS_PER_SOL;
        await connection.requestAirdrop(vaultPda, fundAmount);
        await connection.confirmTransaction(
            await connection.requestAirdrop(vaultPda, fundAmount)
        );

        // Create transfer transaction
        const transferAmount = 1 * LAMPORTS_PER_SOL;
        const receiver = Keypair.generate();
        const transferInstruction = SystemProgram.transfer({
            fromPubkey: vaultPda,
            toPubkey: receiver.publicKey,
            lamports: transferAmount,
        });
        // Compile transaction for synchronous execution
        const { instructions, accounts: instruction_accounts } =
            multisig.utils.instructionsToSynchronousTransactionDetails({
                vaultPda,
                members: [members.proposer.publicKey, members.voter.publicKey],
                transaction_instructions: [transferInstruction],
            });
        const synchronousTransactionInstruction = multisig.instructions.vaultTransactionSync({
            multisigPda,
            numSigners: 2,
            vaultIndex: 0,
            instructions,
            instruction_accounts,
            programId,
        });

        const message = new TransactionMessage({
            payerKey: members.almighty.publicKey,
            recentBlockhash: (await connection.getLatestBlockhash()).blockhash,
            instructions: [synchronousTransactionInstruction],
        }).compileToV0Message();

        const transaction = new VersionedTransaction(message);
        transaction.sign([members.proposer, members.voter]);
        // Execute synchronous transaction
        assert.rejects(async () => {
            const signature = await connection.sendRawTransaction(transaction.serialize());
        }, /TimeLockNotZero/);

    });

    it("error: missing a signature", async () => {
        // Create multisig
        const createKey = Keypair.generate();
        const [multisigPda] = await createAutonomousMultisigV2({
            connection,
            createKey,
            members,
            threshold: 2,
            // Adding a 20s time lock
            timeLock: 20,
            rentCollector: null,
            programId,
        });

        // Get vault PDA
        const [vaultPda] = multisig.getVaultPda({
            multisigPda,
            index: 0,
            programId,
        });

        // Fund vault
        const fundAmount = 2 * LAMPORTS_PER_SOL;
        await connection.requestAirdrop(vaultPda, fundAmount);
        await connection.confirmTransaction(
            await connection.requestAirdrop(vaultPda, fundAmount)
        );

        // Create transfer transaction
        const transferAmount = 1 * LAMPORTS_PER_SOL;
        const receiver = Keypair.generate();
        // Having the executor here as the sender puts it as the 3rd account
        const transferInstruction = SystemProgram.transfer({
            fromPubkey: members.executor.publicKey,
            toPubkey: receiver.publicKey,
            lamports: transferAmount,
        });
        // Compile transaction for synchronous execution
        const { instructions, accounts: instruction_accounts } =
            multisig.utils.instructionsToSynchronousTransactionDetails({
                vaultPda,
                members: [members.proposer.publicKey, members.voter.publicKey],
                transaction_instructions: [transferInstruction],
            });
        const synchronousTransactionInstruction = multisig.instructions.vaultTransactionSync({
            multisigPda,
            // Adding a non existent 3rd signer
            numSigners: 3,
            vaultIndex: 0,
            instructions,
            instruction_accounts,
            programId,
        });

        const message = new TransactionMessage({
            payerKey: members.almighty.publicKey,
            recentBlockhash: (await connection.getLatestBlockhash()).blockhash,
            instructions: [synchronousTransactionInstruction],
        }).compileToV0Message();

        const transaction = new VersionedTransaction(message);
        transaction.sign([members.proposer, members.voter]);
        // Execute synchronous transaction
        assert.rejects(async () => {
            const signature = await connection.sendRawTransaction(transaction.serialize());
        }, /MissingSignature/);

    });

    it("error: not a member", async () => {
        // Create multisig
        const createKey = Keypair.generate();
        const [multisigPda] = await createAutonomousMultisigV2({
            connection,
            createKey,
            members,
            threshold: 2,
            // Adding a 20s time lock
            timeLock: 20,
            rentCollector: null,
            programId,
        });

        // Get vault PDA
        const [vaultPda] = multisig.getVaultPda({
            multisigPda,
            index: 0,
            programId,
        });

        // Fund vault
        const fundAmount = 2 * LAMPORTS_PER_SOL;
        await connection.requestAirdrop(vaultPda, fundAmount);
        await connection.confirmTransaction(
            await connection.requestAirdrop(vaultPda, fundAmount)
        );

        // Create transfer transaction
        const transferAmount = 1 * LAMPORTS_PER_SOL;
        const receiver = Keypair.generate();
        // Having a non member here as the sender puts it as the 3rd account
        // when compiling the accounts thereby letting us test the not a member
        // error, as long as theyre a signer
        const randomNonMember = Keypair.generate();
        const transferInstruction = SystemProgram.transfer({
            fromPubkey: randomNonMember.publicKey,
            toPubkey: receiver.publicKey,
            lamports: transferAmount,
        });
        // Compile transaction for synchronous execution
        const { instructions, accounts: instruction_accounts } =
            multisig.utils.instructionsToSynchronousTransactionDetails({
                vaultPda,
                members: [members.proposer.publicKey, members.voter.publicKey],
                transaction_instructions: [transferInstruction],
            });
        const synchronousTransactionInstruction = multisig.instructions.vaultTransactionSync({
            multisigPda,
            // Adding the non member as a signer
            numSigners: 3,
            vaultIndex: 0,
            instructions,
            instruction_accounts,
            programId,
        });

        const message = new TransactionMessage({
            payerKey: members.almighty.publicKey,
            recentBlockhash: (await connection.getLatestBlockhash()).blockhash,
            instructions: [synchronousTransactionInstruction],
        }).compileToV0Message();

        const transaction = new VersionedTransaction(message);
        transaction.sign([members.proposer, members.voter,]);
        // Execute synchronous transaction
        assert.rejects(async () => {
            const signature = await connection.sendRawTransaction(transaction.serialize());
        }, /NotASigner/);
    });

    it("error: duplicate signer", async () => {
        // Create multisig
        const createKey = Keypair.generate();
        const [multisigPda] = await createAutonomousMultisigV2({
            connection,
            createKey,
            members,
            threshold: 2,
            // Adding a 20s time lock
            timeLock: 20,
            rentCollector: null,
            programId,
        });

        // Get vault PDA
        const [vaultPda] = multisig.getVaultPda({
            multisigPda,
            index: 0,
            programId,
        });

        // Fund vault
        const fundAmount = 2 * LAMPORTS_PER_SOL;
        await connection.requestAirdrop(vaultPda, fundAmount);
        await connection.confirmTransaction(
            await connection.requestAirdrop(vaultPda, fundAmount)
        );

        // Create transfer transaction
        const transferAmount = 1 * LAMPORTS_PER_SOL;
        const receiver = Keypair.generate();
        // Having the proposer here as the sender puts it as the 3rd account
        // when compiling the accounts thereby letting us test the duplicate signer
        // error
        const transferInstruction = SystemProgram.transfer({
            fromPubkey: members.proposer.publicKey,
            toPubkey: receiver.publicKey,
            lamports: transferAmount,
        });
        // Compile transaction for synchronous execution
        const { instructions, accounts: instruction_accounts } =
            multisig.utils.instructionsToSynchronousTransactionDetails({
                vaultPda,
                members: [members.proposer.publicKey, members.voter.publicKey],
                transaction_instructions: [transferInstruction],
            });
        const synchronousTransactionInstruction = multisig.instructions.vaultTransactionSync({
            multisigPda,
            // Adding the non member as a signer
            numSigners: 3,
            vaultIndex: 0,
            instructions,
            instruction_accounts,
            programId,
        });

        const message = new TransactionMessage({
            payerKey: members.almighty.publicKey,
            recentBlockhash: (await connection.getLatestBlockhash()).blockhash,
            instructions: [synchronousTransactionInstruction],
        }).compileToV0Message();

        const transaction = new VersionedTransaction(message);
        transaction.sign([members.proposer, members.voter,]);
        // Execute synchronous transaction
        assert.rejects(async () => {
            const signature = await connection.sendRawTransaction(transaction.serialize());
        }, /DuplicateSigner/);
    });
});
