/**
 * Standalone test: verify InterfaceAccount<ResolvedSigner> works at runtime.
 *
 * Run: npx mocha --node-option require=ts-node/register --extension ts -t 60000 tests/standalone/test-resolved-signer.ts
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
} from "../utils";

const programId = getTestProgramId();
const connection = createLocalhostConnection();

describe("ResolvedSigner: transaction_buffer_create", () => {
  let members: TestMembers;
  let settingsPda: PublicKey;
  let vaultPda: PublicKey;

  before(async () => {
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

  it("create transaction buffer with ResolvedSigner", async () => {
    const creator = members.almighty;

    // Build a dummy transfer message
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

    const msg = new TransactionMessage({
      payerKey: creator.publicKey,
      recentBlockhash: (await connection.getLatestBlockhash()).blockhash,
      instructions: [createIx],
    }).compileToV0Message();

    const tx = new VersionedTransaction(msg);
    tx.sign([creator]);

    const sig = await connection.sendTransaction(tx, { skipPreflight: false });
    await connection.confirmTransaction(sig);

    // Verify
    const bufferAccount = await connection.getAccountInfo(transactionBuffer);
    assert.ok(bufferAccount, "Transaction buffer account should exist");
    assert.strictEqual(
      bufferAccount.owner.toBase58(),
      programId.toBase58()
    );
    console.log("    Buffer created:", transactionBuffer.toBase58());
  });
});
