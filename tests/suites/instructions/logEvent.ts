import * as web3 from "@solana/web3.js";
import * as smartAccount from "@sqds/smart-account";
import { createLocalhostConnection, getTestProgramId } from "../../utils";
import assert from "assert";

const programId = getTestProgramId();

describe("Instructions / Log Event", () => {
  it("Calling log event after using assign", async () => {
    const connection = createLocalhostConnection();
    const feePayer = web3.Keypair.generate();

    let airdropSignature = await connection.requestAirdrop(
      feePayer.publicKey,
      1_000_000_000
    );
    await connection.confirmTransaction(airdropSignature);

    const keyPair = web3.Keypair.generate();

    let transferIx = web3.SystemProgram.transfer({
      fromPubkey: feePayer.publicKey,
      toPubkey: keyPair.publicKey,
      lamports: 1586880,
    });

    let allocateIx = web3.SystemProgram.allocate({
      accountPubkey: keyPair.publicKey,
      space: 100,
    });

    let ix = web3.SystemProgram.assign({
      accountPubkey: keyPair.publicKey,
      programId,
    });

    let logEventIx = smartAccount.generated.createLogEventInstruction(
      {
        logAuthority: keyPair.publicKey,
      },
      {
        args: {
          event: Buffer.from("test"),
        },
      },
      programId
    );

    let tx = new web3.Transaction().add(transferIx, allocateIx, ix);
    tx.recentBlockhash = (await connection.getLatestBlockhash()).blockhash;
    tx.feePayer = feePayer.publicKey;
    tx.partialSign(feePayer);
    tx.partialSign(keyPair);

    let signature = await connection.sendRawTransaction(tx.serialize(), {
      skipPreflight: true,
    });
    await connection.confirmTransaction(signature);

    let logEventTx = new web3.Transaction().add(logEventIx);
    logEventTx.recentBlockhash = (
      await connection.getLatestBlockhash()
    ).blockhash;
    logEventTx.feePayer = feePayer.publicKey;
    logEventTx.partialSign(feePayer);
    logEventTx.partialSign(keyPair);

    assert.rejects(
      connection.sendRawTransaction(logEventTx.serialize()),
      (error: any) => {
        return true;
      }
    );
  });
});
