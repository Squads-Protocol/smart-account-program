import * as smartAccount from "@sqds/smart-account";
import {
  createLocalhostConnection,
  getTestProgramId,
} from "../../utils";

const programId = getTestProgramId();
const connection = createLocalhostConnection();

describe("Instructions / transaction_execute_sync_legacy", () => {
  // Legacy sync handler uses LegacySyncTransactionArgs with SmallVec<u8, CompiledInstruction>
  // instruction encoding and a different remaining accounts layout than the current sync handler.
  // No SDK wrapper exists for this instruction — requires manual instruction construction.
  it("execute a legacy synchronous transfer from vault");
});
