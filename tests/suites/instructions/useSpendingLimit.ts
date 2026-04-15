import {
  Keypair,
  LAMPORTS_PER_SOL,
  PublicKey,
  TransactionMessage,
  VersionedTransaction,
} from "@solana/web3.js";
import * as smartAccount from "@sqds/smart-account";
import assert from "assert";
import { BN } from "bn.js";
import {
  createAutonomousMultisig,
  createLocalhostConnection,
  generateSmartAccountSigners,
  getNextAccountIndex,
  getTestProgramId,
  TestMembers,
} from "../../utils";

const { SpendingLimit } = smartAccount.accounts;
const { Period } = smartAccount.types;

const programId = getTestProgramId();
const connection = createLocalhostConnection();

describe("Instructions / use_spending_limit", () => {
  let settingsPda: PublicKey;
  let members: TestMembers;
  let solSpendingLimitSeed: PublicKey;
  let solSpendingLimitPda: PublicKey;
  let destination: PublicKey;

  before(async () => {
    members = await generateSmartAccountSigners(connection);
    const accountIndex = await getNextAccountIndex(connection, programId);

    settingsPda = (
      await createAutonomousMultisig({
        connection,
        members,
        threshold: 1,
        timeLock: 0,
        programId,
        accountIndex,
      })
    )[0];

    // Airdrop SOL to vault
    const [vaultPda] = smartAccount.getSmartAccountPda({
      settingsPda,
      accountIndex: 0,
      programId,
    });
    await connection.confirmTransaction(
      await connection.requestAirdrop(vaultPda, 10 * LAMPORTS_PER_SOL)
    );

    // Set up spending limit params
    solSpendingLimitSeed = Keypair.generate().publicKey;
    destination = Keypair.generate().publicKey;

    [solSpendingLimitPda] = smartAccount.getSpendingLimitPda({
      settingsPda,
      seed: solSpendingLimitSeed,
      programId,
    });

    // Create spending limit via settings transaction
    const transactionIndex = 1n;
    const message = new TransactionMessage({
      payerKey: members.almighty.publicKey,
      recentBlockhash: (await connection.getLatestBlockhash()).blockhash,
      instructions: [
        smartAccount.instructions.createSettingsTransaction({
          settingsPda,
          transactionIndex,
          creator: members.almighty.publicKey,
          actions: [
            {
              __kind: "AddSpendingLimit",
              seed: solSpendingLimitSeed,
              accountIndex: 0,
              mint: PublicKey.default,
              amount: 5 * LAMPORTS_PER_SOL,
              period: Period.OneTime,
              signers: [members.almighty.publicKey],
              destinations: [destination],
              expiration: new BN("9223372036854775807"),
            },
          ],
          programId,
        }),
        smartAccount.instructions.createProposal({
          settingsPda,
          transactionIndex,
          creator: members.almighty.publicKey,
          programId,
        }),
        smartAccount.instructions.approveProposal({
          settingsPda,
          transactionIndex,
          signer: members.almighty.publicKey,
          programId,
        }),
      ],
    }).compileToV0Message();

    const tx = new VersionedTransaction(message);
    tx.sign([members.almighty]);

    let signature = await connection
      .sendTransaction(tx, { skipPreflight: true })
      .catch((err) => {
        console.log(err.logs);
        throw err;
      });
    await connection.confirmTransaction(signature);

    // Execute to create the spending limit
    signature = await smartAccount.rpc
      .executeSettingsTransaction({
        connection,
        feePayer: members.executor,
        settingsPda,
        transactionIndex,
        signer: members.executor,
        rentPayer: members.executor,
        spendingLimits: [solSpendingLimitPda],
        programId,
      })
      .catch((err) => {
        console.log(err.logs);
        throw err;
      });
    await connection.confirmTransaction(signature);
  });

  it("use SOL spending limit and verify remaining amount", async () => {
    // Verify spending limit was created
    let spendingLimitAccount = await SpendingLimit.fromAccountAddress(
      connection,
      solSpendingLimitPda
    );
    assert.strictEqual(
      spendingLimitAccount.remainingAmount.toString(),
      (5 * LAMPORTS_PER_SOL).toString()
    );

    // Use half the spending limit
    const useAmount = 2 * LAMPORTS_PER_SOL;
    const signature = await smartAccount.rpc
      .useSpendingLimit({
        connection,
        feePayer: members.almighty,
        signer: members.almighty,
        settingsPda,
        spendingLimit: solSpendingLimitPda,
        mint: undefined,
        accountIndex: 0,
        amount: useAmount,
        decimals: 9,
        destination,
        programId,
      })
      .catch((err) => {
        console.log(err.logs);
        throw err;
      });
    await connection.confirmTransaction(signature);

    // Verify remaining amount decreased
    spendingLimitAccount = await SpendingLimit.fromAccountAddress(
      connection,
      solSpendingLimitPda
    );
    assert.strictEqual(
      spendingLimitAccount.remainingAmount.toString(),
      (3 * LAMPORTS_PER_SOL).toString()
    );

    // Verify destination received funds
    const destinationBalance = await connection.getBalance(destination);
    assert.strictEqual(destinationBalance, useAmount);
  });

  it("error: spending limit exceeded", async () => {
    await assert.rejects(
      () =>
        smartAccount.rpc.useSpendingLimit({
          connection,
          feePayer: members.almighty,
          signer: members.almighty,
          settingsPda,
          spendingLimit: solSpendingLimitPda,
          mint: undefined,
          accountIndex: 0,
          amount: 10 * LAMPORTS_PER_SOL,
          decimals: 9,
          destination,
          programId,
        }),
      /Spending limit exceeded/
    );
  });
});
