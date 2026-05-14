import { renderVisitor as renderJavaScriptVisitor } from "@codama/renderers-js";
import { renderVisitor as renderRustVisitor } from "@codama/renderers-rust";
import { rootNodeFromAnchor } from "@codama/nodes-from-anchor";
import {
  addPdasVisitor,
  createFromRoot,
  updateDefinedTypesVisitor,
} from "codama";
import fs from "node:fs";
import path from "node:path";

import { ignoredDefinedTypes } from "./transforms/filter-types";
import { smartAccountPdas } from "./transforms/pdas";
import {
  rewriteSmallVecsInIdl,
  smallVecTypeOverrides,
} from "./transforms/small-vec";

const repoRoot = path.resolve(__dirname, "..");
const idlPath = path.join(
  repoRoot,
  "idl",
  "squads_smart_account_program.json"
);
const tsClientDir = path.join(repoRoot, "sdk", "ts");
const rustClientDir = path.join(repoRoot, "sdk", "rust");

// Program name as it appears once codama camelCases the Anchor IDL name
// `squads_smart_account_program`.
const programName = "squadsSmartAccountProgram";

// The program ID is not embedded in the Anchor 0.29-format IDL JSON.  Source
// of truth is Anchor.toml + the published deployment.
const programId = "SMRTzfY6DfH5ik3TKiyLFfXexV8uSG3d2UksSCYdunG";

async function main() {
  const rawIdl = JSON.parse(fs.readFileSync(idlPath, "utf8"));
  rawIdl.metadata = { ...(rawIdl.metadata ?? {}), address: programId };
  const { idl, bindings } = rewriteSmallVecsInIdl(rawIdl);

  const codama = createFromRoot(rootNodeFromAnchor(idl));

  // Replace each `SmallVec<N, T>` placeholder defined-type with a proper
  // N-prefixed array — fixes the long-standing Solita encoding bug.
  codama.update(updateDefinedTypesVisitor(smallVecTypeOverrides(bindings)));

  // Drop defined types that reference undefined siblings or have known
  // IDL-representation issues (see filter-types.ts for the list and rationale).
  codama.update(
    updateDefinedTypesVisitor(
      Object.fromEntries(
        Array.from(ignoredDefinedTypes).map((name) => [
          name,
          { delete: true } as const,
        ])
      )
    )
  );

  // Inject PDA definitions hand-ported from sdk/smart-account/src/pda.ts.
  codama.update(addPdasVisitor({ [programName]: smartAccountPdas() }));

  const restore = preserveConfigFiles();
  try {
    await codama.accept(
      renderJavaScriptVisitor(tsClientDir, {
        formatCode: true,
        deleteFolderBeforeRendering: true,
      })
    );

    codama.accept(
      renderRustVisitor(rustClientDir, {
        crateFolder: rustClientDir,
        formatCode: true,
        deleteFolderBeforeRendering: true,
      })
    );
  } finally {
    restore();
  }

  console.log("SDK generation complete.");
  console.log(`  TypeScript: ${path.relative(repoRoot, tsClientDir)}/src/`);
  console.log(`  Rust:       ${path.relative(repoRoot, rustClientDir)}/src/`);
}

/**
 * Codama renderers honour `deleteFolderBeforeRendering` by removing the
 * package root, which would also delete the hand-curated manifests.  Snapshot
 * them to `.temp` siblings and restore after the renderers finish.
 */
function preserveConfigFiles(): () => void {
  const preserved = new Map<string, string>();
  const snapshot = (file: string) => {
    if (!fs.existsSync(file)) return;
    const temp = `${file}.codama-temp`;
    fs.copyFileSync(file, temp);
    preserved.set(file, temp);
  };

  for (const f of ["package.json", "tsconfig.json", ".npmignore"]) {
    snapshot(path.join(tsClientDir, f));
  }
  for (const f of ["Cargo.toml", "Cargo.lock"]) {
    snapshot(path.join(rustClientDir, f));
  }
  snapshot(path.join(rustClientDir, "src", "lib.rs"));

  return () => {
    for (const [original, temp] of preserved) {
      if (fs.existsSync(temp)) {
        fs.mkdirSync(path.dirname(original), { recursive: true });
        fs.copyFileSync(temp, original);
        fs.unlinkSync(temp);
      }
    }
  };
}

main().catch((err) => {
  console.error(err);
  process.exit(1);
});
