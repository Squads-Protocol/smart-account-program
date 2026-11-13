#!/usr/bin/env node
// CI guards for @sqds/smart-account-core.
//   node scripts/ci-guards.mjs deps    # fail on any runtime "dependencies", any peer other than @solana/kit,
//                                      # or any devDependency outside ALLOWED_DEV_DEPENDENCIES
//   node scripts/ci-guards.mjs lines   # fail if hand-written src/ exceeds MAX_SRC_LINES non-blank lines
//   node scripts/ci-guards.mjs casts   # fail on `as` type assertions (other than `as const`) or `any` in src/
//                                      # outside src/encoding/, the one place byte-level casts are allowed
// Files whose first line contains "@generated" are excluded from the line budget.
import { readFile, readdir } from 'node:fs/promises';
import { dirname, join, relative } from 'node:path';
import { fileURLToPath } from 'node:url';

const ROOT = join(dirname(fileURLToPath(import.meta.url)), '..');
const MAX_SRC_LINES = 1500;
// Third-party packages are kept to what is required (Carlos, Nov 11). litesvm is required; @solana/kit is the
// peer dependency; typescript and @types/node run the strict typecheck. Adding to this list needs owner review.
const ALLOWED_DEV_DEPENDENCIES = ['@solana/kit', '@types/node', 'litesvm', 'typescript'];

async function deps() {
  const pkg = JSON.parse(await readFile(join(ROOT, 'package.json'), 'utf8'));
  const runtime = Object.keys(pkg.dependencies ?? {});
  const peers = Object.keys(pkg.peerDependencies ?? {});
  if (runtime.length) {
    console.error(`FAIL: runtime dependencies are not allowed: ${runtime.join(', ')}`);
    process.exit(1);
  }
  const unexpectedPeers = peers.filter((p) => p !== '@solana/kit');
  if (unexpectedPeers.length) {
    console.error(`FAIL: only @solana/kit may be a peer dependency, found: ${unexpectedPeers.join(', ')}`);
    process.exit(1);
  }
  const dev = Object.keys(pkg.devDependencies ?? {});
  const unexpectedDev = dev.filter((d) => !ALLOWED_DEV_DEPENDENCIES.includes(d));
  if (unexpectedDev.length) {
    console.error(`FAIL: devDependencies outside the allowlist: ${unexpectedDev.join(', ')}`);
    process.exit(1);
  }
  console.log(`OK: 0 runtime dependencies; peers: ${peers.join(', ') || '(none)'}; dev: ${dev.join(', ')}`);
}

async function* walk(dir) {
  let entries;
  try {
    entries = await readdir(dir, { withFileTypes: true });
  } catch (e) {
    if (e.code === 'ENOENT') return;
    throw e;
  }
  for (const e of entries) {
    const p = join(dir, e.name);
    if (e.isDirectory()) yield* walk(p);
    else if (/\.(ts|mts|cts|js|mjs)$/.test(e.name)) yield p;
  }
}

async function lines() {
  let total = 0;
  for await (const file of walk(join(ROOT, 'src'))) {
    const text = await readFile(file, 'utf8');
    if (text.split('\n', 1)[0].includes('@generated')) continue;
    const n = text.split('\n').filter((l) => l.trim() !== '').length;
    console.log(`${String(n).padStart(6)}  ${relative(ROOT, file)}`);
    total += n;
  }
  if (total > MAX_SRC_LINES) {
    console.error(`FAIL: ${total} hand-written src lines > budget ${MAX_SRC_LINES}`);
    process.exit(1);
  }
  console.log(`OK: ${total}/${MAX_SRC_LINES} hand-written src lines`);
}

/** Source with comments removed and string/template text blanked (template `${...}` code is kept). */
export function codeOnly(src) {
  let out = '';
  const stack = []; // open template literals: brace depth inside each `${`
  for (let i = 0; i < src.length; i++) {
    const c = src[i];
    const next = src[i + 1];
    if (c === '/' && next === '/') {
      while (i < src.length && src[i] !== '\n') i++;
      out += '\n';
    } else if (c === '/' && next === '*') {
      const end = src.indexOf('*/', i + 2);
      const body = src.slice(i, end < 0 ? src.length : end + 2);
      out += body.replace(/[^\n]/g, ' ');
      i += body.length - 1;
    } else if (c === "'" || c === '"' || c === '`' || (c === '}' && stack.length && stack[stack.length - 1] === 0)) {
      const quote = c === '}' ? '`' : c;
      if (c === '}') stack.pop();
      out += quote;
      for (i++; i < src.length && src[i] !== quote; i++) {
        if (src[i] === '\\') i++;
        else if (quote === '`' && src[i] === '$' && src[i + 1] === '{') {
          stack.push(0);
          out += '${';
          i++;
          break;
        }
      }
      if (src[i] === quote) out += quote;
    } else {
      if (stack.length && c === '{') stack[stack.length - 1]++;
      if (stack.length && c === '}') stack[stack.length - 1]--;
      out += c;
    }
  }
  return out;
}

/** Violations in one file's source: `as <type>` (not `as const`) and `any`. */
export function castViolations(src) {
  const found = [];
  // `import { a as b }` / `export { a as b }` / `export * as ns` rename bindings; they are not type assertions.
  const renames = /\b(?:import|export)\s+(?:type\s+)?(?:\{[^}]*\}|\*\s+as\s+\w+)/g;
  codeOnly(src)
    .replace(renames, (m) => m.replace(/[^\n]/g, ' '))
    .split('\n')
    .forEach((line, i) => {
      for (const m of line.matchAll(/\bas\s+(?!const\b)[\w{[(<'"`]/g)) found.push(`${i + 1}: ${m[0]}`);
      for (const m of line.matchAll(/\bany\b/g)) found.push(`${i + 1}: ${m[0]}`);
    });
  return found;
}

async function casts() {
  const bad = [];
  for await (const file of walk(join(ROOT, 'src'))) {
    const rel = relative(ROOT, file);
    if (rel.startsWith(join('src', 'encoding'))) continue;
    for (const v of castViolations(await readFile(file, 'utf8'))) bad.push(`${rel}:${v}`);
  }
  if (bad.length) {
    console.error(`FAIL: type assertions or \`any\` outside src/encoding/:\n  ${bad.join('\n  ')}`);
    process.exit(1);
  }
  console.log('OK: no `as` casts (other than `as const`) or `any` in src/ outside src/encoding/');
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  const cmd = process.argv[2];
  if (cmd === 'deps') await deps();
  else if (cmd === 'lines') await lines();
  else if (cmd === 'casts') await casts();
  else {
    console.error('usage: ci-guards.mjs deps|lines|casts');
    process.exit(2);
  }
}
