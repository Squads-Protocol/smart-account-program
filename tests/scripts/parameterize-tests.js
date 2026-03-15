#!/usr/bin/env node
/**
 * Parameterize V1 test files to run for both V1 and V2 signer formats.
 *
 * Transforms:
 *   describe("Instructions / foo", () => { ... })
 * Into:
 *   for (const format of formatsToRun) {
 *     const rpc = getRpc(format);
 *     describe(`Instructions / foo [${format}]`, () => { ... });
 *   }
 *
 * And replaces:
 *   smartAccount.rpc.XXX(  →  rpc.XXX(
 */

const fs = require('fs');
const path = require('path');

const TESTS_DIR = path.join(__dirname, '..', 'suites', 'instructions');

// Files to parameterize (V1 files that have V2 counterparts)
const FILES = [
  'batchAccountsClose.ts',
  'batchTransactionAccountClose.ts',
  'cancelRealloc.ts',
  'incrementAccountIndex.ts',
  'internalFundTransferPolicy.ts',
  'logEvent.ts',
  'policyCreation.ts',
  'policyExpiration.ts',
  'policyUpdate.ts',
  'programInteractionPolicy.ts',
  'removePolicy.ts',
  'settingsChangePolicy.ts',
  'settingsTransactionAccountsClose.ts',
  'settingsTransactionExecute.ts',
  'settingsTransactionSynchronous.ts',
  'smartAccountCreate.ts',
  'smartAccountSetArchivalAuthority.ts',
  'spendingLimitPolicy.ts',
  'transactionAccountsClose.ts',
  'transactionBufferClose.ts',
  'transactionBufferCreate.ts',
  'transactionBufferExtend.ts',
  'transactionCreateFromBuffer.ts',
  'transactionSynchronous.ts',
];

function parameterizeFile(filePath) {
  const fileName = path.basename(filePath);
  let content = fs.readFileSync(filePath, 'utf8');

  // Skip if already parameterized
  if (content.includes('formatsToRun')) {
    console.log(`  SKIP: ${fileName} (already parameterized)`);
    return false;
  }

  const original = content;

  // Step 1: Add formatsToRun and getRpc imports
  // Find the import from "../../utils" and add formatsToRun, getRpc
  const utilsImportRegex = /^(import\s*\{[^}]*)\}\s*from\s*["']\.\.\/\.\.\/utils["'];/m;
  const utilsMatch = content.match(utilsImportRegex);

  if (utilsMatch) {
    let importBlock = utilsMatch[1];
    // Add formatsToRun and getRpc if not already present
    const additions = [];
    if (!importBlock.includes('formatsToRun')) additions.push('formatsToRun');
    if (!importBlock.includes('getRpc')) additions.push('getRpc');

    if (additions.length > 0) {
      // Add before the closing brace
      const newImport = utilsMatch[0].replace(
        /\}\s*from/,
        `  ${additions.join(',\n  ')},\n} from`
      );
      content = content.replace(utilsMatch[0], newImport);
    }
  } else {
    // No utils import found — add one
    const firstImport = content.match(/^import /m);
    if (firstImport) {
      content = content.slice(0, firstImport.index) +
        'import {\n  formatsToRun,\n  getRpc,\n} from "../../utils";\n' +
        content.slice(firstImport.index);
    }
  }

  // Step 2: Replace smartAccount.rpc.XXX( with rpc.XXX(
  // Match smartAccount.rpc.functionName( but NOT smartAccount.rpc.functionNameV2(
  // We want to replace BOTH base and V2 calls with the base name via rpc
  content = content.replace(/smartAccount\.rpc\.(\w+?)V2\(/g, 'rpc.$1(');
  content = content.replace(/smartAccount\.rpc\.(\w+)\(/g, 'rpc.$1(');

  // Step 3: Find the top-level describe and wrap in for loop
  const describeRegex = /^(describe\(["'`])([^"'`]+)(["'`],\s*\(\)\s*=>\s*\{)/m;
  const describeMatch = content.match(describeRegex);

  if (!describeMatch) {
    console.log(`  ERROR: ${fileName} - no describe() found`);
    return false;
  }

  const describeName = describeMatch[2];

  // Replace describe line with for loop + parameterized describe
  const oldDescribeLine = describeMatch[0];
  const newDescribeLine = `for (const format of formatsToRun) {\n  const rpc = getRpc(format);\n\n  describe(\`${describeName} [\${format}]\`, () => {`;
  content = content.replace(oldDescribeLine, newDescribeLine);

  // Step 4: Find the matching closing of the describe and add the for loop closing brace
  // The describe's closing is ");\n" at the end of file (or near end)
  // We need to add "}\n" after the last ");"
  const lastCloseParen = content.lastIndexOf('});');
  if (lastCloseParen !== -1) {
    content = content.slice(0, lastCloseParen + 3) + '\n}' + content.slice(lastCloseParen + 3);
  }

  // Step 5: Indent the describe body (everything between the for loop open and close)
  // Actually, let's just add 2 spaces to all lines inside the describe block
  // This is tricky without a full parser, so we'll indent the describe and its contents
  const lines = content.split('\n');
  const forLoopStart = lines.findIndex(l => l.startsWith('for (const format'));
  const forLoopEnd = lines.length - 1; // The closing }

  if (forLoopStart !== -1) {
    // Find the describe line (should be 2 lines after for)
    const describeLineIdx = lines.findIndex((l, i) => i > forLoopStart && l.trimStart().startsWith('describe('));

    if (describeLineIdx !== -1) {
      // Indent everything from describe to end-2 by 2 spaces (inside the for loop)
      // But the describe line itself and the const rpc line are already at correct indent
      // We need to indent the body inside describe by 2 more spaces
      // Actually this is getting complex. Let's skip indentation — it's cosmetic.
    }
  }

  if (content !== original) {
    fs.writeFileSync(filePath, content);
    console.log(`  OK: ${fileName}`);
    return true;
  }

  return false;
}

function main() {
  console.log('Parameterizing V1 test files...\n');
  let count = 0;

  for (const file of FILES) {
    const filePath = path.join(TESTS_DIR, file);
    if (!fs.existsSync(filePath)) {
      console.log(`  MISSING: ${file}`);
      continue;
    }
    if (parameterizeFile(filePath)) count++;
  }

  console.log(`\nDone! Parameterized ${count} file(s).`);
}

main();
