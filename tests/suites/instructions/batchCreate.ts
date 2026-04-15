import {
  Keypair,
  PublicKey,
  TransactionMessage,
  VersionedTransaction,
} from "@solana/web3.js";
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
  formatsToRun,
  getRpc,
} from "../../utils";

const { Permissions } = smartAccount.types;

const programId = getTestProgramId();
const connection = createLocalhostConnection();

for (const format of formatsToRun) {
  const rpc = getRpc(format);

  describe(`Instructions / smart_account_batch_transactions [${format}]`, () => {
    const newSigner = {
      key: Keypair.generate().publicKey,
      permissions: Permissions.all(),
    } as const;
    const newMember2 = {
      key: Keypair.generate().publicKey,
      permissions: Permissions.all(),
    } as const;

    let members: TestMembers;
    let settingsPda: PublicKey;
    let configAuthority: Keypair;

    before(async () => {
      members = await generateSmartAccountSigners(connection);
      configAuthority = await generateFundedKeypair(connection);
      const accountIndex = await getNextAccountIndex(connection, programId);
      // Create new controlled smartAccount.
      settingsPda = (
        await createControlledSmartAccount({
          connection,
          accountIndex,
          configAuthority: configAuthority.publicKey,
          members,
          threshold: 2,
          timeLock: 0,
          programId,
        })
      )[0];

      // Increment account index to unlock vault index 1
      for (let i = 0; i <= 1; i++) {
        const incrementIx = smartAccount.generated.createIncrementAccountIndexInstruction(
          { settings: settingsPda, signer: members.almighty.publicKey, program: programId },
          programId
        );
        const msg = new TransactionMessage({
          payerKey: members.almighty.publicKey,
          recentBlockhash: (await connection.getLatestBlockhash()).blockhash,
          instructions: [incrementIx],
        }).compileToV0Message();
        const tx = new VersionedTransaction(msg);
        tx.sign([members.almighty]);
        const sig = await connection.sendRawTransaction(tx.serialize());
        await connection.confirmTransaction(sig);
      }
    });

    it("create a batch transaction", async () => {
      const feePayer = await generateFundedKeypair(connection);

      const createBatchSignature = await rpc.createBatch({
        connection,
        batchIndex: 1n,
        creator: members.proposer,
        feePayer,
        settingsPda,
        accountIndex: 1,
        programId,
      });
      await connection.confirmTransaction(createBatchSignature);
    });
  });
}
