import {
  Keypair,
  LAMPORTS_PER_SOL,
  PublicKey,
  SystemProgram,
  TransactionMessage,
  VersionedTransaction,
} from "@solana/web3.js";
import * as smartAccount from "@sqds/smart-account";
import {
  CreateTransactionBufferArgs,
  CreateTransactionBufferInstructionArgs,
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
const connection = createLocalhostConnection();

describe("Instructions / transaction_buffer_close", () => {
  let members: TestMembers;
  let settingsPda: PublicKey;
  let vaultPda: PublicKey;
  let transactionBuffer: PublicKey;

  before(async () => {
    members = await generateSmartAccountSigners(connection);
    const accountIndex = await getNextAccountIndex(connection, programId);
    settingsPda = (
      await createAutonomousSmartAccountV2({
        accountIndex,
        connection,
        members,
        threshold: 2,
        timeLock: 0,
        rentCollector: vaultPda,
        programId,
      })
    )[0];

    [vaultPda] = smartAccount.getSmartAccountPda({
      settingsPda,
      accountIndex: 0,
      programId,
    });

    let signature = await connection.requestAirdrop(
      vaultPda,
      10 * LAMPORTS_PER_SOL
    );
    await connection.confirmTransaction(signature);

    const bufferIndex = 0;
    const testIx = await createTestTransferInstruction(
      vaultPda,
      Keypair.generate().publicKey,
      0.1 * LAMPORTS_PER_SOL
    );

    const testTransferMessage = new TransactionMessage({
      payerKey: vaultPda,
      recentBlockhash: (await connection.getLatestBlockhash()).blockhash,
      instructions: [testIx],
    });

    const messageBuffer =
      smartAccount.utils.transactionMessageToMultisigTransactionMessageBytes({
        message: testTransferMessage,
        addressLookupTableAccounts: [],
        smartAccountPda: vaultPda,
      });

    [transactionBuffer] = PublicKey.findProgramAddressSync(
      [
        Buffer.from("smart_account"),
        settingsPda.toBuffer(),
        Buffer.from("transaction_buffer"),
        members.proposer.publicKey.toBuffer(),
        Uint8Array.from([bufferIndex]),
      ],
      programId
    );
    const messageHash = crypto
      .createHash("sha256")
      .update(messageBuffer)
      .digest();

    const createIx =
      smartAccount.generated.createCreateTransactionBufferInstruction(
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
            bufferIndex: Number(bufferIndex),
            finalBufferHash: Array.from(messageHash),
            finalBufferSize: messageBuffer.length,
            buffer: messageBuffer,
          } as CreateTransactionBufferArgs,
        } as CreateTransactionBufferInstructionArgs,
        programId
      );

    const createMessage = new TransactionMessage({
      payerKey: members.proposer.publicKey,
      recentBlockhash: (await connection.getLatestBlockhash()).blockhash,
      instructions: [createIx],
    }).compileToV0Message();

    const createTx = new VersionedTransaction(createMessage);
    createTx.sign([members.proposer]);

    const createSig = await connection.sendRawTransaction(
      createTx.serialize(),
      { skipPreflight: true }
    );
    await connection.confirmTransaction(createSig);
  });

  it("error: close buffer with non-creator signature", async () => {
    const closeIx =
      smartAccount.generated.createCloseTransactionBufferInstruction(
        {
          settings: settingsPda,
          transactionBuffer,
          creator: members.voter.publicKey,
        },
        programId
      );

    const closeMessage = new TransactionMessage({
      payerKey: members.voter.publicKey,
      recentBlockhash: (await connection.getLatestBlockhash()).blockhash,
      instructions: [closeIx],
    }).compileToV0Message();

    const closeTx = new VersionedTransaction(closeMessage);
    closeTx.sign([members.voter]);

    await assert.rejects(
      () =>
        connection
          .sendTransaction(closeTx)
          .catch(smartAccount.errors.translateAndThrowAnchorError),
      /(Unauthorized|ConstraintSeeds)/
    );
  });

  it("close buffer with creator signature", async () => {
    const closeIx =
      smartAccount.generated.createCloseTransactionBufferInstruction(
        {
          settings: settingsPda,
          transactionBuffer,
          creator: members.proposer.publicKey,
        },
        programId
      );

    const closeMessage = new TransactionMessage({
      payerKey: members.proposer.publicKey,
      recentBlockhash: (await connection.getLatestBlockhash()).blockhash,
      instructions: [closeIx],
    }).compileToV0Message();

    const closeTx = new VersionedTransaction(closeMessage);
    closeTx.sign([members.proposer]);

    const closeSig = await connection.sendTransaction(closeTx, {
      skipPreflight: true,
    });
    await connection.confirmTransaction(closeSig);
    const transactionBufferAccount = await connection.getAccountInfo(
      transactionBuffer
    );
    assert.equal(
      transactionBufferAccount,
      null,
      "Transaction buffer account should be closed"
    );
  });
});
