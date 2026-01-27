import * as smartAccount from "@sqds/smart-account";
import { Keypair, PublicKey } from "@solana/web3.js";
import BN from "bn.js";
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

describe("Instructions / proposal_create", () => {
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

  const createProposalV2 = async ({
    consensusAccount,
    transactionIndex,
    proposer,
    proposerSigner,
    draft,
    memo,
    includeProposerAsRemaining = true,
  }: {
    consensusAccount: PublicKey;
    transactionIndex: bigint;
    proposer: PublicKey;
    proposerSigner?: Keypair;
    draft: boolean;
    memo?: string | null;
    includeProposerAsRemaining?: boolean;
  }) => {
    const [proposalPda] = smartAccount.getProposalPda({
      settingsPda: consensusAccount,
      transactionIndex,
      programId,
    });
    const ix = smartAccount.generated.createCreateProposalV2Instruction(
      {
        consensusAccount,
        proposal: proposalPda,
        rentPayer: members.proposer.publicKey,
        program: programId,
        anchorRemainingAccounts: includeProposerAsRemaining
          ? [
              {
                pubkey: proposer,
                isSigner: true,
                isWritable: false,
              },
            ]
          : [],
      },
      {
        args: {
          transactionIndex: new BN(transactionIndex.toString()),
          draft,
          proposerKey: proposer,
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
      signers: proposerSigner ? [proposerSigner] : [],
    });

    return proposalPda;
  };

  // Golden Path Tests (V1 + V2 parity)
  it("should_create_proposal_active_v1", async () => {
    const settingsPda = await createSettings();
    const transactionIndex = 1n;
    await createSettingsTransaction(settingsPda, transactionIndex);

    const signature = await smartAccount.rpc.createProposal({
      connection,
      feePayer: members.proposer,
      settingsPda,
      transactionIndex,
      creator: members.proposer,
      programId,
    });
    await connection.confirmTransaction(signature);

    const [proposalPda, proposalBump] = smartAccount.getProposalPda({
      settingsPda,
      transactionIndex,
      programId,
    });
    const proposalAccount = await Proposal.fromAccountAddress(
      connection,
      proposalPda
    );

    assert.strictEqual(
      proposalAccount.settings.toBase58(),
      settingsPda.toBase58()
    );
    assert.strictEqual(
      proposalAccount.transactionIndex.toString(),
      transactionIndex.toString()
    );
    assert.ok(
      smartAccount.types.isProposalStatusActive(proposalAccount.status)
    );
    assert.strictEqual(proposalAccount.bump, proposalBump);
  });

  it("should_create_proposal_active_v2", async () => {
    const settingsPda = await createSettings();
    const transactionIndex = 1n;
    await createSettingsTransaction(settingsPda, transactionIndex);

    const proposalPda = await createProposalV2({
      consensusAccount: settingsPda,
      transactionIndex,
      proposer: members.proposer.publicKey,
      proposerSigner: members.proposer,
      draft: false,
    });

    const proposalAccount = await Proposal.fromAccountAddress(
      connection,
      proposalPda
    );
    assert.strictEqual(
      proposalAccount.settings.toBase58(),
      settingsPda.toBase58()
    );
    assert.strictEqual(
      proposalAccount.transactionIndex.toString(),
      transactionIndex.toString()
    );
    assert.ok(
      smartAccount.types.isProposalStatusActive(proposalAccount.status)
    );
  });

  it("should_create_proposal_draft_v1", async () => {
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

    const [proposalPda] = smartAccount.getProposalPda({
      settingsPda,
      transactionIndex,
      programId,
    });
    const proposalAccount = await Proposal.fromAccountAddress(
      connection,
      proposalPda
    );

    assert.strictEqual(proposalAccount.status.__kind, "Draft");
  });

  it("should_create_proposal_draft_v2", async () => {
    const settingsPda = await createSettings();
    const transactionIndex = 1n;
    await createSettingsTransaction(settingsPda, transactionIndex);

    const proposalPda = await createProposalV2({
      consensusAccount: settingsPda,
      transactionIndex,
      proposer: members.proposer.publicKey,
      proposerSigner: members.proposer,
      draft: true,
    });

    const proposalAccount = await Proposal.fromAccountAddress(
      connection,
      proposalPda
    );
    assert.strictEqual(proposalAccount.status.__kind, "Draft");
  });

  skip.it("should_emit_create_event_v1");
  skip.it("should_emit_create_event_v2");

  it("should_initialize_vote_vectors_v1", async () => {
    const settingsPda = await createSettings();
    const transactionIndex = 1n;
    await createSettingsTransaction(settingsPda, transactionIndex);

    const signature = await smartAccount.rpc.createProposal({
      connection,
      feePayer: members.proposer,
      settingsPda,
      transactionIndex,
      creator: members.proposer,
      programId,
    });
    await connection.confirmTransaction(signature);

    const [proposalPda] = smartAccount.getProposalPda({
      settingsPda,
      transactionIndex,
      programId,
    });
    const proposalAccount = await Proposal.fromAccountAddress(
      connection,
      proposalPda
    );

    assert.deepEqual(proposalAccount.approved, []);
    assert.deepEqual(proposalAccount.rejected, []);
    assert.deepEqual(proposalAccount.cancelled, []);
  });

  it("should_initialize_vote_vectors_v2", async () => {
    const settingsPda = await createSettings();
    const transactionIndex = 1n;
    await createSettingsTransaction(settingsPda, transactionIndex);

    const proposalPda = await createProposalV2({
      consensusAccount: settingsPda,
      transactionIndex,
      proposer: members.proposer.publicKey,
      proposerSigner: members.proposer,
      draft: false,
    });

    const proposalAccount = await Proposal.fromAccountAddress(
      connection,
      proposalPda
    );
    assert.deepEqual(proposalAccount.approved, []);
    assert.deepEqual(proposalAccount.rejected, []);
    assert.deepEqual(proposalAccount.cancelled, []);
  });

  // Invariant & Safety Tests (V1 + V2 parity)
  skip.it("should_fail_on_invalid_consensus_account_derivation_v1");
  skip.it("should_fail_on_invalid_consensus_account_derivation_v2");
  skip.it("should_fail_when_consensus_account_inactive_v1");
  skip.it("should_fail_when_consensus_account_inactive_v2");

  it("should_fail_on_invalid_transaction_index_gt_current_v1", async () => {
    const settingsPda = await createSettings();

    await assert.rejects(
      () =>
        smartAccount.rpc.createProposal({
          connection,
          feePayer: members.proposer,
          settingsPda,
          transactionIndex: 1n,
          creator: members.proposer,
          programId,
        }),
      /InvalidTransactionIndex|Invalid transaction index/
    );
  });

  it("should_fail_on_invalid_transaction_index_gt_current_v2", async () => {
    const settingsPda = await createSettings();

    await assert.rejects(
      () =>
        createProposalV2({
          consensusAccount: settingsPda,
          transactionIndex: 1n,
          proposer: members.proposer.publicKey,
          proposerSigner: members.proposer,
          draft: false,
        }),
      /InvalidTransactionIndex|Invalid transaction index/
    );
  });

  skip.it("should_fail_on_stale_transaction_index_v1");
  skip.it("should_fail_on_stale_transaction_index_v2");

  it("should_fail_on_not_a_signer_v1", async () => {
    const settingsPda = await createSettings();
    const transactionIndex = 1n;
    await createSettingsTransaction(settingsPda, transactionIndex);
    const nonMember = await generateFundedKeypair(connection);

    await assert.rejects(
      () =>
        smartAccount.rpc.createProposal({
          connection,
          feePayer: nonMember,
          settingsPda,
          transactionIndex,
          creator: nonMember,
          programId,
        }),
      /NotASigner|Provided pubkey is not a signer of the smart account/
    );
  });

  it("should_fail_on_not_a_signer_v2", async () => {
    const settingsPda = await createSettings();
    const transactionIndex = 1n;
    await createSettingsTransaction(settingsPda, transactionIndex);
    const nonMember = await generateFundedKeypair(connection);

    await assert.rejects(
      () =>
        createProposalV2({
          consensusAccount: settingsPda,
          transactionIndex,
          proposer: nonMember.publicKey,
          proposerSigner: nonMember,
          draft: false,
        }),
      /NotASigner|Provided pubkey is not a signer of the smart account/
    );
  });

  it("should_fail_on_missing_initiate_and_vote_permissions_v1", async () => {
    const settingsPda = await createSettings();
    const transactionIndex = 1n;
    await createSettingsTransaction(settingsPda, transactionIndex);

    await assert.rejects(
      () =>
        smartAccount.rpc.createProposal({
          connection,
          feePayer: members.executor,
          settingsPda,
          transactionIndex,
          creator: members.executor,
          programId,
        }),
      /Unauthorized|Attempted to perform an unauthorized action/
    );
  });

  it("should_fail_on_missing_initiate_and_vote_permissions_v2", async () => {
    const settingsPda = await createSettings();
    const transactionIndex = 1n;
    await createSettingsTransaction(settingsPda, transactionIndex);

    await assert.rejects(
      () =>
        createProposalV2({
          consensusAccount: settingsPda,
          transactionIndex,
          proposer: members.executor.publicKey,
          proposerSigner: members.executor,
          draft: false,
        }),
      /Unauthorized|Attempted to perform an unauthorized action/
    );
  });

  it("should_fail_on_invalid_v2_context_signature_v2", async () => {
    const settingsPda = await createSettings();
    const transactionIndex = 1n;
    await createSettingsTransaction(settingsPda, transactionIndex);

    await assert.rejects(
      () =>
        createProposalV2({
          consensusAccount: settingsPda,
          transactionIndex,
          proposer: members.proposer.publicKey,
          proposerSigner: members.proposer,
          draft: false,
          includeProposerAsRemaining: false,
        }),
      /Missing signature|MissingSignature/
    );
  });

  skip.it("should_fail_on_invalid_webauthn_client_data_v2");

  // Edge Case Tests (V1 + V2 parity)
  skip.it("should_create_with_min_transaction_index_v1");
  skip.it("should_create_with_min_transaction_index_v2");
  skip.it("should_create_with_max_transaction_index_v1");
  skip.it("should_create_with_max_transaction_index_v2");

  it("should_reject_duplicate_proposal_pda_v1", async () => {
    const settingsPda = await createSettings();
    const transactionIndex = 1n;
    await createSettingsTransaction(settingsPda, transactionIndex);

    const signature = await smartAccount.rpc.createProposal({
      connection,
      feePayer: members.proposer,
      settingsPda,
      transactionIndex,
      creator: members.proposer,
      programId,
    });
    await connection.confirmTransaction(signature);

    await assert.rejects(() =>
      smartAccount.rpc.createProposal({
        connection,
        feePayer: members.proposer,
        settingsPda,
        transactionIndex,
        creator: members.proposer,
        programId,
      })
    );
  });

  it("should_reject_duplicate_proposal_pda_v2", async () => {
    const settingsPda = await createSettings();
    const transactionIndex = 1n;
    await createSettingsTransaction(settingsPda, transactionIndex);

    await createProposalV2({
      consensusAccount: settingsPda,
      transactionIndex,
      proposer: members.proposer.publicKey,
      proposerSigner: members.proposer,
      draft: false,
    });

    await assert.rejects(() =>
      createProposalV2({
        consensusAccount: settingsPda,
        transactionIndex,
        proposer: members.proposer.publicKey,
        proposerSigner: members.proposer,
        draft: false,
      })
    );
  });

  skip.it("should_allow_empty_memo_v1");

  it("should_allow_empty_memo_v2", async () => {
    const settingsPda = await createSettings();
    const transactionIndex = 1n;
    await createSettingsTransaction(settingsPda, transactionIndex);

    const proposalPda = await createProposalV2({
      consensusAccount: settingsPda,
      transactionIndex,
      proposer: members.proposer.publicKey,
      proposerSigner: members.proposer,
      draft: false,
      memo: "",
    });

    const proposalAccount = await Proposal.fromAccountAddress(
      connection,
      proposalPda
    );
    assert.strictEqual(
      proposalAccount.transactionIndex.toString(),
      transactionIndex.toString()
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
