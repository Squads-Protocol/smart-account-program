// V2-only tests: runs parameterized tests with V2 signer format + V2-exclusive tests.
// Used by: yarn testV2 (requires running validator)
process.env.SIGNER_FORMAT = "v2";

// Parameterized tests (will run V2 format only)
import "./suites/program-config-init";
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
import "./suites/instructions/externalSignerSyscall";
import "./suites/instructions/externalSignerPrecompile";
import "./suites/instructions/spendingLimitPolicy";
import "./suites/instructions/internalFundTransferPolicy";
import "./suites/instructions/authorityAddSigner";
import "./suites/instructions/authorityRemoveSigner";
import "./suites/instructions/authorityChangeThreshold";
import "./suites/instructions/authoritySetTimeLock";
import "./suites/instructions/authoritySetSettingsAuthority";
import "./suites/instructions/authorityAddSpendingLimit";
import "./suites/instructions/authorityRemoveSpendingLimit";
import "./suites/instructions/settingsTransactionCreate";
import "./suites/instructions/transactionCreate";
import "./suites/instructions/transactionExecute";
import "./suites/instructions/proposalCreate";
import "./suites/instructions/proposalApprove";
import "./suites/instructions/proposalReject";
import "./suites/instructions/proposalCancel";
import "./suites/instructions/batchCreate";
import "./suites/instructions/sdkUtils";

// V2-only tests (unique behavior, not parameterized)
import "./suites/v2/instructions/mixedSignerSync";
