import { TransactionMessage, VersionedTransaction } from "@solana/web3.js";
import * as multisig from "@sqds/multisig";
import assert from "assert";
import {
  createAutonomousMultisig,
  createLocalhostConnection,
  generateMultisigMembers,
  getNextAccountIndex,
  getTestProgramId,
  TestMembers,
} from "../../utils";

const { Settings } = multisig.accounts;

const programId = getTestProgramId();

/**
 * If user can sign a transaction with enough member keys to reach the threshold,
 * they can batch all multisig instructions required to create, approve and execute the multisig transaction
 * into one Solana transaction, so the transaction is executed immediately.
 */
describe("Examples / Immediate Execution", () => {
  const connection = createLocalhostConnection();

  let members: TestMembers;
  before(async () => {
    members = await generateMultisigMembers(connection);
  });

  it("create, approve and execute, all in 1 Solana transaction", async () => {
    const accountIndex = await getNextAccountIndex(connection, programId);

    const [settingsPda] = await createAutonomousMultisig({
      connection,
      members,
      threshold: 1,
      timeLock: 0,
      programId,
      accountIndex,
    });

    const transactionIndex = 1n;

    const createTransactionIx = multisig.instructions.createSettingsTransaction({
      settingsPda,
      transactionIndex,
      creator: members.almighty.publicKey,
      // Change threshold to 2.
      actions: [{ __kind: "ChangeThreshold", newThreshold: 2 }],
      programId,
    });
    const createProposalIx = multisig.instructions.createProposal({
      settingsPda,
      transactionIndex,
      creator: members.almighty.publicKey,
      programId,
    });

    const approveProposalIx = multisig.instructions.approveProposal({
      settingsPda,
      transactionIndex,
      signer: members.almighty.publicKey,
      programId,
    });

    const executeTransactionIx = multisig.instructions.executeSettingsTransaction(
      {
        settingsPda,
        transactionIndex,
        signer: members.almighty.publicKey,
        rentPayer: members.almighty.publicKey,
        programId,
      }
    );

    const message = new TransactionMessage({
      payerKey: members.almighty.publicKey,
      recentBlockhash: (await connection.getLatestBlockhash()).blockhash,
      instructions: [
        createTransactionIx,
        createProposalIx,
        approveProposalIx,
        executeTransactionIx,
      ],
    }).compileToV0Message();

    const tx = new VersionedTransaction(message);

    tx.sign([members.almighty]);

    const signature = await connection.sendTransaction(tx, {
      skipPreflight: true,
    });
    await connection.confirmTransaction(signature);

    // Verify the multisig account.
    const multisigAccount = await Settings.fromAccountAddress(
      connection,
      settingsPda
    );

    // The threshold should be updated.
    assert.strictEqual(multisigAccount.threshold, 2);
  });
});
