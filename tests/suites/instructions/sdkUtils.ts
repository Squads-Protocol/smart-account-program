import { PublicKey } from "@solana/web3.js";
import * as smartAccount from "@sqds/smart-account";
import * as assert from "assert";
import {
  createLocalhostConnection,
  fundKeypair,
  generateSmartAccountSigners,
  getNextAccountIndex,
  getTestAccountCreationAuthority,
  getTestProgramId,
  TestMembers,
} from "../../utils";

const { Permissions } = smartAccount.types;

const programId = getTestProgramId();
const connection = createLocalhostConnection();

describe("Instructions / utils", () => {
  let members: TestMembers;

  before(async () => {
    members = await generateSmartAccountSigners(connection);
  });

  describe("getAvailableMemoSize", () => {
    it("provides estimates for available size to use for memo", async () => {
      const multisigCreator = getTestAccountCreationAuthority();
      await fundKeypair(connection, multisigCreator);

      const accountIndex = await getNextAccountIndex(connection, programId);
      const [settingsPda] = smartAccount.getSettingsPda({
        accountIndex,
        programId,
      });
      const [configAuthority] = smartAccount.getSmartAccountPda({
        settingsPda,
        accountIndex: 0,
        programId,
      });
      const programConfigPda = smartAccount.getProgramConfigPda({
        programId,
      })[0];
      const programConfig =
        await smartAccount.accounts.ProgramConfig.fromAccountAddress(
          connection,
          programConfigPda
        );
      const treasury = programConfig.treasury;
      const multisigCreateArgs: Parameters<
        typeof smartAccount.transactions.createSmartAccount
      >[0] = {
        blockhash: (await connection.getLatestBlockhash()).blockhash,
        creator: multisigCreator.publicKey,
        treasury: treasury,
        rentCollector: null,
        settings: settingsPda,
        settingsAuthority: configAuthority,
        timeLock: 0,
        signers: [
          {
            __kind: "Native",
            key: members.almighty.publicKey,
            permissions: Permissions.all(),
          },
        ],
        threshold: 1,
        programId,
      };

      const createMultisigTxWithoutMemo =
        smartAccount.transactions.createSmartAccount(multisigCreateArgs);

      const availableMemoSize = smartAccount.utils.getAvailableMemoSize(
        createMultisigTxWithoutMemo
      );

      const memo = "a".repeat(availableMemoSize);

      const createMultisigTxWithMemo =
        smartAccount.transactions.createSmartAccount({
          ...multisigCreateArgs,
          memo,
        });
      // The transaction with memo should have the maximum allowed size.
      assert.strictEqual(createMultisigTxWithMemo.serialize().length, 1232);
      // The transaction should work.
      createMultisigTxWithMemo.sign([multisigCreator]);
      const signature = await connection.sendTransaction(
        createMultisigTxWithMemo
      );
      await connection.confirmTransaction(signature);
    });
  });
});
