import * as smartAccount from "@sqds/smart-account";
import * as web3 from "@solana/web3.js";
import assert from "assert";
import { BN } from "bn.js";
import {
  createAutonomousMultisig,
  createControlledSmartAccount,
  createLocalhostConnection,
  generateFundedKeypair,
  generateSmartAccountSigners,
  getNextAccountIndex,
  getTestProgramId,
  TestMembers,
} from "../../utils";

const programId = getTestProgramId();
const connection = createLocalhostConnection();

describe("Audit / AccountIndexLocked on Spending Limit", () => {
  let members: TestMembers;

  before(async () => {
    members = await generateSmartAccountSigners(connection);
  });

  it("settings transaction: AddSpendingLimit with locked index 1 fails", async () => {
    // Fresh autonomous multisig: account_utilization = 0, so only index 0 is unlocked
    const settingsPda = (
      await createAutonomousMultisig({
        connection,
        members,
        threshold: 1,
        timeLock: 0,
        programId,
      })
    )[0];

    const transactionIndex = BigInt(1);
    const seed = web3.Keypair.generate().publicKey;

    // Create settings transaction targeting index 1 (locked)
    let signature = await smartAccount.rpc.createSettingsTransaction({
      connection,
      feePayer: members.proposer,
      settingsPda,
      transactionIndex,
      creator: members.proposer.publicKey,
      actions: [
        {
          __kind: "AddSpendingLimit",
          seed,
          accountIndex: 1,
          mint: web3.PublicKey.default,
          amount: 1_000_000_000,
          period: smartAccount.types.Period.OneTime,
          signers: [members.almighty.publicKey],
          destinations: [web3.Keypair.generate().publicKey],
          expiration: new BN("9223372036854775807"),
        },
      ],
      programId,
    });
    await connection.confirmTransaction(signature);

    // Create and approve proposal
    signature = await smartAccount.rpc.createProposal({
      connection,
      feePayer: members.proposer,
      settingsPda,
      transactionIndex,
      creator: members.proposer,
      programId,
    });
    await connection.confirmTransaction(signature);

    signature = await smartAccount.rpc.approveProposal({
      connection,
      feePayer: members.voter,
      settingsPda,
      transactionIndex,
      signer: members.voter,
      programId,
    });
    await connection.confirmTransaction(signature);

    const [spendingLimitPda] = smartAccount.getSpendingLimitPda({
      settingsPda,
      seed,
      programId,
    });

    // Execution fails because index 1 > account_utilization (0)
    await assert.rejects(
      () =>
        smartAccount.rpc.executeSettingsTransaction({
          connection,
          feePayer: members.executor,
          settingsPda,
          transactionIndex,
          signer: members.executor,
          rentPayer: members.executor,
          spendingLimits: [spendingLimitPda],
          programId,
        }),
      /AccountIndexLocked/
    );
  });

  it("authority path: addSpendingLimitAsAuthority with locked index 1 fails", async () => {
    const accountIndex = await getNextAccountIndex(connection, programId);
    const controlledSettingsPda = (
      await createControlledSmartAccount({
        accountIndex,
        configAuthority: members.almighty.publicKey,
        members,
        threshold: 2,
        timeLock: 0,
        connection,
        programId,
      })
    )[0];

    const feePayer = await generateFundedKeypair(connection);
    const seed = web3.Keypair.generate().publicKey;
    const [spendingLimitPda] = smartAccount.getSpendingLimitPda({
      settingsPda: controlledSettingsPda,
      seed,
      programId,
    });

    // Direct authority call targeting index 1 (locked)
    await assert.rejects(
      () =>
        smartAccount.rpc.addSpendingLimitAsAuthority({
          connection,
          feePayer,
          settingsPda: controlledSettingsPda,
          spendingLimit: spendingLimitPda,
          seed,
          rentPayer: feePayer,
          amount: BigInt(1_000_000_000),
          settingsAuthority: members.almighty,
          period: smartAccount.generated.Period.OneTime,
          mint: web3.PublicKey.default,
          destinations: [web3.Keypair.generate().publicKey],
          signers: [members.almighty.publicKey],
          accountIndex: 1,
          programId,
        }),
      /AccountIndexLocked/
    );
  });
});
