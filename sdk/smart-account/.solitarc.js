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
  "SmartAccountEvent",
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
    return {
      ...idl,
      types: idl.types.filter((type) => {
        return !ignoredTypes.has(type.name);
      }),
    };
  },
};
