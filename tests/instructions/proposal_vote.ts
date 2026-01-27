import * as smartAccount from "@sqds/smart-account";
import { Keypair, PublicKey, SystemProgram } from "@solana/web3.js";
import assert from "assert";
import {
  createLocalhostConnection,
  generateFundedKeypair,
  generateSmartAccountSigners,
  getTestProgramId,
  TestMembers,
} from "../utils";
import { createSettings as createSettingsHelper } from "./utils/settings";
import { sendV2Instruction } from "./utils/v2Instruction";

const { Proposal } = smartAccount.accounts;

const programId = getTestProgramId();
const connection = createLocalhostConnection();

describe("Instructions / proposal_vote", () => {
  const skip = { it: it.skip };
  let members: TestMembers;

  before(async () => {
    members = await generateSmartAccountSigners(connection);
  });

  const createSettings = (timeLock = 0) =>
    createSettingsHelper({ connection, members, programId, timeLock });

  const createSettingsTransaction = async (
    settingsPda: PublicKey,
    transactionIndex: bigint
  ) => {
    const signature = await smartAccount.rpc.createSettingsTransaction({
      connection,
      feePayer: members.proposer,
      settingsPda,
      transactionIndex,
      creator: members.proposer.publicKey,
      actions: [{ __kind: "ChangeThreshold", newThreshold: 1 }],
      programId,
    });
    await connection.confirmTransaction(signature);
  };

  const createProposal = async (settingsPda: PublicKey, transactionIndex: bigint) => {
    const signature = await smartAccount.rpc.createProposal({
      connection,
      feePayer: members.proposer,
      settingsPda,
      transactionIndex,
      creator: members.proposer,
      programId,
    });
    await connection.confirmTransaction(signature);

    return smartAccount.getProposalPda({
      settingsPda,
      transactionIndex,
      programId,
    })[0];
  };

  const createVoteInstruction = (vote: "approve" | "reject" | "cancel") => {
    switch (vote) {
      case "approve":
        return smartAccount.generated.createApproveProposalV2Instruction;
      case "reject":
        return smartAccount.generated.createRejectProposalV2Instruction;
      default:
        return smartAccount.generated.createCancelProposalV2Instruction;
    }
  };

  const sendVoteV2 = async ({
    consensusAccount,
    proposal,
    voter,
    voterSigner,
    vote,
    memo,
    includeVoterAsRemaining = true,
    includePayer = true,
    includeSystemProgram = true,
  }: {
    consensusAccount: PublicKey;
    proposal: PublicKey;
    voter: PublicKey;
    voterSigner?: Keypair;
    vote: "approve" | "reject" | "cancel";
    memo?: string | null;
    includeVoterAsRemaining?: boolean;
    includePayer?: boolean;
    includeSystemProgram?: boolean;
  }) => {
    const ixFactory = createVoteInstruction(vote);
    const ix = ixFactory(
      {
        consensusAccount,
        proposal,
        payer: includePayer ? members.proposer.publicKey : undefined,
        systemProgram: includeSystemProgram ? SystemProgram.programId : undefined,
        program: programId,
        anchorRemainingAccounts: includeVoterAsRemaining
          ? [
              {
                pubkey: voter,
                isSigner: true,
                isWritable: false,
              },
            ]
          : [],
      },
      {
        args: {
          voterKey: voter,
          clientDataParams: null,
          memo: memo ?? null,
        },
      },
      programId
    );

    await sendV2Instruction({
      connection,
      payer: members.proposer,
      instruction: ix,
      signers: voterSigner ? [voterSigner] : [],
    });
  };

  // Golden Path Tests (V1 + V2 parity)
  it("should_approve_proposal_v1", async () => {
    const settingsPda = await createSettings();
    const transactionIndex = 1n;
    await createSettingsTransaction(settingsPda, transactionIndex);
    await createProposal(settingsPda, transactionIndex);

    const signature = await smartAccount.rpc.approveProposal({
      connection,
      feePayer: members.voter,
      signer: members.voter,
      settingsPda,
      transactionIndex,
      programId,
    });
    await connection.confirmTransaction(signature);

    const proposalAccount = await Proposal.fromAccountAddress(
      connection,
      smartAccount.getProposalPda({
        settingsPda,
        transactionIndex,
        programId,
      })[0]
    );
    assert.ok(
      smartAccount.types.isProposalStatusApproved(proposalAccount.status)
    );
  });

  it("should_approve_proposal_v2", async () => {
    const settingsPda = await createSettings();
    const transactionIndex = 1n;
    await createSettingsTransaction(settingsPda, transactionIndex);
    const proposalPda = await createProposal(settingsPda, transactionIndex);

    await sendVoteV2({
      consensusAccount: settingsPda,
      proposal: proposalPda,
      voter: members.voter.publicKey,
      voterSigner: members.voter,
      vote: "approve",
    });

    const proposalAccount = await Proposal.fromAccountAddress(
      connection,
      proposalPda
    );
    assert.ok(
      smartAccount.types.isProposalStatusApproved(proposalAccount.status)
    );
  });

  it("should_reject_proposal_v1", async () => {
    const settingsPda = await createSettings();
    const transactionIndex = 1n;
    await createSettingsTransaction(settingsPda, transactionIndex);
    await createProposal(settingsPda, transactionIndex);

    const signature = await smartAccount.rpc.rejectProposal({
      connection,
      feePayer: members.voter,
      signer: members.voter,
      settingsPda,
      transactionIndex,
      programId,
    });
    await connection.confirmTransaction(signature);

    const proposalAccount = await Proposal.fromAccountAddress(
      connection,
      smartAccount.getProposalPda({
        settingsPda,
        transactionIndex,
        programId,
      })[0]
    );
    assert.ok(
      proposalAccount.rejected.some((rejected) =>
        rejected.equals(members.voter.publicKey)
      )
    );
  });

  it("should_reject_proposal_v2", async () => {
    const settingsPda = await createSettings();
    const transactionIndex = 1n;
    await createSettingsTransaction(settingsPda, transactionIndex);
    const proposalPda = await createProposal(settingsPda, transactionIndex);

    await sendVoteV2({
      consensusAccount: settingsPda,
      proposal: proposalPda,
      voter: members.voter.publicKey,
      voterSigner: members.voter,
      vote: "reject",
    });

    const proposalAccount = await Proposal.fromAccountAddress(
      connection,
      proposalPda
    );
    assert.ok(
      proposalAccount.rejected.some((rejected) =>
        rejected.equals(members.voter.publicKey)
      )
    );
  });

  it("should_cancel_proposal_v1", async () => {
    const settingsPda = await createSettings();
    const transactionIndex = 1n;
    await createSettingsTransaction(settingsPda, transactionIndex);
    await createProposal(settingsPda, transactionIndex);

    const approveSignature = await smartAccount.rpc.approveProposal({
      connection,
      feePayer: members.voter,
      signer: members.voter,
      settingsPda,
      transactionIndex,
      programId,
    });
    await connection.confirmTransaction(approveSignature);

    const cancelSignature = await smartAccount.rpc.cancelProposal({
      connection,
      feePayer: members.voter,
      signer: members.voter,
      settingsPda,
      transactionIndex,
      programId,
    });
    await connection.confirmTransaction(cancelSignature);

    const proposalAccount = await Proposal.fromAccountAddress(
      connection,
      smartAccount.getProposalPda({
        settingsPda,
        transactionIndex,
        programId,
      })[0]
    );
    assert.ok(
      smartAccount.types.isProposalStatusCancelled(proposalAccount.status)
    );
  });

  it("should_cancel_proposal_v2", async () => {
    const settingsPda = await createSettings();
    const transactionIndex = 1n;
    await createSettingsTransaction(settingsPda, transactionIndex);
    const proposalPda = await createProposal(settingsPda, transactionIndex);

    await sendVoteV2({
      consensusAccount: settingsPda,
      proposal: proposalPda,
      voter: members.voter.publicKey,
      voterSigner: members.voter,
      vote: "approve",
    });

    await sendVoteV2({
      consensusAccount: settingsPda,
      proposal: proposalPda,
      voter: members.voter.publicKey,
      voterSigner: members.voter,
      vote: "cancel",
    });

    const proposalAccount = await Proposal.fromAccountAddress(
      connection,
      proposalPda
    );
    assert.ok(
      smartAccount.types.isProposalStatusCancelled(proposalAccount.status)
    );
  });

  skip.it("should_emit_vote_events_v1");
  skip.it("should_emit_vote_events_v2");
  skip.it("should_realloc_on_cancel_when_needed_v1");
  skip.it("should_realloc_on_cancel_when_needed_v2");

  // Invariant & Safety Tests (V1 + V2 parity)
  skip.it("should_fail_on_invalid_consensus_account_derivation_v1");
  skip.it("should_fail_on_invalid_consensus_account_derivation_v2");
  skip.it("should_fail_when_consensus_account_inactive_v1");
  skip.it("should_fail_when_consensus_account_inactive_v2");

  it("should_fail_on_not_a_signer_v1", async () => {
    const settingsPda = await createSettings();
    const transactionIndex = 1n;
    await createSettingsTransaction(settingsPda, transactionIndex);
    await createProposal(settingsPda, transactionIndex);
    const nonMember = await generateFundedKeypair(connection);

    await assert.rejects(
      () =>
        smartAccount.rpc.approveProposal({
          connection,
          feePayer: nonMember,
          signer: nonMember,
          settingsPda,
          transactionIndex,
          programId,
        }),
      /NotASigner|Provided pubkey is not a signer of the smart account/
    );
  });

  it("should_fail_on_not_a_signer_v2", async () => {
    const settingsPda = await createSettings();
    const transactionIndex = 1n;
    await createSettingsTransaction(settingsPda, transactionIndex);
    const proposalPda = await createProposal(settingsPda, transactionIndex);
    const nonMember = await generateFundedKeypair(connection);

    await assert.rejects(
      () =>
        sendVoteV2({
          consensusAccount: settingsPda,
          proposal: proposalPda,
          voter: nonMember.publicKey,
          voterSigner: nonMember,
          vote: "approve",
        }),
      /NotASigner|Provided pubkey is not a signer of the smart account/
    );
  });

  it("should_fail_on_missing_vote_permission_v1", async () => {
    const settingsPda = await createSettings();
    const transactionIndex = 1n;
    await createSettingsTransaction(settingsPda, transactionIndex);
    await createProposal(settingsPda, transactionIndex);

    await assert.rejects(
      () =>
        smartAccount.rpc.approveProposal({
          connection,
          feePayer: members.executor,
          signer: members.executor,
          settingsPda,
          transactionIndex,
          programId,
        }),
      /Unauthorized|Attempted to perform an unauthorized action/
    );
  });

  it("should_fail_on_missing_vote_permission_v2", async () => {
    const settingsPda = await createSettings();
    const transactionIndex = 1n;
    await createSettingsTransaction(settingsPda, transactionIndex);
    const proposalPda = await createProposal(settingsPda, transactionIndex);

    await assert.rejects(
      () =>
        sendVoteV2({
          consensusAccount: settingsPda,
          proposal: proposalPda,
          voter: members.executor.publicKey,
          voterSigner: members.executor,
          vote: "approve",
        }),
      /Unauthorized|Attempted to perform an unauthorized action/
    );
  });

  it("should_fail_on_invalid_proposal_status_for_approve_v1", async () => {
    const settingsPda = await createSettings();
    const transactionIndex = 1n;
    await createSettingsTransaction(settingsPda, transactionIndex);

    const signature = await smartAccount.rpc.createProposal({
      connection,
      feePayer: members.proposer,
      settingsPda,
      transactionIndex,
      creator: members.proposer,
      isDraft: true,
      programId,
    });
    await connection.confirmTransaction(signature);

    await assert.rejects(
      () =>
        smartAccount.rpc.approveProposal({
          connection,
          feePayer: members.voter,
          signer: members.voter,
          settingsPda,
          transactionIndex,
          programId,
        }),
      /InvalidProposalStatus/ 
    );
  });

  it("should_fail_on_invalid_proposal_status_for_approve_v2", async () => {
    const settingsPda = await createSettings();
    const transactionIndex = 1n;
    await createSettingsTransaction(settingsPda, transactionIndex);

    const signature = await smartAccount.rpc.createProposal({
      connection,
      feePayer: members.proposer,
      settingsPda,
      transactionIndex,
      creator: members.proposer,
      isDraft: true,
      programId,
    });
    await connection.confirmTransaction(signature);
    const proposalPda = smartAccount.getProposalPda({
      settingsPda,
      transactionIndex,
      programId,
    })[0];

    await assert.rejects(
      () =>
        sendVoteV2({
          consensusAccount: settingsPda,
          proposal: proposalPda,
          voter: members.voter.publicKey,
          voterSigner: members.voter,
          vote: "approve",
        }),
      /InvalidProposalStatus/
    );
  });

  skip.it("should_fail_on_invalid_proposal_status_for_reject_v1");
  skip.it("should_fail_on_invalid_proposal_status_for_reject_v2");
  skip.it("should_fail_on_invalid_proposal_status_for_cancel_v1");
  skip.it("should_fail_on_invalid_proposal_status_for_cancel_v2");
  skip.it("should_fail_on_stale_proposal_for_approve_v1");
  skip.it("should_fail_on_stale_proposal_for_approve_v2");
  skip.it("should_fail_on_stale_proposal_for_reject_v1");
  skip.it("should_fail_on_stale_proposal_for_reject_v2");

  it("should_fail_on_double_approve_v1", async () => {
    const settingsPda = await createSettings();
    const transactionIndex = 1n;
    await createSettingsTransaction(settingsPda, transactionIndex);
    await createProposal(settingsPda, transactionIndex);

    const signature = await smartAccount.rpc.approveProposal({
      connection,
      feePayer: members.voter,
      signer: members.voter,
      settingsPda,
      transactionIndex,
      programId,
    });
    await connection.confirmTransaction(signature);

    await assert.rejects(
      () =>
        smartAccount.rpc.approveProposal({
          connection,
          feePayer: members.voter,
          signer: members.voter,
          settingsPda,
          transactionIndex,
          programId,
        }),
      /InvalidProposalStatus/
    );
  });

  it("should_fail_on_double_approve_v2", async () => {
    const settingsPda = await createSettings();
    const transactionIndex = 1n;
    await createSettingsTransaction(settingsPda, transactionIndex);
    const proposalPda = await createProposal(settingsPda, transactionIndex);

    await sendVoteV2({
      consensusAccount: settingsPda,
      proposal: proposalPda,
      voter: members.voter.publicKey,
      voterSigner: members.voter,
      vote: "approve",
    });

    await assert.rejects(
      () =>
        sendVoteV2({
          consensusAccount: settingsPda,
          proposal: proposalPda,
          voter: members.voter.publicKey,
          voterSigner: members.voter,
          vote: "approve",
        }),
      /InvalidProposalStatus/
    );
  });

  it("should_fail_on_double_reject_v1", async () => {
    const settingsPda = await createSettings();
    const transactionIndex = 1n;
    await createSettingsTransaction(settingsPda, transactionIndex);
    await createProposal(settingsPda, transactionIndex);

    const signature = await smartAccount.rpc.rejectProposal({
      connection,
      feePayer: members.voter,
      signer: members.voter,
      settingsPda,
      transactionIndex,
      programId,
    });
    await connection.confirmTransaction(signature);

    await assert.rejects(
      () =>
        smartAccount.rpc.rejectProposal({
          connection,
          feePayer: members.voter,
          signer: members.voter,
          settingsPda,
          transactionIndex,
          programId,
        }),
      /AlreadyRejected/
    );
  });

  it("should_fail_on_double_reject_v2", async () => {
    const settingsPda = await createSettings();
    const transactionIndex = 1n;
    await createSettingsTransaction(settingsPda, transactionIndex);
    const proposalPda = await createProposal(settingsPda, transactionIndex);

    await sendVoteV2({
      consensusAccount: settingsPda,
      proposal: proposalPda,
      voter: members.voter.publicKey,
      voterSigner: members.voter,
      vote: "reject",
    });

    await assert.rejects(
      () =>
        sendVoteV2({
          consensusAccount: settingsPda,
          proposal: proposalPda,
          voter: members.voter.publicKey,
          voterSigner: members.voter,
          vote: "reject",
        }),
      /AlreadyRejected/
    );
  });

  it("should_fail_on_double_cancel_v1", async () => {
    const settingsPda = await createSettings();
    const transactionIndex = 1n;
    await createSettingsTransaction(settingsPda, transactionIndex);
    await createProposal(settingsPda, transactionIndex);

    const approveSignature = await smartAccount.rpc.approveProposal({
      connection,
      feePayer: members.voter,
      signer: members.voter,
      settingsPda,
      transactionIndex,
      programId,
    });
    await connection.confirmTransaction(approveSignature);

    const cancelSignature = await smartAccount.rpc.cancelProposal({
      connection,
      feePayer: members.voter,
      signer: members.voter,
      settingsPda,
      transactionIndex,
      programId,
    });
    await connection.confirmTransaction(cancelSignature);

    await assert.rejects(
      () =>
        smartAccount.rpc.cancelProposal({
          connection,
          feePayer: members.voter,
          signer: members.voter,
          settingsPda,
          transactionIndex,
          programId,
        }),
      /InvalidProposalStatus/
    );
  });

  it("should_fail_on_double_cancel_v2", async () => {
    const settingsPda = await createSettings();
    const transactionIndex = 1n;
    await createSettingsTransaction(settingsPda, transactionIndex);
    const proposalPda = await createProposal(settingsPda, transactionIndex);

    await sendVoteV2({
      consensusAccount: settingsPda,
      proposal: proposalPda,
      voter: members.voter.publicKey,
      voterSigner: members.voter,
      vote: "approve",
    });

    await sendVoteV2({
      consensusAccount: settingsPda,
      proposal: proposalPda,
      voter: members.voter.publicKey,
      voterSigner: members.voter,
      vote: "cancel",
    });

    await assert.rejects(
      () =>
        sendVoteV2({
          consensusAccount: settingsPda,
          proposal: proposalPda,
          voter: members.voter.publicKey,
          voterSigner: members.voter,
          vote: "cancel",
        }),
      /InvalidProposalStatus/
    );
  });

  skip.it("should_fail_on_missing_payer_for_cancel_v1");
  skip.it("should_fail_on_missing_payer_for_cancel_v2");
  skip.it("should_fail_on_missing_system_program_for_cancel_v1");
  skip.it("should_fail_on_missing_system_program_for_cancel_v2");

  it("should_fail_on_invalid_v2_context_signature_v2", async () => {
    const settingsPda = await createSettings();
    const transactionIndex = 1n;
    await createSettingsTransaction(settingsPda, transactionIndex);
    const proposalPda = await createProposal(settingsPda, transactionIndex);

    await assert.rejects(
      () =>
        sendVoteV2({
          consensusAccount: settingsPda,
          proposal: proposalPda,
          voter: members.voter.publicKey,
          vote: "approve",
          includeVoterAsRemaining: false,
        }),
      /Missing signature|MissingSignature/
    );
  });

  skip.it("should_fail_on_invalid_webauthn_client_data_v2");

  // Edge Case Tests (V1 + V2 parity)
  skip.it("should_approve_at_threshold_v1");
  skip.it("should_approve_at_threshold_v2");
  skip.it("should_reject_at_cutoff_v1");
  skip.it("should_reject_at_cutoff_v2");
  skip.it("should_cancel_at_threshold_v1");
  skip.it("should_cancel_at_threshold_v2");
  skip.it("should_trim_cancelled_signers_not_in_settings_v1");
  skip.it("should_trim_cancelled_signers_not_in_settings_v2");

  it("should_allow_empty_memo_v1", async () => {
    const settingsPda = await createSettings();
    const transactionIndex = 1n;
    await createSettingsTransaction(settingsPda, transactionIndex);
    await createProposal(settingsPda, transactionIndex);

    const signature = await smartAccount.rpc.approveProposal({
      connection,
      feePayer: members.voter,
      signer: members.voter,
      settingsPda,
      transactionIndex,
      memo: "",
      programId,
    });
    await connection.confirmTransaction(signature);

    const proposalAccount = await Proposal.fromAccountAddress(
      connection,
      smartAccount.getProposalPda({
        settingsPda,
        transactionIndex,
        programId,
      })[0]
    );
    assert.ok(
      smartAccount.types.isProposalStatusApproved(proposalAccount.status)
    );
  });

  it("should_allow_empty_memo_v2", async () => {
    const settingsPda = await createSettings();
    const transactionIndex = 1n;
    await createSettingsTransaction(settingsPda, transactionIndex);
    const proposalPda = await createProposal(settingsPda, transactionIndex);

    await sendVoteV2({
      consensusAccount: settingsPda,
      proposal: proposalPda,
      voter: members.voter.publicKey,
      voterSigner: members.voter,
      vote: "approve",
      memo: "",
    });

    const proposalAccount = await Proposal.fromAccountAddress(
      connection,
      proposalPda
    );
    assert.ok(
      smartAccount.types.isProposalStatusApproved(proposalAccount.status)
    );
  });

  skip.it("should_reject_oversized_memo_v1");
  skip.it("should_reject_oversized_memo_v2");
  // -------------------------------------------------------------------------------------
  // Settings V2 Tests
  // -------------------------------------------------------------------------------------

  skip.it("should_run_with_native_settings_v2");
  skip.it("should_run_with_p256_webauthn_settings_v2");
  skip.it("should_run_with_secp256k1_settings_v2");
  skip.it("should_run_with_ed25519_external_settings_v2");
});
