import { execFileSync } from 'node:child_process';
import { writeFile } from 'node:fs/promises';
import { resolve } from 'node:path';
import { root, output } from './content.mjs';
import { outputDigests, requiredChecks, validationFile } from './publication-checks.mjs';

const git = args => execFileSync('git', args, { cwd: root, encoding: 'utf8' }).trim();
const sourceRevision = git(['rev-parse', 'HEAD']);
if (process.env.GITHUB_SHA && process.env.GITHUB_SHA !== sourceRevision) throw new Error('Checkout differs from the workflow revision');
if (git(['status', '--porcelain', '--untracked-files=normal'])) throw new Error('Commit publication inputs before recording source-revision evidence');
await writeFile(resolve(output, validationFile), JSON.stringify({ schema: 1, sourceRevision,
  checks: requiredChecks, files: await outputDigests(output) }, null, 2) + '\n');
console.log(`Recorded checked publication output for ${sourceRevision}. Run only after content, unit, build and browser checks succeed.`);
