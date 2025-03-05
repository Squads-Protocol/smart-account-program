import {
  ComputeBudgetProgram,
  Keypair,
  LAMPORTS_PER_SOL,
  PublicKey,
  SystemProgram,
  Transaction,
  TransactionMessage,
  VersionedTransaction,
} from "@solana/web3.js";
import * as smartAccount from "@sqds/smart-account";
import {
  CreateTransactionArgs,
  CreateTransactionBufferArgs,
  CreateTransactionBufferInstructionArgs,
  CreateTransactionFromBufferInstructionArgs,
} from "@sqds/smart-account/lib/generated";
import assert from "assert";
import { BN } from "bn.js";
import * as crypto from "crypto";
import {
  TestMembers,
  createAutonomousSmartAccountV2,
  createLocalhostConnection,
  createTestTransferInstruction,
  generateSmartAccountSigners,
  getTestProgramId,
  processBufferInChunks,
} from "../../utils";

const programId = getTestProgramId();
const connection = createLocalhostConnection();

describe("Examples / Custom Heap Usage", () => {
  let members: TestMembers;

  const createKey = Keypair.generate();

  let settingsPda: PublicKey;
  let vaultPda: PublicKey;

  // Set up a smart account with some transactions.
  before(async () => {
    members = await generateSmartAccountSigners(connection);

    // Create new autonomous smart account with rentCollector set to its default vault.
    await createAutonomousSmartAccountV2({
      connection,
      members,
      threshold: 1,
      timeLock: 0,
      rentCollector: vaultPda,
      programId,
    });

    // Airdrop some SOL to the vault
    let signature = await connection.requestAirdrop(
      vaultPda,
      10 * LAMPORTS_PER_SOL
    );
    await connection.confirmTransaction(signature);
  });

  // We expect this to succeed when requesting extra heap.
  it("execute large transaction (custom heap)", async () => {
    const transactionIndex = 1n;

    const testIx = await createTestTransferInstruction(vaultPda, vaultPda, 1);

    let instructions = [];

    // Add 64 transfer instructions to the message.
    for (let i = 0; i <= 59; i++) {
      instructions.push(testIx);
    }

    const testTransferMessage = new TransactionMessage({
      payerKey: vaultPda,
      recentBlockhash: (await connection.getLatestBlockhash()).blockhash,
      instructions: instructions,
    });

    //region Create & Upload Buffer
    // Serialize the message. Must be done with this util function
    const messageBuffer =
      smartAccount.utils.transactionMessageToMultisigTransactionMessageBytes({
        message: testTransferMessage,
        addressLookupTableAccounts: [],
        smartAccountPda: vaultPda,
      });

    const [transactionBuffer, _] = PublicKey.findProgramAddressSync(
      [
        Buffer.from("smart_account"),
        settingsPda.toBuffer(),
        Buffer.from("transaction_buffer"),
        new BN(Number(transactionIndex)).toBuffer("le", 8),
      ],
      programId
    );

    const messageHash = crypto
      .createHash("sha256")
      .update(messageBuffer)
      .digest();

    // Slice the message buffer into two parts.
    const firstSlice = messageBuffer.slice(0, 700);
    const bufferLength = messageBuffer.length;

    const ix = smartAccount.generated.createCreateTransactionBufferInstruction(
      {
        settings: settingsPda,
        transactionBuffer,
        creator: members.proposer.publicKey,
        rentPayer: members.proposer.publicKey,
        systemProgram: SystemProgram.programId,
      },
      {
        args: {
          accountIndex: 0,
          // Must be a SHA256 hash of the message buffer.
          finalBufferHash: Array.from(messageHash),
          finalBufferSize: bufferLength,
          buffer: firstSlice,
        } as CreateTransactionBufferArgs,
      } as CreateTransactionBufferInstructionArgs,
      programId
    );

    const message = new TransactionMessage({
      payerKey: members.proposer.publicKey,
      recentBlockhash: (await connection.getLatestBlockhash()).blockhash,
      instructions: [ix],
    }).compileToV0Message();

    const tx = new VersionedTransaction(message);
    tx.sign([members.proposer]);

    // Send first transaction.
    const signature1 = await connection.sendRawTransaction(tx.serialize(), {
      skipPreflight: true,
    });
    await connection.confirmTransaction(signature1);

    const transactionBufferAccount = await connection.getAccountInfo(
      transactionBuffer
    );

    const [txBufferDeser1] =
      smartAccount.generated.TransactionBuffer.fromAccountInfo(
        transactionBufferAccount!
      );

    // Check buffer account exists.
    assert.notEqual(transactionBufferAccount, null);
    assert.ok(transactionBufferAccount?.data.length! > 0);

    // First chunk uploaded. Check that length is as expected.
    assert.equal(txBufferDeser1.buffer.length, 700);

    // Process the buffer in <=700 byte chunks.
    await processBufferInChunks(
      members.proposer as Keypair,
      settingsPda,
      transactionBuffer,
      messageBuffer,
      connection,
      programId,
      700,
      700
    );

    // Get account info and deserialize to run checks.
    const transactionBufferInfo2 = await connection.getAccountInfo(
      transactionBuffer
    );
    const [txBufferDeser2] =
      smartAccount.generated.TransactionBuffer.fromAccountInfo(
        transactionBufferInfo2!
      );

    // Final chunk uploaded. Check that length is as expected.
    assert.equal(txBufferDeser2.buffer.length, messageBuffer.byteLength);

    // Derive transaction PDA.
    const [transactionPda] = smartAccount.getTransactionPda({
      settingsPda,
      transactionIndex: transactionIndex,
      programId,
    });
    //endregion

    //region Create Transaction From Buffer
    // Create final instruction.
    const thirdIx =
      smartAccount.generated.createCreateTransactionFromBufferInstruction(
        {
          transactionCreateItemSettings: settingsPda,
          transactionCreateItemTransaction: transactionPda,
          transactionCreateItemCreator: members.proposer.publicKey,
          transactionCreateItemRentPayer: members.proposer.publicKey,
          transactionCreateItemSystemProgram: SystemProgram.programId,
          transactionBuffer,
          creator: members.proposer.publicKey,
        },
        {
          args: {
            ephemeralSigners: 0,
            memo: null,
          } as CreateTransactionArgs,
        } as CreateTransactionFromBufferInstructionArgs,
        programId
      );

    // Request heap memory
    const prelimHeap = ComputeBudgetProgram.requestHeapFrame({
      bytes: 8 * 32 * 1024,
    });

    const prelimHeapCU = ComputeBudgetProgram.setComputeUnitLimit({
      units: 1_400_000,
    });

    const bufferConvertMessage = new TransactionMessage({
      payerKey: members.proposer.publicKey,
      recentBlockhash: (await connection.getLatestBlockhash()).blockhash,
      instructions: [prelimHeap, prelimHeapCU, thirdIx],
    }).compileToV0Message();

    const bufferConvertTx = new VersionedTransaction(bufferConvertMessage);

    bufferConvertTx.sign([members.proposer]);

    // Send buffer conversion transaction.
    const signature3 = await connection.sendRawTransaction(
      bufferConvertTx.serialize(),
      {
        skipPreflight: true,
      }
    );
    await connection.confirmTransaction(signature3);

    const transactionInfo =
      await smartAccount.accounts.Transaction.fromAccountAddress(
        connection,
        transactionPda
      );

    // Ensure final transaction has 60 instructions
    assert.equal(transactionInfo.message.instructions.length, 60);
    //endregion

    //region Create, Vote, and Execute
    // Create a proposal for the newly uploaded transaction.
    const signature4 = await smartAccount.rpc.createProposal({
      connection,
      feePayer: members.almighty,
      settingsPda,
      transactionIndex: 1n,
      creator: members.almighty,
      isDraft: false,
      programId,
    });
    await connection.confirmTransaction(signature4);

    // Approve the proposal.
    const signature5 = await smartAccount.rpc.approveProposal({
      connection,
      feePayer: members.almighty,
      settingsPda,
      transactionIndex: 1n,
      signer: members.almighty,
      programId,
    });
    await connection.confirmTransaction(signature5);

    // Execute the transaction.
    const executeIx = await smartAccount.instructions.executeTransaction({
      connection,
      settingsPda,
      transactionIndex: 1n,
      signer: members.almighty.publicKey,
      programId,
    });

    // Request heap for execution (it's very much needed here).
    const computeBudgetIx = ComputeBudgetProgram.requestHeapFrame({
      bytes: 8 * 32 * 1024,
    });

    const computeBudgetCUIx = ComputeBudgetProgram.setComputeUnitLimit({
      units: 1_400_000,
    });

    const executeTx = new Transaction().add(
      computeBudgetIx,
      computeBudgetCUIx,
      executeIx.instruction
    );
    const signature6 = await connection.sendTransaction(
      executeTx,
      [members.almighty],
      { skipPreflight: true }
    );

    await connection.confirmTransaction(signature6);

    const proposal = await smartAccount.getProposalPda({
      settingsPda,
      transactionIndex: 1n,
      programId,
    })[0];

    const proposalInfo =
      await smartAccount.accounts.Proposal.fromAccountAddress(
        connection,
        proposal
      );

    assert.equal(proposalInfo.status.__kind, "Executed");
    //endregion
  });
});
