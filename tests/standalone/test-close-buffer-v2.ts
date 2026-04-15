/**
 * Standalone test: V2 close_transaction_buffer
 * Creates a buffer, then closes it via the V2 instruction.
 *
 * Run: npx mocha --node-option require=ts-node/register --extension ts -t 60000 tests/standalone/test-close-buffer-v2.ts
 */

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
  generateSmartAccountSigners,
  getNextAccountIndex,
  getTestProgramId,
  getTestProgramConfigInitializer,
  getTestProgramConfigAuthority,
  getTestProgramTreasury,
} from "../utils";

const programId = getTestProgramId();
const connection = createLocalhostConnection();

describe("V2 close_transaction_buffer", () => {
  let members: TestMembers;
  let settingsPda: PublicKey;
  let vaultPda: PublicKey;

  before(async () => {
    // Init ProgramConfig (ignore if already exists)
    const programConfigInitializer = getTestProgramConfigInitializer();
    const programConfigAuthority = getTestProgramConfigAuthority();
    const programTreasury = getTestProgramTreasury();
    const programConfigPda = smartAccount.getProgramConfigPda({ programId })[0];
    try {
      const airdropSig = await connection.requestAirdrop(programConfigInitializer.publicKey, 2 * LAMPORTS_PER_SOL);
      await connection.confirmTransaction(airdropSig);
      const initIx = smartAccount.generated.createInitializeProgramConfigInstruction(
        { programConfig: programConfigPda, initializer: programConfigInitializer.publicKey },
        { args: { authority: programConfigAuthority.publicKey, treasury: programTreasury, smartAccountCreationFee: 0 } },
        programId
      );
      const msg = new TransactionMessage({ payerKey: programConfigInitializer.publicKey, recentBlockhash: (await connection.getLatestBlockhash()).blockhash, instructions: [initIx] }).compileToV0Message();
      const tx = new VersionedTransaction(msg);
      tx.sign([programConfigInitializer]);
      const sig = await connection.sendRawTransaction(tx.serialize(), { skipPreflight: true });
      await connection.confirmTransaction(sig);
    } catch (_) {}

    members = await generateSmartAccountSigners(connection);
    const accountIndex = await getNextAccountIndex(connection, programId);

    [settingsPda] = await createAutonomousSmartAccountV2({
      connection,
      members,
      threshold: 1,
      timeLock: 0,
      rentCollector: null,
      programId,
      accountIndex,
    });

    vaultPda = smartAccount.getSmartAccountPda({
      settingsPda,
      accountIndex: 0,
      programId,
    })[0];

    const sig = await connection.requestAirdrop(
      vaultPda,
      5 * LAMPORTS_PER_SOL
    );
    await connection.confirmTransaction(sig);
  });

  it("create then close buffer via V2", async () => {
    const creator = members.almighty;

    // Build dummy message
    const testIx = SystemProgram.transfer({
      fromPubkey: vaultPda,
      toPubkey: Keypair.generate().publicKey,
      lamports: LAMPORTS_PER_SOL,
    });

    const testMessage = new TransactionMessage({
      payerKey: vaultPda,
      recentBlockhash: (await connection.getLatestBlockhash()).blockhash,
      instructions: [testIx],
    });

    const messageBuffer =
      smartAccount.utils.transactionMessageToMultisigTransactionMessageBytes({
        message: testMessage,
        addressLookupTableAccounts: [],
        smartAccountPda: vaultPda,
      });

    const messageHash = crypto
      .createHash("sha256")
      .update(messageBuffer.transactionMessageBytes)
      .digest();

    const bufferIndex = 0;
    const [transactionBuffer] = PublicKey.findProgramAddressSync(
      [
        Buffer.from("smart_account"),
        settingsPda.toBuffer(),
        Buffer.from("transaction_buffer"),
        creator.publicKey.toBuffer(),
        Buffer.from([bufferIndex]),
      ],
      programId
    );

    // Create buffer
    const createIx =
      smartAccount.generated.createCreateTransactionBufferInstruction(
        {
          consensusAccount: settingsPda,
          transactionBuffer,
          creator: creator.publicKey,
          rentPayer: creator.publicKey,
          systemProgram: SystemProgram.programId,
        },
        {
          args: {
            bufferIndex,
            accountIndex: 0,
            finalBufferHash: Array.from(messageHash),
            finalBufferSize: messageBuffer.transactionMessageBytes.byteLength,
            buffer: messageBuffer.transactionMessageBytes.slice(0, 750),
          } as CreateTransactionBufferArgs,
        } as CreateTransactionBufferInstructionArgs,
        programId
      );

    const createMsg = new TransactionMessage({
      payerKey: creator.publicKey,
      recentBlockhash: (await connection.getLatestBlockhash()).blockhash,
      instructions: [createIx],
    }).compileToV0Message();
    const createTx = new VersionedTransaction(createMsg);
    createTx.sign([creator]);
    const createSig = await connection.sendTransaction(createTx, {
      skipPreflight: false,
    });
    await connection.confirmTransaction(createSig);

    // Verify buffer exists
    let bufferAccount = await connection.getAccountInfo(transactionBuffer);
    assert.ok(bufferAccount, "Buffer should exist after creation");
    console.log("    Buffer created:", transactionBuffer.toBase58());

    // Close buffer via V2 (native signer, no extra verification data)
    const closeIx =
      smartAccount.generated.createCloseTransactionBufferV2Instruction(
        {
          consensusAccount: settingsPda,
          transactionBuffer,
          creator: creator.publicKey,
        },
        {
          extraVerificationData: null,
        },
        programId
      );

    // Override creator to be signer (SDK has isSigner: false for V2)
    const creatorMeta = closeIx.keys.find((k) =>
      k.pubkey.equals(creator.publicKey)
    );
    if (creatorMeta) creatorMeta.isSigner = true;

    const closeMsg = new TransactionMessage({
      payerKey: creator.publicKey,
      recentBlockhash: (await connection.getLatestBlockhash()).blockhash,
      instructions: [closeIx],
    }).compileToV0Message();
    const closeTx = new VersionedTransaction(closeMsg);
    closeTx.sign([creator]);
    const closeSig = await connection.sendTransaction(closeTx, {
      skipPreflight: false,
    });
    await connection.confirmTransaction(closeSig);

    // Verify buffer is closed
    bufferAccount = await connection.getAccountInfo(transactionBuffer);
    assert.strictEqual(
      bufferAccount,
      null,
      "Buffer should be closed (null)"
    );
    console.log("    Buffer closed successfully");
  });
});
