import {
  AccountMeta,
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
  ExtendTransactionBufferArgs,
  ExtendTransactionBufferInstructionArgs,
} from "@sqds/smart-account/lib/generated";
import assert from "assert";
import * as crypto from "crypto";
import {
  TestMembers,
  createAutonomousSmartAccountV2,
  createLocalhostConnection,
  createTestTransferInstruction,
  generateSmartAccountSigners,
  getNextAccountIndex,
  getTestProgramId,
} from "../../utils";

const programId = getTestProgramId();

describe("Examples / Transaction Buffers", () => {
  const connection = createLocalhostConnection();

  let members: TestMembers;

  let settingsPda: PublicKey;
  let vaultPda: PublicKey;

  before(async () => {
    members = await generateSmartAccountSigners(connection);
    let accountIndex = await getNextAccountIndex(connection, programId);
    settingsPda = (
      await createAutonomousSmartAccountV2({
        connection,
        members: members,
        accountIndex,
        threshold: 1,
        timeLock: 0,
        programId,
        rentCollector: vaultPda,
      })
    )[0];
    vaultPda = smartAccount.getSmartAccountPda({
      settingsPda,
      accountIndex: 0,
      programId,
    })[0];
    // Airdrop some SOL to the vault
    let signature = await connection.requestAirdrop(
      vaultPda,
      10 * LAMPORTS_PER_SOL
    );
    await connection.confirmTransaction(signature);
  });

  it("set buffer, extend, and create", async () => {
    const transactionIndex = 1n;
    const bufferIndex = 0;

    const testIx = createTestTransferInstruction(vaultPda, vaultPda, 1);

    let instructions = [];

    // Add 32 transfer instructions to the message.
    for (let i = 0; i <= 22; i++) {
      instructions.push(testIx);
    }

    const testTransferMessage = new TransactionMessage({
      payerKey: vaultPda,
      recentBlockhash: (await connection.getLatestBlockhash()).blockhash,
      instructions: instructions,
    });

    // Serialize the message. Must be done with this util function
    const messageBuffer =
      smartAccount.utils.transactionMessageToMultisigTransactionMessageBytes({
        message: testTransferMessage,
        addressLookupTableAccounts: [],
        smartAccountPda: vaultPda,
      });

    const [transactionBuffer, _] = await PublicKey.findProgramAddressSync(
      [
        Buffer.from("smart_account"),
        settingsPda.toBuffer(),
        Buffer.from("transaction_buffer"),
        members.almighty.publicKey.toBuffer(),
        Buffer.from([bufferIndex]),
      ],
      programId
    );

    const messageHash = crypto
      .createHash("sha256")
      .update(messageBuffer)
      .digest();

    // Slice the message buffer into two parts.
    const firstSlice = messageBuffer.slice(0, 400);

    const ix = smartAccount.generated.createCreateTransactionBufferInstruction(
      {
        settings: settingsPda,
        transactionBuffer,
        creator: members.almighty.publicKey,
        rentPayer: members.almighty.publicKey,
      },
      {
        args: {
          accountIndex: 0,
          bufferIndex: 0,
          // Must be a SHA256 hash of the message buffer.
          finalBufferHash: Array.from(messageHash),
          finalBufferSize: messageBuffer.length,
          buffer: firstSlice,
        } as CreateTransactionBufferArgs,
      } as CreateTransactionBufferInstructionArgs,
      programId
    );

    const message = new TransactionMessage({
      payerKey: members.almighty.publicKey,
      recentBlockhash: (await connection.getLatestBlockhash()).blockhash,
      instructions: [ix],
    }).compileToV0Message();

    const tx = new VersionedTransaction(message);

    tx.sign([members.almighty]);

    // Send first transaction.
    const signature = await connection.sendTransaction(tx, {
      skipPreflight: true,
    });
    await connection.confirmTransaction(signature);

    const transactionBufferAccount = await connection.getAccountInfo(
      transactionBuffer
    );

    // Check buffer account exists.
    assert.notEqual(transactionBufferAccount, null);
    assert.ok(transactionBufferAccount?.data.length! > 0);

    // Need to add some deserialization to check if it actually worked.
    const transactionBufferInfo1 = await connection.getAccountInfo(
      transactionBuffer
    );
    const [txBufferDeser1] =
      await smartAccount.generated.TransactionBuffer.fromAccountInfo(
        transactionBufferInfo1!
      );

    // First chunk uploaded. Check that length is as expected.
    assert.equal(txBufferDeser1.buffer.length, 400);

    const secondSlice = messageBuffer.slice(400, messageBuffer.byteLength);

    // Extned the buffer.
    const secondIx =
      smartAccount.generated.createExtendTransactionBufferInstruction(
        {
          settings: settingsPda,
          transactionBuffer,
          creator: members.almighty.publicKey,
        },
        {
          args: {
            buffer: secondSlice,
          } as ExtendTransactionBufferArgs,
        } as ExtendTransactionBufferInstructionArgs,
        programId
      );

    const secondMessage = new TransactionMessage({
      payerKey: members.almighty.publicKey,
      recentBlockhash: (await connection.getLatestBlockhash()).blockhash,
      instructions: [secondIx],
    }).compileToV0Message();

    const secondTx = new VersionedTransaction(secondMessage);

    secondTx.sign([members.almighty]);

    // Send second transaction to extend.
    const secondSignature = await connection.sendTransaction(secondTx, {
      skipPreflight: true,
    });

    await connection.confirmTransaction(secondSignature);

    // Need to add some deserialization to check if it actually worked.
    const transactionBufferInfo2 = await connection.getAccountInfo(
      transactionBuffer
    );
    const [txBufferDeser2] =
      await smartAccount.generated.TransactionBuffer.fromAccountInfo(
        transactionBufferInfo2!
      );

    // Full buffer uploaded. Check that length is as expected.
    assert.equal(txBufferDeser2.buffer.length, messageBuffer.byteLength);

    // Derive transaction PDA.
    const [transactionPda] = smartAccount.getTransactionPda({
      settingsPda,
      transactionIndex,
      programId,
    });

    const transactionBufferMeta: AccountMeta = {
      pubkey: transactionBuffer,
      isWritable: true,
      isSigner: false,
    };
    // Create final instruction.
    const thirdIx =
      smartAccount.generated.createCreateTransactionFromBufferInstruction(
        {
          transactionCreateItemSettings: settingsPda,
          transactionCreateItemTransaction: transactionPda,
          transactionCreateItemCreator: members.almighty.publicKey,
          transactionCreateItemRentPayer: members.almighty.publicKey,
          transactionCreateItemSystemProgram: SystemProgram.programId,
          transactionBuffer: transactionBuffer,
          creator: members.almighty.publicKey,
        },
        {
          args: {
            accountIndex: 0,
            transactionMessage: new Uint8Array(6).fill(0),
            ephemeralSigners: 0,
            memo: null,
          } as CreateTransactionArgs,
        } as CreateTransactionFromBufferInstructionArgs,
        programId
      );

    // Add third instruction to the message.
    const thirdMessage = new TransactionMessage({
      payerKey: members.almighty.publicKey,
      recentBlockhash: (await connection.getLatestBlockhash()).blockhash,
      instructions: [thirdIx],
    }).compileToV0Message();

    const thirdTx = new VersionedTransaction(thirdMessage);

    thirdTx.sign([members.almighty]);

    // Send final transaction.
    const thirdSignature = await connection.sendTransaction(thirdTx, {
      skipPreflight: true,
    });

    await connection.confirmTransaction(thirdSignature);

    const transactionInfo =
      await smartAccount.accounts.Transaction.fromAccountAddress(
        connection,
        transactionPda
      );

    // Ensure final transaction has 23 instructions
    assert.equal(transactionInfo.message.instructions.length, 23);
  });

  it("create proposal, approve, execute from buffer derived transaction", async () => {
    const transactionIndex = 1n;

    // Derive transaction PDA.
    const [transactionPda] = smartAccount.getTransactionPda({
      settingsPda,
      transactionIndex,
      programId,
    });

    const transactionInfo =
      await smartAccount.accounts.Transaction.fromAccountAddress(
        connection,
        transactionPda
      );

    // Check that we're dealing with the same account from last test.
    assert.equal(transactionInfo.message.instructions.length, 23);

    const [proposalPda] = smartAccount.getProposalPda({
      settingsPda,
      transactionIndex,
      programId,
    });

    const signature = await smartAccount.rpc.createProposal({
      connection,
      feePayer: members.almighty,
      settingsPda,
      transactionIndex,
      creator: members.almighty,
      isDraft: false,
      programId,
    });
    await connection.confirmTransaction(signature);

    const signature3 = await smartAccount.rpc.approveProposal({
      connection,
      feePayer: members.almighty,
      settingsPda,
      transactionIndex,
      signer: members.almighty,
      programId,
    });
    await connection.confirmTransaction(signature3);

    // Fetch the proposal account.
    let proposalAccount1 =
      await smartAccount.accounts.Proposal.fromAccountAddress(
        connection,
        proposalPda
      );

    const ix = await smartAccount.instructions.executeTransaction({
      connection,
      settingsPda,
      transactionIndex,
      signer: members.almighty.publicKey,
      programId,
    });

    const tx = new Transaction().add(ix.instruction);
    const signature4 = await connection.sendTransaction(
      tx,
      [members.almighty],
      { skipPreflight: true }
    );

    await connection.confirmTransaction(signature4);

    // Fetch the proposal account.
    let proposalAccount =
      await smartAccount.accounts.Proposal.fromAccountAddress(
        connection,
        proposalPda
      );

    // Check status.
    assert.equal(proposalAccount.status.__kind, "Executed");
  });
});
