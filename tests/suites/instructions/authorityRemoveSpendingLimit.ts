import { Keypair, PublicKey, TransactionMessage, VersionedTransaction } from "@solana/web3.js";
import * as smartAccount from "@sqds/smart-account";
import * as assert from "assert";
import {
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

describe("Instructions / authority_remove_spending_limit", () => {
  let members: TestMembers;

  before(async () => {
    members = await generateSmartAccountSigners(connection);
  });

  let controlledsettingsPda: PublicKey;
  let feePayer: Keypair;
  let spendingLimitPda: PublicKey;
  let spendingLimitCreateKey: PublicKey;

  before(async () => {
    const accountIndex = await getNextAccountIndex(connection, programId);
    controlledsettingsPda = (
      await createControlledSmartAccount({
        accountIndex,
        connection,
        configAuthority: members.almighty.publicKey,
        members,
        threshold: 2,
        timeLock: 0,
        programId,
      })
    )[0];

    // Increment account_utilization to unlock index 1 for spending limits
    {
      const ix = smartAccount.generated.createIncrementAccountIndexInstruction(
        { settings: controlledsettingsPda, signer: members.almighty.publicKey, program: programId },
        programId
      );
      const msg = new TransactionMessage({
        payerKey: members.almighty.publicKey,
        recentBlockhash: (await connection.getLatestBlockhash()).blockhash,
        instructions: [ix],
      }).compileToV0Message();
      const tx = new VersionedTransaction(msg);
      tx.sign([members.almighty]);
      const sig = await connection.sendRawTransaction(tx.serialize());
      await connection.confirmTransaction(sig);
    }

    feePayer = await generateFundedKeypair(connection);

    spendingLimitCreateKey = Keypair.generate().publicKey;

    spendingLimitPda = smartAccount.getSpendingLimitPda({
      settingsPda: controlledsettingsPda,
      seed: spendingLimitCreateKey,
      programId,
    })[0];

    const signature = await smartAccount.rpc.addSpendingLimitAsAuthority({
      connection,
      feePayer: feePayer,
      settingsPda: controlledsettingsPda,
      spendingLimit: spendingLimitPda,
      seed: spendingLimitCreateKey,
      rentPayer: feePayer,
      amount: BigInt(1000000000),
      settingsAuthority: members.almighty,
      period: smartAccount.generated.Period.Day,
      mint: Keypair.generate().publicKey,
      destinations: [Keypair.generate().publicKey],
      signers: [members.almighty.publicKey],
      accountIndex: 1,
      sendOptions: { skipPreflight: true },
      programId,
    });

    await connection.confirmTransaction(signature);
  });

  it("error: invalid authority", async () => {
    await assert.rejects(
      () =>
        smartAccount.rpc.removeSpendingLimitAsAuthority({
          connection,
          settingsPda: controlledsettingsPda,
          spendingLimit: spendingLimitPda,
          settingsAuthority: members.voter.publicKey,
          feePayer: feePayer,
          rentCollector: members.voter.publicKey,
          signers: [feePayer, members.voter],
          programId,
        }),
      /Attempted to perform an unauthorized action/
    );
  });

  it("error: Spending Limit doesn't belong to the smart account", async () => {
    const accountIndex = await getNextAccountIndex(connection, programId);
    const wrongControlledsettingsPda = (
      await createControlledSmartAccount({
        accountIndex,
        connection,
        configAuthority: members.almighty.publicKey,
        members,
        threshold: 2,
        timeLock: 0,
        programId,
      })
    )[0];

    // Increment account_utilization to unlock index 1 for spending limits
    {
      const ix = smartAccount.generated.createIncrementAccountIndexInstruction(
        { settings: wrongControlledsettingsPda, signer: members.almighty.publicKey, program: programId },
        programId
      );
      const msg = new TransactionMessage({
        payerKey: members.almighty.publicKey,
        recentBlockhash: (await connection.getLatestBlockhash()).blockhash,
        instructions: [ix],
      }).compileToV0Message();
      const tx = new VersionedTransaction(msg);
      tx.sign([members.almighty]);
      const sig = await connection.sendRawTransaction(tx.serialize());
      await connection.confirmTransaction(sig);
    }

    const wrongCreateKey = Keypair.generate().publicKey;
    const wrongSpendingLimitPda = smartAccount.getSpendingLimitPda({
      settingsPda: wrongControlledsettingsPda,
      seed: wrongCreateKey,
      programId,
    })[0];

    const addSpendingLimitSignature =
      await smartAccount.rpc.addSpendingLimitAsAuthority({
        connection,
        feePayer: feePayer,
        settingsPda: wrongControlledsettingsPda,
        spendingLimit: wrongSpendingLimitPda,
        seed: wrongCreateKey,
        rentPayer: feePayer,
        amount: BigInt(1000000000),
        settingsAuthority: members.almighty,
        period: smartAccount.generated.Period.Day,
        mint: Keypair.generate().publicKey,
        destinations: [Keypair.generate().publicKey],
        signers: [members.almighty.publicKey],
        accountIndex: 1,
        programId,
      });

    await connection.confirmTransaction(addSpendingLimitSignature);
    await assert.rejects(
      () =>
        smartAccount.rpc.removeSpendingLimitAsAuthority({
          connection,
          settingsPda: controlledsettingsPda,
          spendingLimit: wrongSpendingLimitPda,
          settingsAuthority: members.almighty.publicKey,
          feePayer: feePayer,
          rentCollector: members.almighty.publicKey,
          signers: [feePayer, members.almighty],
          programId,
        }),
      /Invalid account provided/
    );
  });

  it("remove the Spending Limit from the controlled smart account", async () => {
    const signature = await smartAccount.rpc.removeSpendingLimitAsAuthority({
      connection,
      settingsPda: controlledsettingsPda,
      spendingLimit: spendingLimitPda,
      settingsAuthority: members.almighty.publicKey,
      feePayer: feePayer,
      rentCollector: members.almighty.publicKey,
      sendOptions: { skipPreflight: true },
      signers: [feePayer, members.almighty],
      programId,
    });
    await connection.confirmTransaction(signature);
  });
});
