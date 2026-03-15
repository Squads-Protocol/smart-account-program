import { Keypair, PublicKey } from "@solana/web3.js";
import * as smartAccount from "@sqds/smart-account";
import * as assert from "assert";
import BN from "bn.js";
import {
  createControlledSmartAccount,
  createLocalhostConnection,
  generateFundedKeypair,
  generateSmartAccountSigners,
  getNextAccountIndex,
  getTestProgramId,
  TestMembers,
} from "../../utils";

const { SpendingLimit } = smartAccount.accounts;

const programId = getTestProgramId();
const connection = createLocalhostConnection();

describe("Instructions / authority_add_spending_limit", () => {
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
        configAuthority: members.almighty.publicKey,
        members,
        threshold: 2,
        timeLock: 0,
        connection,
        programId,
      })
    )[0];

    feePayer = await generateFundedKeypair(connection);

    spendingLimitCreateKey = Keypair.generate().publicKey;

    spendingLimitPda = smartAccount.getSpendingLimitPda({
      settingsPda: controlledsettingsPda,
      seed: spendingLimitCreateKey,
      programId,
    })[0];
  });

  it("error: invalid authority", async () => {
    await assert.rejects(
      () =>
        smartAccount.rpc.addSpendingLimitAsAuthority({
          connection,
          feePayer: feePayer,
          settingsPda: controlledsettingsPda,
          spendingLimit: spendingLimitPda,
          seed: spendingLimitCreateKey,
          rentPayer: feePayer,
          amount: BigInt(1000000000),
          settingsAuthority: members.voter,
          period: smartAccount.generated.Period.Day,
          mint: Keypair.generate().publicKey,
          destinations: [Keypair.generate().publicKey],
          signers: [members.almighty.publicKey],
          accountIndex: 1,
          programId,
        }),
      /Attempted to perform an unauthorized action/
    );
  });

  it("error: invalid SpendingLimit amount", async () => {
    await assert.rejects(
      () =>
        smartAccount.rpc.addSpendingLimitAsAuthority({
          connection,
          feePayer: feePayer,
          settingsPda: controlledsettingsPda,
          spendingLimit: spendingLimitPda,
          seed: spendingLimitCreateKey,
          rentPayer: feePayer,
          // Must be positive.
          amount: BigInt(0),
          settingsAuthority: members.almighty,
          period: smartAccount.generated.Period.Day,
          mint: Keypair.generate().publicKey,
          destinations: [Keypair.generate().publicKey],
          signers: [members.almighty.publicKey],
          accountIndex: 1,
          programId,
        }),
      /Invalid SpendingLimit amount/
    );
  });

  it("create a new Spending Limit for the controlled smart account with signer of the smart account and non-signer", async () => {
    const nonMember = await generateFundedKeypair(connection);
    const expiration = Date.now() / 1000 + 5;
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
      signers: [members.almighty.publicKey, nonMember.publicKey],
      accountIndex: 1,
      expiration: expiration,
      sendOptions: { skipPreflight: true },
      programId,
    });

    await connection.confirmTransaction(signature);

    const spendingLimitAccount = await SpendingLimit.fromAccountAddress(
      connection,
      spendingLimitPda
    );
    assert.strictEqual(
      spendingLimitAccount.expiration.toString(),
      new BN(expiration).toString()
    );
  });
});
