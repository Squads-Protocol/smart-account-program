// The auditability guard (scripts/ci-guards.mjs casts): no `as` assertions or `any` in src/ outside src/encoding/.
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { describe, it } from 'node:test';
import { fileURLToPath } from 'node:url';

const script = new URL('../scripts/ci-guards.mjs', import.meta.url);
const guards = (await import(script.href)) as { castViolations(src: string): string[] };

describe('ci-guards casts', () => {
  it('passes on src/ as committed', () => {
    const r = spawnSync(process.execPath, [fileURLToPath(script), 'casts'], { encoding: 'utf8' });
    assert.equal(r.status, 0, r.stderr);
  });
  it('safeguard removed: a cast slipped into real src (send.ts) is caught', async () => {
    const { readFileSync } = await import('node:fs');
    const real = readFileSync(new URL('../src/send.ts', import.meta.url), 'utf8');
    assert.deepEqual(guards.castViolations(real), []);
    const tampered = real.replace('const signature = getSignatureFromTransaction(transaction);', 'const signature = getSignatureFromTransaction(transaction) as Signature;');
    assert.notEqual(tampered, real);
    assert.equal(guards.castViolations(tampered).length, 1);
  });
  it('flags type assertions and any, including inside template expressions', () => {
    for (const src of [
      'const a = b as Address;',
      'const a = <T>(b as unknown);',
      'let x: any = 1;',
      'f((y as { z: number }).z);',
      'const s = `${n as bigint}`;',
      'const t = `a${`b${c as D}`}`;',
    ]) assert.equal(guards.castViolations(src).length, 1, src);
  });
  it('ignores as const, comments, strings and import/export renames', () => {
    for (const src of [
      'const L = [1, 2] as const;',
      '/** such as any wallet, as in fetchAccount */ const a = 1;',
      '// cast as Address is not allowed\nconst b = 2;',
      "const m = 'as Address or any';",
      'const m = `as any ${x} as well`;',
      "import { a as b, type C as D } from './x.ts';",
      "export { e as f } from './y.ts';",
      "export * as ns from './z.ts';",
    ]) assert.deepEqual(guards.castViolations(src), [], src);
  });
});
