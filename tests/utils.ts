// Re-export facade — all implementations live in tests/helpers/.

// Versioned helpers (parameterized V1/V2 tests)
export { SIGNER_FORMAT, formatsToRun, getRpc } from "./helpers/versioned";

// Signer format helpers
export {
  createSignerObject,
  createSignerArray,
  createSignerWrapper,
  getSignerKey,
  unwrapSigners,
  wrapSigners,
} from "./helpers/signers";

// Connection / config helpers
export {
  getTestProgramId,
  getTestProgramConfigInitializer,
  getProgramConfigInitializer,
  getTestProgramConfigAuthority,
  getTestProgramTreasury,
  getTestAccountCreationAuthority,
  createLocalhostConnection,
  getLogs,
  getNextAccountIndex,
} from "./helpers/connection";

// Account creation helpers
export {
  type TestMembers,
  generateFundedKeypair,
  fundKeypair,
  generateSmartAccountSigners,
  createAutonomousMultisig,
  createAutonomousSmartAccountV2,
  createControlledSmartAccount,
  createControlledMultisigV2,
  type MultisigWithRentReclamationAndVariousBatches,
  createAutonomousMultisigWithRentReclamationAndVariousBatches,
  createTestTransferInstruction,
  processBufferInChunks,
  createMintAndTransferTo,
} from "./helpers/accounts";

// Assertion helpers
export {
  isCloseToNow,
  range,
  comparePubkeys,
  extractTransactionPayloadDetails,
} from "./helpers/assertions";

// External signer crypto utilities
export {
  type Ed25519ExternalKeypair,
  generateEd25519ExternalKeypair,
  signEd25519External,
  type Secp256k1Keypair,
  generateSecp256k1Keypair,
  signSecp256k1,
  type P256WebauthnKeypair,
  generateP256Keypair,
  signP256,
  buildEd25519PrecompileInstruction,
  buildSecp256k1PrecompileInstruction,
  buildSecp256r1PrecompileInstruction,
  buildSecp256r1MultiSigPrecompileInstruction,
  base64urlEncode,
  buildVoteMessage,
} from "./helpers/crypto";
