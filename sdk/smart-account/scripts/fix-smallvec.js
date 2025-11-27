#!/usr/bin/env node
/**
 * fix-smallvec.js
 *
 * Fixes SmallVec serialization in solita-generated files.
 *
 * Problem: Rust SmallVec<u8, T> uses a 1-byte length prefix, SmallVec<u16, T> uses 2 bytes.
 *          Solita generates beet.array() which always uses 4 bytes.
 *
 * Usage: Run automatically via `yarn generate` or manually with `node scripts/fix-smallvec.js`
 *
 * Adding new SmallVec types:
 *   1. If the Rust type ALWAYS uses SmallVec (never Vec), add the beet type to SMALLVEC_U8_BEET_TYPES_GLOBAL
 *      Example: InstructionConstraintCompiled is only ever used with SmallVec, so add 'instructionConstraintCompiledBeet'
 *
 *   2. If the Rust type uses SmallVec in some structs but Vec in others, add to SMALLVEC_U8_BEET_TYPES_FILE_SPECIFIC
 *      Example: DataConstraint uses Vec in InstructionConstraint (legacy) but SmallVec in InstructionConstraintCompiled,
 *               so we list only the files where it should be SmallVec
 *
 *   3. For SmallVec<u8, u8> or SmallVec<u16, u8> fields (byte arrays), add file-specific handling below
 */

const fs = require('fs');
const path = require('path');

const GENERATED_DIR = path.join(__dirname, '..', 'src', 'generated', 'types');

// Beet types that ALWAYS use SmallVec<u8, X> (never Vec)
const SMALLVEC_U8_BEET_TYPES_GLOBAL = [
  'instructionConstraintCompiledBeet',
  'accountConstraintCompiledBeet',
  'limitedSpendingLimitCompiledBeet',
  'compiledInstructionBeet',
  'messageAddressTableLookupBeet',
];

// Beet types that use SmallVec only in specific files (Vec elsewhere)
const SMALLVEC_U8_BEET_TYPES_FILE_SPECIFIC = {
  'dataConstraintBeet': [
    'InstructionConstraintCompiled.ts',
    'AccountConstraintTypeCompiled.ts',
  ],
  'beetSolana.publicKey': [
    'SmartAccountTransactionMessage.ts',
    'ProgramInteractionPolicyCreationPayload.ts',
  ],
};

function processFile(filePath) {
  if (!fs.existsSync(filePath)) {
    return false;
  }

  let content = fs.readFileSync(filePath, 'utf8');
  const originalContent = content;
  const fileName = path.basename(filePath);

  // Global replacements
  for (const beetType of SMALLVEC_U8_BEET_TYPES_GLOBAL) {
    const regex = new RegExp(`beet\\.array\\(${beetType.replace('.', '\\.')}\\)`, 'g');
    content = content.replace(regex, `smallArray(beet.u8, ${beetType})`);
  }

  // File-specific replacements
  for (const [beetType, files] of Object.entries(SMALLVEC_U8_BEET_TYPES_FILE_SPECIFIC)) {
    if (files.includes(fileName)) {
      const regex = new RegExp(`beet\\.array\\(${beetType.replace('.', '\\.')}\\)`, 'g');
      content = content.replace(regex, `smallArray(beet.u8, ${beetType})`);
    }
  }

  // File-specific fixes for byte array fields (SmallVec<u8/u16, u8> -> beet.bytes in IDL)
  if (fileName === 'CompiledInstruction.ts') {
    // accountIndexes: SmallVec<u8, u8>
    content = content.replace(/\['accountIndexes', beet\.bytes\]/g, "['accountIndexes', smallArray(beet.u8, beet.u8)]");
    // data: SmallVec<u16, u8> (note: u16 length prefix)
    content = content.replace(/\['data', beet\.bytes\]/g, "['data', smallArray(beet.u16, beet.u8)]");
    content = content.replace(/accountIndexes: Uint8Array/g, 'accountIndexes: number[]');
    content = content.replace(/data: Uint8Array/g, 'data: number[]');
  }

  if (fileName === 'MessageAddressTableLookup.ts') {
    content = content.replace(/\['writableIndexes', beet\.bytes\]/g, "['writableIndexes', smallArray(beet.u8, beet.u8)]");
    content = content.replace(/\['readonlyIndexes', beet\.bytes\]/g, "['readonlyIndexes', smallArray(beet.u8, beet.u8)]");
    content = content.replace(/writableIndexes: Uint8Array/g, 'writableIndexes: number[]');
    content = content.replace(/readonlyIndexes: Uint8Array/g, 'readonlyIndexes: number[]');
  }

  if (fileName === 'HookCompiled.ts') {
    content = content.replace(/\['instructionData', beet\.bytes\]/g, "['instructionData', smallArray(beet.u8, beet.u8)]");
    content = content.replace(/instructionData: Uint8Array/g, 'instructionData: number[]');
  }

  if (fileName === 'AccountConstraintTypeCompiled.ts') {
    // Pubkey variant uses tuple([bytes]) for SmallVec<u8, u8>
    content = content.replace(/beet\.tuple\(\[beet\.bytes\]\)/g, 'beet.tuple([smallArray(beet.u8, beet.u8)])');
    content = content.replace(/Pubkey: \{ fields: \[Uint8Array\] \}/g, 'Pubkey: { fields: [number[]] }');
  }

  // Add smallArray import if file was modified
  if (content !== originalContent) {
    if (!content.includes("import { smallArray }") && !content.includes("{ smallArray }")) {
      content = content.replace(
        /import \* as beet from '@metaplex-foundation\/beet'/,
        `import * as beet from '@metaplex-foundation/beet'\nimport { smallArray } from '../../types'`
      );
    }

    fs.writeFileSync(filePath, content);
    console.log(`Fixed SmallVec serialization in ${fileName}`);
    return true;
  }

  return false;
}

function main() {
  console.log('Post-processing solita-generated files for SmallVec...\n');

  let filesFixed = 0;

  if (fs.existsSync(GENERATED_DIR)) {
    const files = fs.readdirSync(GENERATED_DIR).filter(f => f.endsWith('.ts'));

    for (const file of files) {
      const filePath = path.join(GENERATED_DIR, file);
      if (processFile(filePath)) {
        filesFixed++;
      }
    }
  }

  console.log(`\nDone! Fixed ${filesFixed} file(s).`);
}

main();
