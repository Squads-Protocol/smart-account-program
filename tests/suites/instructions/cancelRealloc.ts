import {
  Keypair,
  LAMPORTS_PER_SOL,
  PublicKey,
  TransactionMessage,
} from "@solana/web3.js";
import * as smartAccount from "@sqds/smart-account";
import assert from "assert";
import {
  createAutonomousMultisig,
  createLocalhostConnection,
  createTestTransferInstruction,
  generateSmartAccountSigners,
  getNextAccountIndex,
  getTestProgramId,
  TestMembers,
} from "../../utils";

const { Settings, Proposal } = smartAccount.accounts;

const programId = getTestProgramId();
const connection = createLocalhostConnection();

describe("Instructions / proposal_cancel_v2", () => {
  let members: TestMembers;
  let settingsPda: PublicKey;
  let newVotingMember = new Keypair();
  let newVotingMember2 = new Keypair();
  let newVotingMember3 = new Keypair();
  let newVotingMember4 = new Keypair();
  let addMemberCollection = [
    {
      key: newVotingMember.publicKey,
      permissions: smartAccount.types.Permissions.all(),
    },
    {
      key: newVotingMember2.publicKey,
      permissions: smartAccount.types.Permissions.all(),
    },
    {
      key: newVotingMember3.publicKey,
      permissions: smartAccount.types.Permissions.all(),
    },
    {
      key: newVotingMember4.publicKey,
      permissions: smartAccount.types.Permissions.all(),
    },
  ];
  let cancelVotesCollection = [
    newVotingMember,
    newVotingMember2,
    newVotingMember3,
    newVotingMember4,
  ];
  let originalCancel: Keypair;

  before(async () => {
    members = await generateSmartAccountSigners(connection);
    const accountIndex = await getNextAccountIndex(connection, programId);
    // Create new autonomous smartAccount.
    settingsPda = (
      await createAutonomousMultisig({
        connection,
        members,
        threshold: 2,
        timeLock: 0,
        programId,
        accountIndex,
      })
    )[0];
  });

  // smart account current has a threhsold of 2 with two voting members.
  // create a proposal to add asignerto the smart account (which we will cancel)
  // the proposal size will be allocated to TOTAL members length
  it("cancel basic config tx proposal", async () => {
    // Create a settings transaction.
    const transactionIndex = 1n;
    const [proposalPda] = smartAccount.getProposalPda({
      settingsPda,
      transactionIndex,
      programId,
    });

    let signature = await smartAccount.rpc.createSettingsTransaction({
      connection,
      feePayer: members.proposer,
      settingsPda,
      transactionIndex,
      creator: members.proposer.publicKey,
      actions: [
        {
          __kind: "AddSigner",
          newSigner: {
            key: newVotingMember.publicKey,
            permissions: smartAccount.types.Permissions.all(),
          },
        },
      ],
      programId,
    });
    await connection.confirmTransaction(signature);

    // Create a proposal for the transaction.
    signature = await smartAccount.rpc.createProposal({
      connection,
      feePayer: members.proposer,
      settingsPda,
      transactionIndex,
      creator: members.proposer,
      programId,
    });
    await connection.confirmTransaction(signature);

    // Proposal status must be "Cancelled".
    let proposalAccount = await Proposal.fromAccountAddress(
      connection,
      proposalPda
    );

    // Approve the proposal 1.
    signature = await smartAccount.rpc.approveProposal({
      connection,
      feePayer: members.voter,
      settingsPda,
      transactionIndex,
      signer: members.voter,
      programId,
    });
    await connection.confirmTransaction(signature);

    // Approve the proposal 2.
    signature = await smartAccount.rpc.approveProposal({
      connection,
      feePayer: members.almighty,
      settingsPda,
      transactionIndex,
      signer: members.almighty,
      programId,
    });
    await connection.confirmTransaction(signature);

    // Proposal is now ready to execute, cast the 2 cancels using the new functionality.
    signature = await smartAccount.rpc.cancelProposal({
      connection,
      feePayer: members.voter,
      signer: members.voter,
      settingsPda,
      transactionIndex,
      programId,
    });
    await connection.confirmTransaction(signature);

    // Proposal is now ready to execute, cast the 2 cancels using the new functionality.
    signature = await smartAccount.rpc.cancelProposal({
      connection,
      feePayer: members.almighty,
      signer: members.almighty,
      settingsPda,
      transactionIndex,
      programId,
    });
    await connection.confirmTransaction(signature);

    // Proposal status must be "Cancelled".
    proposalAccount = await Proposal.fromAccountAddress(
      connection,
      proposalPda
    );
    assert.ok(
      smartAccount.types.isProposalStatusCancelled(proposalAccount.status)
    );
  });

  // in order to test this, we create a basic transfer transaction
  // then we vote to approve it
  // then we cast 1 cancel vote
  // then we change the state of the smart account so the new amount of voting members is greater than the last total size
  // then we change the threshold to be greater than the last total size
  // then we change the state of the smart account so that one original cancel voter is removed
  // then we vote to cancel (and be able to close the transfer transaction)
  it("cancel tx with stale state size", async () => {
    // Create a settings transaction.
    let transactionIndex = 2n;
    const [proposalPda] = smartAccount.getProposalPda({
      settingsPda,
      transactionIndex,
      programId,
    });

    // Default vault.
    const [vaultPda, vaultBump] = smartAccount.getSmartAccountPda({
      settingsPda,
      accountIndex: 0,
      programId,
    });
    const testPayee = Keypair.generate();
    const testIx1 = await createTestTransferInstruction(
      vaultPda,
      testPayee.publicKey,
      1 * LAMPORTS_PER_SOL
    );
    const testTransferMessage = new TransactionMessage({
      payerKey: vaultPda,
      recentBlockhash: (await connection.getLatestBlockhash()).blockhash,
      instructions: [testIx1],
    });

    let signature = await smartAccount.rpc.createTransaction({
      connection,
      feePayer: members.proposer,
      settingsPda,
      transactionIndex,
      creator: members.proposer.publicKey,
      accountIndex: 0,
      ephemeralSigners: 0,
      transactionMessage: testTransferMessage,
      memo: "Transfer 1 SOL to a test account",
      programId,
    });
    await connection.confirmTransaction(signature);

    // Create a proposal for the transaction.
    signature = await smartAccount.rpc.createProposal({
      connection,
      feePayer: members.proposer,
      settingsPda,
      transactionIndex,
      creator: members.proposer,
      programId,
    });
    await connection.confirmTransaction(signature);

    // Approve the proposal 1.
    signature = await smartAccount.rpc.approveProposal({
      connection,
      feePayer: members.voter,
      settingsPda,
      transactionIndex,
      signer: members.voter,
      programId,
    });
    await connection.confirmTransaction(signature);

    // Approve the proposal 2.
    signature = await smartAccount.rpc.approveProposal({
      connection,
      feePayer: members.almighty,
      settingsPda,
      transactionIndex,
      signer: members.almighty,
      programId,
    });
    await connection.confirmTransaction(signature);

    // Proposal status must be "Approved".
    let proposalAccount = await Proposal.fromAccountAddress(
      connection,
      proposalPda
    );
    assert.ok(
      smartAccount.types.isProposalStatusApproved(proposalAccount.status)
    );
    // check the account size

    // TX/Proposal is now in an approved/ready state.
    // Now cancel vec has enough room for 4 votes.

    // Cast the 1 cancel using the new functionality and the 'voter' member.
    signature = await smartAccount.rpc.cancelProposal({
      connection,
      feePayer: members.voter,
      signer: members.voter,
      settingsPda,
      transactionIndex,
      programId,
    });
    await connection.confirmTransaction(signature);
    // set the original cancel voter
    originalCancel = members.voter;

    // ensure that the account size has not changed yet

    // Change the smart account state to have 5 voting members.
    // loop through the process to add the 4 members
    for (let i = 0; i < addMemberCollection.length; i++) {
      const newMember = addMemberCollection[i];
      transactionIndex++;
      signature = await smartAccount.rpc.createSettingsTransaction({
        connection,
        feePayer: members.proposer,
        settingsPda,
        transactionIndex,
        creator: members.proposer.publicKey,
        actions: [{ __kind: "AddSigner", newSigner: newMember }],
        programId,
      });
      await connection.confirmTransaction(signature);

      // Create a proposal for the transaction.
      signature = await smartAccount.rpc.createProposal({
        connection,
        feePayer: members.proposer,
        settingsPda,
        transactionIndex,
        creator: members.proposer,
        programId,
      });
      await connection.confirmTransaction(signature);

      // Proposal status must be "Cancelled".
      proposalAccount = await Proposal.fromAccountAddress(
        connection,
        proposalPda
      );

      // Approve the proposal 1.
      signature = await smartAccount.rpc.approveProposal({
        connection,
        feePayer: members.voter,
        settingsPda,
        transactionIndex,
        signer: members.voter,
        programId,
      });
      await connection.confirmTransaction(signature);

      // Approve the proposal 2.
      signature = await smartAccount.rpc.approveProposal({
        connection,
        feePayer: members.almighty,
        settingsPda,
        transactionIndex,
        signer: members.almighty,
        programId,
      });
      await connection.confirmTransaction(signature);

      // use the execute onlysignerto execute
      signature = await smartAccount.rpc.executeSettingsTransaction({
        connection,
        feePayer: members.executor,
        settingsPda,
        transactionIndex,
        signer: members.executor,
        rentPayer: members.executor,
        programId,
      });
      await connection.confirmTransaction(signature);
    }

    // assert that oursignerlength is now 8
    let multisigAccount = await Settings.fromAccountAddress(
      connection,
      settingsPda
    );
    assert.strictEqual(multisigAccount.signers.length, 8);

    transactionIndex++;
    // now remove the original cancel voter
    signature = await smartAccount.rpc.createSettingsTransaction({
      connection,
      feePayer: members.proposer,
      settingsPda,
      transactionIndex,
      creator: members.proposer.publicKey,
      actions: [
        { __kind: "RemoveSigner", oldSigner: originalCancel.publicKey },
        { __kind: "ChangeThreshold", newThreshold: 5 },
      ],
      programId,
    });
    await connection.confirmTransaction(signature);
    // create the remove proposal
    signature = await smartAccount.rpc.createProposal({
      connection,
      feePayer: members.proposer,
      settingsPda,
      transactionIndex,
      creator: members.proposer,
      programId,
    });
    await connection.confirmTransaction(signature);
    // approve the proposal 1
    signature = await smartAccount.rpc.approveProposal({
      connection,
      feePayer: members.voter,
      settingsPda,
      transactionIndex,
      signer: members.voter,
      programId,
    });
    await connection.confirmTransaction(signature);
    // approve the proposal 2
    signature = await smartAccount.rpc.approveProposal({
      connection,
      feePayer: members.almighty,
      settingsPda,
      transactionIndex,
      signer: members.almighty,
      programId,
    });
    await connection.confirmTransaction(signature);
    // execute the proposal
    signature = await smartAccount.rpc.executeSettingsTransaction({
      connection,
      feePayer: members.executor,
      settingsPda,
      transactionIndex,
      signer: members.executor,
      rentPayer: members.executor,
      programId,
    });
    await connection.confirmTransaction(signature);
    // now assert we have 7 members
    multisigAccount = await Settings.fromAccountAddress(
      connection,
      settingsPda
    );
    assert.strictEqual(multisigAccount.signers.length, 7);
    assert.strictEqual(multisigAccount.threshold, 5);

    // so now our threshold should be 5 for cancelling, which exceeds the original space allocated at the beginning
    // get the original proposer and assert the originalCancel is in the cancel array
    proposalAccount = await Proposal.fromAccountAddress(
      connection,
      proposalPda
    );
    assert.strictEqual(proposalAccount.cancelled.length, 1);
    let deprecatedCancelVote = proposalAccount.cancelled[0];
    assert.ok(deprecatedCancelVote.equals(originalCancel.publicKey));

    // get the pre realloc size
    const rawProposal = await connection.getAccountInfo(proposalPda);
    const rawProposalData = rawProposal?.data.length;

    // now cast a cancel against it with the first all perm key
    signature = await smartAccount.rpc.cancelProposal({
      connection,
      feePayer: members.almighty,
      signer: members.almighty,
      settingsPda,
      transactionIndex: 2n,
      programId,
    });
    await connection.confirmTransaction(signature);
    // now assert that the cancelled array only has 1 key and it is the one that just voted
    proposalAccount = await Proposal.fromAccountAddress(
      connection,
      proposalPda
    );
    // check the data length to ensure it has changed
    const updatedRawProposal = await connection.getAccountInfo(proposalPda);
    const updatedRawProposalData = updatedRawProposal?.data.length;
    assert.notStrictEqual(updatedRawProposalData, rawProposalData);
    assert.strictEqual(proposalAccount.cancelled.length, 1);
    let newCancelVote = proposalAccount.cancelled[0];
    assert.ok(newCancelVote.equals(members.almighty.publicKey));
    // now cast 4 more cancels with the new key
    for (let i = 0; i < cancelVotesCollection.length; i++) {
      signature = await smartAccount.rpc.cancelProposal({
        connection,
        feePayer: members.executor,
        signer: cancelVotesCollection[i],
        settingsPda,
        transactionIndex: 2n,
        programId,
      });
      await connection.confirmTransaction(signature);
    }

    // now assert the proposals is cancelled
    proposalAccount = await Proposal.fromAccountAddress(
      connection,
      proposalPda
    );
    assert.ok(
      smartAccount.types.isProposalStatusCancelled(proposalAccount.status)
    );
    // assert there are 5 cancelled votes
    assert.strictEqual(proposalAccount.cancelled.length, 5);
  });
});
