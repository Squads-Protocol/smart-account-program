import * as smartAccount from "@sqds/smart-account";

// Signer format from env
export const SIGNER_FORMAT = process.env.SIGNER_FORMAT || 'v2';

// Determine which formats to run tests for
export const formatsToRun: string[] = process.env.SIGNER_FORMAT
  ? [process.env.SIGNER_FORMAT]
  : ['v1', 'v2'];

/**
 * Returns the appropriate RPC function set based on signer format.
 * V1 format uses base RPC functions, V2 uses V2 variants.
 */
export function getRpc(format: string) {
  const isV2 = format === 'v2';
  return {
    createSmartAccount: smartAccount.rpc.createSmartAccount,
    createSettingsTransaction: isV2
      ? smartAccount.rpc.createSettingsTransactionV2
      : smartAccount.rpc.createSettingsTransaction,
    createProposal: isV2
      ? smartAccount.rpc.createProposalV2
      : smartAccount.rpc.createProposal,
    approveProposal: isV2
      ? smartAccount.rpc.approveProposalV2
      : smartAccount.rpc.approveProposal,
    rejectProposal: isV2
      ? smartAccount.rpc.rejectProposalV2
      : smartAccount.rpc.rejectProposal,
    cancelProposal: isV2
      ? smartAccount.rpc.cancelProposalV2
      : smartAccount.rpc.cancelProposal,
    activateProposal: isV2
      ? smartAccount.rpc.activateProposalV2
      : smartAccount.rpc.activateProposal,
    executeSettingsTransaction: isV2
      ? smartAccount.rpc.executeSettingsTransactionV2
      : smartAccount.rpc.executeSettingsTransaction,
    executeSettingsTransactionSync: isV2
      ? smartAccount.rpc.executeSettingsTransactionSyncV2
      : smartAccount.rpc.executeSettingsTransactionSync,
    // NOTE: V2 sync uses V1 executeTransactionSyncV2 due to breaking change in remaining accounts layout
    // There is no V1 executeTransactionSync exported from SDK — only V2 exists
    executeTransactionSync: smartAccount.rpc.executeTransactionSyncV2,
    executeTransaction: isV2
      ? smartAccount.rpc.executeTransactionV2
      : smartAccount.rpc.executeTransaction,
    createTransaction: isV2
      ? smartAccount.rpc.createTransactionV2
      : smartAccount.rpc.createTransaction,
    createBatch: isV2
      ? smartAccount.rpc.createBatchV2
      : smartAccount.rpc.createBatch,
    addTransactionToBatch: isV2
      ? smartAccount.rpc.addTransactionToBatchV2
      : smartAccount.rpc.addTransactionToBatch,
    executeBatchTransaction: isV2
      ? smartAccount.rpc.executeBatchTransactionV2
      : smartAccount.rpc.executeBatchTransaction,
    closeBatch: smartAccount.rpc.closeBatch,
    closeBatchTransaction: smartAccount.rpc.closeBatchTransaction,
    // Buffer operations only have V2 variants in the SDK
    createTransactionBuffer: smartAccount.rpc.createTransactionBufferV2,
    extendTransactionBuffer: smartAccount.rpc.extendTransactionBufferV2,
    createTransactionFromBuffer: smartAccount.rpc.createTransactionFromBufferV2,
    // Policy operations (no V2 variants exist)
    createPolicyTransaction: smartAccount.rpc.createPolicyTransaction,
    executePolicyTransaction: smartAccount.rpc.executePolicyTransaction,
    executePolicyPayloadSync: smartAccount.rpc.executePolicyPayloadSync,
    closeEmptyPolicyTransaction: smartAccount.rpc.closeEmptyPolicyTransaction,
    // Close operations (no V2 variants)
    closeSettingsTransaction: smartAccount.rpc.closeSettingsTransaction,
    closeTransaction: smartAccount.rpc.closeTransaction,
    // Authority operations (no V2 variants — controlled account only)
    addSignerAsAuthority: smartAccount.rpc.addSignerAsAuthority,
    removeSignerAsAuthority: smartAccount.rpc.removeSignerAsAuthority,
    setTimeLockAsAuthority: smartAccount.rpc.setTimeLockAsAuthority,
    setNewSettingsAuthorityAsAuthority: smartAccount.rpc.setNewSettingsAuthorityAsAuthority,
    setArchivalAuthorityAsAuthority: smartAccount.rpc.setArchivalAuthorityAsAuthority,
    addSpendingLimitAsAuthority: smartAccount.rpc.addSpendingLimitAsAuthority,
    removeSpendingLimitAsAuthority: smartAccount.rpc.removeSpendingLimitAsAuthority,
  };
}
