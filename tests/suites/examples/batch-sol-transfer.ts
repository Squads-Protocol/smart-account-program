import {
  AddressLookupTableAccount,
  AddressLookupTableProgram,
  LAMPORTS_PER_SOL,
  SystemProgram,
  TransactionMessage,
  VersionedTransaction,
} from "@solana/web3.js";
import * as multisig from "@sqds/multisig";
import assert from "assert";
import {
  createAutonomousMultisig,
  createLocalhostConnection,
  createTestTransferInstruction,
  generateFundedKeypair,
  generateMultisigMembers,
  getNextAccountIndex,
  getTestProgramId,
  range,
  TestMembers,
} from "../../utils";

const { Settings, Proposal } = multisig.accounts;

const programId = getTestProgramId();

describe("Examples / Batch SOL Transfer", () => {
  const connection = createLocalhostConnection();

  let members: TestMembers;
  before(async () => {
    members = await generateMultisigMembers(connection);
  });

  it("create and execute batch transaction containing multiple SOL transfers", async () => {
    // Use a different fee payer for the batch execution to isolate member balance changes.
    const feePayer = await generateFundedKeypair(connection);

    const accountIndex = await getNextAccountIndex(connection, programId);

    const [settingsPda] = await createAutonomousMultisig({
      connection,
      members,
      threshold: 2,
      timeLock: 0,
      programId,
      accountIndex,
    });

    let multisigAccount = await Settings.fromAccountAddress(
      connection,
      settingsPda
    );

    const vaultIndex = 0;
    const batchIndex =
      multisig.utils.toBigInt(multisigAccount.transactionIndex) + 1n;

    const [proposalPda] = multisig.getProposalPda({
      settingsPda,
      transactionIndex: batchIndex,
      programId,
    });

    // Default vault, index 0.
    const [vaultPda] = multisig.getSmartAccountPda({
      settingsPda,
      accountIndex: 0,
      programId,
    });

    // Prepare transactions for the batch.
    // We are going to make a payout of 1 SOL to every member of the multisig
    // first as a separate transaction per member, then in a single transaction
    // that also uses an Account Lookup Table containing all member addresses.
    // Airdrop SOL amount required for the payout to the Vault.
    const airdropSig = await connection.requestAirdrop(
      vaultPda,
      // Each member will be paid 2 x 1 SOL.
      Object.keys(members).length * 2 * LAMPORTS_PER_SOL
    );
    await connection.confirmTransaction(airdropSig);
    const {
      value: { blockhash },
      context: { slot },
    } = await connection.getLatestBlockhashAndContext("finalized");

    const testTransactionMessages = [] as {
      message: TransactionMessage;
      addressLookupTableAccounts: AddressLookupTableAccount[];
    }[];
    for (const member of Object.values(members)) {
      const ix = createTestTransferInstruction(
        vaultPda,
        member.publicKey,
        LAMPORTS_PER_SOL
      );
      testTransactionMessages.push({
        message: new TransactionMessage({
          payerKey: vaultPda,
          recentBlockhash: blockhash,
          instructions: [ix],
        }),
        addressLookupTableAccounts: [],
      });
    }

    // Create a lookup table with all member addresses.
    const memberAddresses = Object.values(members).map((m) => m.publicKey);
    const [lookupTableIx, lookupTableAddress] =
      AddressLookupTableProgram.createLookupTable({
        authority: feePayer.publicKey,
        payer: feePayer.publicKey,
        recentSlot: slot,
      });
    const extendTableIx = AddressLookupTableProgram.extendLookupTable({
      payer: feePayer.publicKey,
      authority: feePayer.publicKey,
      lookupTable: lookupTableAddress,
      addresses: [SystemProgram.programId, ...memberAddresses],
    });

    const createLookupTableTx = new VersionedTransaction(
      new TransactionMessage({
        payerKey: feePayer.publicKey,
        recentBlockhash: blockhash,
        instructions: [lookupTableIx, extendTableIx],
      }).compileToV0Message()
    );
    createLookupTableTx.sign([feePayer]);
    let signature = await connection
      .sendRawTransaction(createLookupTableTx.serialize())
      .catch((err: any) => {
        console.error(err.logs);
        throw err;
      });
    await connection.confirmTransaction(signature);

    const lookupTableAccount = await connection
      .getAddressLookupTable(lookupTableAddress)
      .then((res) => res.value);
    assert.ok(lookupTableAccount);

    const batchTransferIxs = Object.values(members).map((member) =>
      createTestTransferInstruction(
        vaultPda,
        member.publicKey,
        LAMPORTS_PER_SOL
      )
    );
    testTransactionMessages.push({
      message: new TransactionMessage({
        payerKey: vaultPda,
        recentBlockhash: blockhash,
        instructions: batchTransferIxs,
      }),
      addressLookupTableAccounts: [lookupTableAccount],
    });

    // Create a batch account.
    signature = await multisig.rpc.createBatch({
      connection,
      feePayer: members.proposer,
      settingsPda,
      creator: members.proposer,
      batchIndex,
      accountIndex: vaultIndex,
      memo: "Distribute funds to members",
      programId,
    });
    await connection.confirmTransaction(signature);

    // Initialize the proposal for the batch.
    signature = await multisig.rpc.createProposal({
      connection,
      feePayer: members.proposer,
      settingsPda,
      transactionIndex: batchIndex,
      creator: members.proposer,
      isDraft: true,
      programId,
    });
    await connection.confirmTransaction(signature);

    // Add transactions to the batch.
    for (const [
      index,
      { message, addressLookupTableAccounts },
    ] of testTransactionMessages.entries()) {
      signature = await multisig.rpc.addTransactionToBatch({
        connection,
        feePayer: members.proposer,
        settingsPda,
        signer: members.proposer,
        accountIndex: vaultIndex,
        batchIndex,
        // Batch transaction indices start at 1.
        transactionIndex: index + 1,
        ephemeralSigners: 0,
        transactionMessage: message,
        addressLookupTableAccounts,
        programId,
      });
      await connection.confirmTransaction(signature);
    }

    // Activate the proposal (finalize the batch).
    signature = await multisig.rpc.activateProposal({
      connection,
      feePayer: members.proposer,
      settingsPda,
      signer: members.proposer,
      transactionIndex: batchIndex,
      programId,
    });
    await connection.confirmTransaction(signature);

    // First approval for the batch proposal.
    signature = await multisig.rpc.approveProposal({
      connection,
      feePayer: members.voter,
      settingsPda,
      signer: members.voter,
      transactionIndex: batchIndex,
      memo: "LGTM",
      programId,
    });
    await connection.confirmTransaction(signature);

    // Second approval for the batch proposal.
    signature = await multisig.rpc.approveProposal({
      connection,
      feePayer: members.almighty,
      settingsPda,
      signer: members.almighty,
      transactionIndex: batchIndex,
      memo: "LGTM too",
      programId,
    });
    await connection.confirmTransaction(signature);

    // Fetch the member balances before the batch execution.
    const preBalances = [] as number[];
    for (const member of Object.values(members)) {
      const balance = await connection.getBalance(member.publicKey);
      preBalances.push(balance);
    }
    assert.strictEqual(Object.values(members).length, preBalances.length);

    // Execute the transactions from the batch sequentially one-by-one.
    for (const transactionIndex of range(1, testTransactionMessages.length)) {
      signature = await multisig.rpc.executeBatchTransaction({
        connection,
        feePayer: feePayer,
        settingsPda,
        signer: members.executor,
        batchIndex,
        transactionIndex,
        programId,
      });
      await connection.confirmTransaction(signature);
    }

    // Proposal status must be "Executed".
    const proposalAccount = await Proposal.fromAccountAddress(
      connection,
      proposalPda
    );
    assert.ok(multisig.types.isProposalStatusExecuted(proposalAccount.status));

    // Verify that the members received the funds.
    for (const [index, preBalance] of preBalances.entries()) {
      const postBalance = await connection.getBalance(
        Object.values(members)[index].publicKey
      );
      assert.strictEqual(postBalance, preBalance + 2 * LAMPORTS_PER_SOL);
    }
  });
});
