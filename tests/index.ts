// The order of imports is the order the test suite will run in.
import "./suites/program-config-init";

// Parameterized tests — run for both V1 and V2 signer formats
import "./suites/instructions/batchAccountsClose";
import "./suites/instructions/cancelRealloc";
import "./suites/instructions/settingsTransactionAccountsClose";
import "./suites/instructions/settingsTransactionExecute";
import "./suites/instructions/settingsTransactionSynchronous";
import "./suites/instructions/smartAccountCreate";
import "./suites/instructions/smartAccountSetArchivalAuthority";
import "./suites/instructions/transactionBufferClose";
import "./suites/instructions/transactionBufferCreate";
import "./suites/instructions/transactionBufferExtend";
import "./suites/instructions/batchTransactionAccountClose";
import "./suites/instructions/transactionAccountsClose";
import "./suites/instructions/transactionCreateFromBuffer";
import "./suites/instructions/transactionSynchronous";
import "./suites/instructions/incrementAccountIndex";
import "./suites/instructions/logEvent";
import "./suites/instructions/policyCreation";
import "./suites/instructions/policyUpdate";
import "./suites/instructions/removePolicy";
import "./suites/instructions/policyExpiration";
import "./suites/instructions/settingsChangePolicy";
import "./suites/instructions/programInteractionPolicy";
import "./suites/instructions/spendingLimitPolicy";
import "./suites/instructions/internalFundTransferPolicy";

// Split from smart-account-sdk.ts — per-instruction tests
import "./suites/instructions/proposalCreate";
import "./suites/instructions/proposalApprove";
import "./suites/instructions/proposalReject";
import "./suites/instructions/proposalCancel";
import "./suites/instructions/activateProposal";
import "./suites/instructions/transactionCreate";
import "./suites/instructions/transactionExecute";
import "./suites/instructions/transactionClose";
import "./suites/instructions/transactionExecuteSyncLegacy";
import "./suites/instructions/settingsTransactionCreate";
import "./suites/instructions/authorityAddSigner";
import "./suites/instructions/authorityRemoveSigner";
import "./suites/instructions/authorityChangeThreshold";
import "./suites/instructions/authoritySetTimeLock";
import "./suites/instructions/authoritySetSettingsAuthority";
import "./suites/instructions/authorityAddSpendingLimit";
import "./suites/instructions/authorityRemoveSpendingLimit";
import "./suites/instructions/authoritySettingsTransactionExecute";
import "./suites/instructions/useSpendingLimit";
import "./suites/instructions/batchCreate";
import "./suites/instructions/batchAddTransaction";
import "./suites/instructions/batchExecuteTransaction";
import "./suites/instructions/sdkUtils";

// V2-only tests — use extraVerificationData and V2 instruction handlers
import "./suites/instructions/externalSignerTypes";
import "./suites/instructions/externalSignerSyscall";
import "./suites/instructions/externalSignerPrecompile";
import "./suites/instructions/sessionKeys";

// V2 instruction handler tests (dedicated V2 create/approve/execute flows)
import "./suites/v2/instructions/smartAccountCreate";
import "./suites/v2/instructions/smartAccountSetArchivalAuthority";
import "./suites/v2/instructions/incrementAccountIndex";
import "./suites/v2/instructions/logEvent";
import "./suites/v2/instructions/transactionSynchronous";
import "./suites/v2/instructions/mixedSignerSync";
import "./suites/v2/instructions/externalSignerSecurity";
import "./suites/v2/instructions/externalSignerNoncePersistence";
import "./suites/v2/instructions/settingsTransactionExecute";
import "./suites/v2/instructions/settingsTransactionSynchronous";
import "./suites/v2/instructions/settingsTransactionAccountsClose";
import "./suites/v2/instructions/transactionAccountsClose";
import "./suites/v2/instructions/transactionBufferCreate";
import "./suites/v2/instructions/transactionBufferExtend";
import "./suites/v2/instructions/transactionBufferClose";
import "./suites/v2/instructions/transactionCreateFromBuffer";
import "./suites/v2/instructions/batchAccountsClose";
import "./suites/v2/instructions/batchTransactionAccountClose";
import "./suites/v2/instructions/cancelRealloc";
import "./suites/v2/instructions/policyCreation";
import "./suites/v2/instructions/policyUpdate";
import "./suites/v2/instructions/removePolicy";
import "./suites/v2/instructions/policyExpiration";
import "./suites/v2/instructions/settingsChangePolicy";
import "./suites/v2/instructions/programInteractionPolicy";
import "./suites/v2/instructions/spendingLimitPolicy";
import "./suites/v2/instructions/internalFundTransferPolicy";
