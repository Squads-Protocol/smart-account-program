#!/usr/bin/env node
/**
 * fix-signer-wrapper.js
 *
 * Fixes SmartAccountSignerWrapper serialization in solita-generated files.
 *
 * Problem: Rust SmartAccountSignerWrapper uses custom Borsh serialization:
 *          V1: [count_low, count_high, 0, 0] + LegacySmartAccountSigner[]
 *          V2: [count_low, count_high, 0, 1] + SmartAccountSigner[]
 *          Solita generates standard beet.dataEnum() which uses discriminant byte.
 *
 * Solution: Replace smartAccountSignerWrapperBeet with customSmartAccountSignerWrapperBeet
 */

const fs = require('fs');
const path = require('path');

const GENERATED_TYPES_DIR = path.join(__dirname, '..', 'src', 'generated', 'types');
const GENERATED_ACCOUNTS_DIR = path.join(__dirname, '..', 'src', 'generated', 'accounts');

function processFile(filePath) {
  if (!fs.existsSync(filePath)) {
    return false;
  }

  let content = fs.readFileSync(filePath, 'utf8');
  const originalContent = content;
  const fileName = path.basename(filePath);

  // Process SettingsAction.ts, Settings.ts, Policy.ts, CreateSmartAccountArgs.ts, and AddSignerArgs.ts
  if (fileName === 'SettingsAction.ts' || fileName === 'Settings.ts' || fileName === 'Policy.ts' || fileName === 'CreateSmartAccountArgs.ts' || fileName === 'AddSignerArgs.ts' || fileName === 'LimitedSettingsAction.ts') {
    // Replace smartAccountSignerWrapperBeet with customSmartAccountSignerWrapperBeet
    content = content.replace(
      /\['signers', smartAccountSignerWrapperBeet\]/g,
      "['signers', customSmartAccountSignerWrapperBeet]"
    );
    content = content.replace(
      /\['newSigner', smartAccountSignerWrapperBeet\]/g,
      "['newSigner', customSmartAccountSignerWrapperBeet]"
    );

    // Fix TypeScript type to accept either LegacySmartAccountSigner[] or SmartAccountSigner[]
    content = content.replace(
      /signers: SmartAccountSignerWrapper/g,
      "signers: LegacySmartAccountSigner[] | SmartAccountSigner[]"
    );
    content = content.replace(
      /newSigner: SmartAccountSignerWrapper/g,
      "newSigner: LegacySmartAccountSigner[] | SmartAccountSigner[]"
    );

    // Fix pretty() method for account files - signers is now an array
    if (fileName === 'Settings.ts' || fileName === 'Policy.ts') {
      content = content.replace(
        /signers: this\.signers\.__kind,/g,
        "signers: this.signers.map(s => ('__kind' in s ? s.__kind : 'V1')),"
      );
    }

    // Handle imports differently for type files vs account files
    if (fileName === 'SettingsAction.ts' || fileName === 'CreateSmartAccountArgs.ts' || fileName === 'AddSignerArgs.ts' || fileName === 'LimitedSettingsAction.ts') {
      // Type files: Add both LegacySmartAccountSigner and SmartAccountSigner imports
      if (content !== originalContent && !content.includes("import { LegacySmartAccountSigner")) {
        content = content.replace(
          /import \{\n  SmartAccountSignerWrapper,\n  smartAccountSignerWrapperBeet,\n\} from '\.\/SmartAccountSignerWrapper'/,
          `import { LegacySmartAccountSigner } from './LegacySmartAccountSigner'\nimport { SmartAccountSigner } from './SmartAccountSigner'\nimport {\n  SmartAccountSignerWrapper,\n  smartAccountSignerWrapperBeet,\n} from './SmartAccountSignerWrapper'`
        );
      }

      // Add custom import
      if (content !== originalContent && !content.includes("import { customSmartAccountSignerWrapperBeet }")) {
        content = content.replace(
          /} from '\.\/SmartAccountSignerWrapper'\n/,
          `} from './SmartAccountSignerWrapper'\nimport { customSmartAccountSignerWrapperBeet } from '../../types'\n`
        );
      }
    } else {
      // Settings.ts and Policy.ts: Add both LegacySmartAccountSigner and SmartAccountSigner imports
      if (content !== originalContent && !content.includes("import { LegacySmartAccountSigner")) {
        content = content.replace(
          /import \{\n  SmartAccountSignerWrapper,\n  smartAccountSignerWrapperBeet,\n\} from '\.\.\/types\/SmartAccountSignerWrapper'/,
          `import { LegacySmartAccountSigner } from '../types/LegacySmartAccountSigner'\nimport { SmartAccountSigner } from '../types/SmartAccountSigner'\nimport {\n  SmartAccountSignerWrapper,\n  smartAccountSignerWrapperBeet,\n} from '../types/SmartAccountSignerWrapper'`
        );
      }

      // Add custom import
      if (content !== originalContent && !content.includes("import { customSmartAccountSignerWrapperBeet }")) {
        content = content.replace(
          /} from '\.\.\/types\/SmartAccountSignerWrapper'\n/,
          `} from '../types/SmartAccountSignerWrapper'\nimport { customSmartAccountSignerWrapperBeet } from '../../types'\n`
        );
      }
    }

    // Write file if changes were made
    if (content !== originalContent) {
      fs.writeFileSync(filePath, content);
      console.log(`Fixed SmartAccountSignerWrapper serialization in ${fileName}`);
      return true;
    }
  }

  return false;
}

function main() {
  console.log('Post-processing solita-generated files for SmartAccountSignerWrapper...\n');

  let filesFixed = 0;

  // Process types directory
  if (fs.existsSync(GENERATED_TYPES_DIR)) {
    const files = fs.readdirSync(GENERATED_TYPES_DIR).filter(f => f.endsWith('.ts'));

    for (const file of files) {
      const filePath = path.join(GENERATED_TYPES_DIR, file);
      if (processFile(filePath)) {
        filesFixed++;
      }
    }
  }

  // Process accounts directory
  if (fs.existsSync(GENERATED_ACCOUNTS_DIR)) {
    const files = fs.readdirSync(GENERATED_ACCOUNTS_DIR).filter(f => f.endsWith('.ts'));

    for (const file of files) {
      const filePath = path.join(GENERATED_ACCOUNTS_DIR, file);
      if (processFile(filePath)) {
        filesFixed++;
      }
    }
  }

  console.log(`\nDone! Fixed ${filesFixed} file(s).`);
}

main();
