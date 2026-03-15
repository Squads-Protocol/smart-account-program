import { Keypair, PublicKey } from "@solana/web3.js";
import * as smartAccount from "@sqds/smart-account";
import * as assert from "assert";
import {
  createAutonomousMultisig,
  createLocalhostConnection,
  generateFundedKeypair,
  generateSmartAccountSigners,
  getNextAccountIndex,
  getTestProgramId,
  TestMembers,
} from "../../utils";

const programId = getTestProgramId();
const connection = createLocalhostConnection();

describe("Instructions / change_threshold_as_authority", () => {
  let members: TestMembers;

  before(async () => {
    members = await generateSmartAccountSigners(connection);
  });

  let settingsPda: PublicKey;
  let configAuthority: Keypair;

  before(async () => {
    configAuthority = await generateFundedKeypair(connection);
    const accountIndex = await getNextAccountIndex(connection, programId);
    // Create new controlled smartAccount.
    settingsPda = (
      await createAutonomousMultisig({
        connection,
        accountIndex,
        members,
        threshold: 1,
        timeLock: 0,
        programId,
      })
    )[0];
  });

  it("error: invalid authority", async () => {
    const feePayer = await generateFundedKeypair(connection);
    await assert.rejects(
      smartAccount.rpc.createSettingsTransaction({
        connection,
        feePayer,
        settingsPda: settingsPda,
        transactionIndex: 1n,
        creator: members.proposer.publicKey,
        actions: [{ __kind: "ChangeThreshold", newThreshold: 1 }],
        programId,
      })
    ),
      /Attempted to perform an unauthorized action/;
  });

  it("error: change threshold to higher amount than members", async () => {
    const feePayer = await generateFundedKeypair(connection);
    const configTransactionCreateSignature =
      await smartAccount.rpc.createSettingsTransaction({
        connection,
        feePayer,
        settingsPda: settingsPda,
        transactionIndex: 1n,
        creator: members.proposer.publicKey,
        actions: [{ __kind: "ChangeThreshold", newThreshold: 10 }],
        signers: [members.proposer, feePayer],
        programId,
      });
    await connection.confirmTransaction(configTransactionCreateSignature);

    const createProposalSignature = await smartAccount.rpc.createProposal({
      connection,
      creator: members.proposer,
      settingsPda,
      feePayer,
      transactionIndex: 1n,
      isDraft: false,
      programId,
    });
    await connection.confirmTransaction(createProposalSignature);

    const approveSignature = await smartAccount.rpc.approveProposal({
      connection,
      feePayer: members.voter,
      settingsPda,
      transactionIndex: 1n,
      signer: members.voter,
      programId,
    });
    await connection.confirmTransaction(approveSignature);

    await assert.rejects(
      smartAccount.rpc.executeSettingsTransaction({
        connection,
        feePayer,
        settingsPda: settingsPda,
        transactionIndex: 1n,
        signer: members.executor,
        rentPayer: feePayer,
        programId,
      }),
      /Invalid threshold, must be between 1 and number of signers with vote permission/
    );
  });

  it("change `threshold` for the controlled smart account", async () => {
    const signature = await smartAccount.rpc.createSettingsTransaction({
      connection,
      feePayer: members.proposer,
      settingsPda: settingsPda,
      transactionIndex: 2n,
      creator: members.proposer.publicKey,
      actions: [{ __kind: "ChangeThreshold", newThreshold: 1 }],
      programId,
    });
    await connection.confirmTransaction(signature);
  });
});
