#!/usr/bin/env node

const fs = require("fs");
const path = require("path");

const GENERATED_INSTRUCTIONS_DIR = path.join(
  __dirname,
  "..",
  "src",
  "generated",
  "instructions"
);

const SYSVAR_SNIPPET = `  keys.push({
    pubkey: web3.SYSVAR_INSTRUCTIONS_PUBKEY,
    isWritable: false,
    isSigner: false,
  })

`;

function processFile(filePath) {
  if (!fs.existsSync(filePath)) {
    return false;
  }

  const originalContent = fs.readFileSync(filePath, "utf8");
  if (originalContent.includes("web3.SYSVAR_INSTRUCTIONS_PUBKEY")) {
    return false;
  }

  const updatedContent = originalContent.replace(
    /\n\n  const ix = new web3\.TransactionInstruction\(/,
    `\n\n${SYSVAR_SNIPPET}  const ix = new web3.TransactionInstruction(`
  );

  if (updatedContent === originalContent) {
    return false;
  }

  fs.writeFileSync(filePath, updatedContent);
  return true;
}

function main() {
  let filesPatched = 0;

  for (const fileName of fs.readdirSync(GENERATED_INSTRUCTIONS_DIR)) {
    if (!fileName.endsWith(".ts")) {
      continue;
    }

    const filePath = path.join(GENERATED_INSTRUCTIONS_DIR, fileName);
    if (processFile(filePath)) {
      filesPatched += 1;
      console.log(`Patched ${fileName}`);
    }
  }

  console.log(`Done! Patched ${filesPatched} generated instruction file(s).`);
}

main();
