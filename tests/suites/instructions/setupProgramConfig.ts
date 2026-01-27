import { LAMPORTS_PER_SOL, TransactionMessage, VersionedTransaction } from "@solana/web3.js";
import * as smartAccount from "@sqds/smart-account";
import {
  createLocalhostConnection,
  getTestProgramConfigAuthority,
  getTestProgramConfigInitializer,
  getTestProgramId,
  getTestProgramTreasury,
} from "../../utils";

const programId = getTestProgramId();
const programConfigInitializer = getTestProgramConfigInitializer();
const programConfigAuthority = getTestProgramConfigAuthority();
const programTreasury = getTestProgramTreasury();
const programConfigPda = smartAccount.getProgramConfigPda({ programId })[0];

const connection = createLocalhostConnection();

before(async () => {
  const existing = await connection.getAccountInfo(programConfigPda);
  if (existing) {
    return;
  }

  const signature = await connection.requestAirdrop(
    programConfigInitializer.publicKey,
    LAMPORTS_PER_SOL
  );
  await connection.confirmTransaction(signature);

  const initIx = smartAccount.generated.createInitializeProgramConfigInstruction(
    {
      programConfig: programConfigPda,
      initializer: programConfigInitializer.publicKey,
    },
    {
      args: {
        authority: programConfigAuthority.publicKey,
        treasury: programTreasury,
        smartAccountCreationFee: 0,
      },
    },
    programId
  );

  const blockhash = (await connection.getLatestBlockhash()).blockhash;
  const message = new TransactionMessage({
    recentBlockhash: blockhash,
    payerKey: programConfigInitializer.publicKey,
    instructions: [initIx],
  }).compileToV0Message();
  const tx = new VersionedTransaction(message);
  tx.sign([programConfigInitializer]);

  const sig = await connection.sendRawTransaction(tx.serialize(), {
    skipPreflight: true,
  });
  const confirmation = await connection.confirmTransaction(sig);
  if (confirmation.value.err) {
    const txResult = await connection.getTransaction(sig, {
      commitment: "confirmed",
      maxSupportedTransactionVersion: 0,
    });
    const logs = txResult?.meta?.logMessages ?? [];
    throw new Error(
      `ProgramConfig init failed: ${JSON.stringify(
        confirmation.value.err
      )}\n${logs.join("\n")}`
    );
  }
});
