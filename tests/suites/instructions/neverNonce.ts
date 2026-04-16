import assert from "assert";
import {
  Keypair,
  LAMPORTS_PER_SOL,
  NONCE_ACCOUNT_LENGTH,
  NonceAccount,
  SystemProgram,
  TransactionMessage,
  VersionedTransaction,
} from "@solana/web3.js";
import * as smartAccount from "@sqds/smart-account";

import {
  createLocalhostConnection,
  generateFundedKeypair,
  getTestProgramConfigAuthority,
  getTestProgramId,
} from "../../utils";

const connection = createLocalhostConnection();
const programId = getTestProgramId();
const programConfigPda = smartAccount.getProgramConfigPda({ programId })[0];

describe("Never Nonce", () => {
  it("rejects durable nonce transactions", async () => {
    const programConfigAuthority = getTestProgramConfigAuthority();
    const nonceAuthority = await generateFundedKeypair(connection);
    const nonceAccount = Keypair.generate();

    const authorityFundingSignature = await connection.requestAirdrop(
      programConfigAuthority.publicKey,
      LAMPORTS_PER_SOL
    );
    await connection.confirmTransaction(authorityFundingSignature);

    const nonceRent = await connection.getMinimumBalanceForRentExemption(
      NONCE_ACCOUNT_LENGTH
    );
    const nonceSetupTx = SystemProgram.createNonceAccount({
      fromPubkey: nonceAuthority.publicKey,
      noncePubkey: nonceAccount.publicKey,
      authorizedPubkey: nonceAuthority.publicKey,
      lamports: nonceRent,
    });
    nonceSetupTx.recentBlockhash = (await connection.getLatestBlockhash()).blockhash;
    nonceSetupTx.feePayer = nonceAuthority.publicKey;
    nonceSetupTx.sign(nonceAuthority, nonceAccount);

    const nonceSetupSignature = await connection.sendRawTransaction(
      nonceSetupTx.serialize()
    );
    await connection.confirmTransaction(nonceSetupSignature);

    const nonceAccountInfo = await connection.getAccountInfo(nonceAccount.publicKey);
    assert.ok(nonceAccountInfo);

    const durableNonce = NonceAccount.fromAccountData(nonceAccountInfo.data).nonce;

    const advanceNonceIx = SystemProgram.nonceAdvance({
      noncePubkey: nonceAccount.publicKey,
      authorizedPubkey: nonceAuthority.publicKey,
    });
    const programIx =
      smartAccount.generated.createSetProgramConfigSmartAccountCreationFeeInstruction(
        {
          programConfig: programConfigPda,
          authority: programConfigAuthority.publicKey,
        },
        {
          args: {
            newSmartAccountCreationFee: 0,
          },
        },
        programId
      );

    const message = new TransactionMessage({
      payerKey: programConfigAuthority.publicKey,
      recentBlockhash: durableNonce,
      instructions: [advanceNonceIx, programIx],
    }).compileToV0Message();
    const transaction = new VersionedTransaction(message);
    transaction.sign([programConfigAuthority, nonceAuthority]);

    await assert.rejects(
      () =>
        connection
          .sendRawTransaction(transaction.serialize())
          .catch(smartAccount.errors.translateAndThrowAnchorError),
      /DurableNonceForbidden: Durable nonce transactions are not allowed/
    );
  });
});
