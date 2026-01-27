const { Keypair } = require("@solana/web3.js");
const { readFileSync } = require("fs");
const path = require("path");

const PROGRAM_NAME = "squads_smart_account_program";

const programDir = path.join(__dirname, "..", "..", "programs", PROGRAM_NAME);
const idlDir = path.join(__dirname, "idl");
const sdkDir = path.join(__dirname, "src", "generated");
const binaryInstallDir = path.join(__dirname, "..", "..", ".crates");

const ignoredTypes = new Set([
  // Exclude `Permission` enum from the IDL because it is not correctly represented there.
  "Permission",
  // Exclude the types that use `SmallVec` because anchor doesn't have it in the IDL.
  "TransactionMessage",
  "CompiledInstruction",
  "MessageAddressTableLookup",
  // Add event types
  "CreateSmartAccountEvent",
  "SynchronousTransactionEvent",
  "SynchronousSettingsTransactionEvent",
  "AddSpendingLimitEvent",
  "RemoveSpendingLimitEvent",
  "UseSpendingLimitEvent",
  "TransactionEvent",
  "ProposalEvent",
  "SettingsChangePolicyEvent",
  "SmartAccountEvent",
  "ConsensusAccount",
  "SynchronousTransactionEventV2",
  "AuthoritySettingsEvent",
  "AuthorityChangeEvent",
  "TransactionContent"
]);

module.exports = {
  idlGenerator: "anchor",
  programName: PROGRAM_NAME,
  programId: "SMRTzfY6DfH5ik3TKiyLFfXexV8uSG3d2UksSCYdunG",
  idlDir,
  sdkDir,
  binaryInstallDir,
  programDir,
  idlHook: (idl) => {
    // Transform the IDL to replace SmallVec types
    const transformType = (obj) => {
      if (typeof obj === "string" && obj === "SmallVec<u16,u8>") {
        return "bytes"; // Replace with bytes type
      }
      if (typeof obj === "object" && obj !== null) {
        if (obj.defined === "SmallVec<u16,u8>") {
          return "bytes"; // Replace just the type reference
        }
        if (Array.isArray(obj)) {
          return obj.map(transformType);
        }
        const transformed = {};
        for (const [key, value] of Object.entries(obj)) {
          transformed[key] = transformType(value);
        }
        return transformed;
      }
      return obj;
    };

    const transformedIdl = transformType(idl);

    return {
      ...transformedIdl,
      types: transformedIdl.types.filter((type) => {
        return !ignoredTypes.has(type.name);
      }),
    };
  },
};
