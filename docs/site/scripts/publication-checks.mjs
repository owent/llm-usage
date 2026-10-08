import { createHash } from 'node:crypto';
import { readFile, readdir } from 'node:fs/promises';
import { resolve, relative } from 'node:path';

export const validationFile = '.llm-usage-validation.json';
export const requiredChecks = ['content', 'unit', 'build', 'browser'];
async function files(directory) {
  const result = [];
  for (const entry of await readdir(directory, { withFileTypes: true })) {
    const path = resolve(directory, entry.name);
    if (entry.isSymbolicLink()) throw new Error(`Symbolic link in publication artifact: ${entry.name}`);
    if (entry.isDirectory()) result.push(...await files(path));
    else if (entry.isFile()) result.push(path);
    else throw new Error(`Unexpected publication artifact entry: ${entry.name}`);
  }
  return result;
}
export async function outputDigests(directory) {
  const result = {};
  for (const path of (await files(directory)).filter(path => relative(directory, path) !== validationFile)) {
    result[relative(directory, path).replaceAll('\\', '/')] = createHash('sha256').update(await readFile(path)).digest('hex');
  }
  return Object.fromEntries(Object.entries(result).sort(([a], [b]) => a.localeCompare(b, 'en')));
}

export async function verifyPublication(directory, sourceRevision) {
  const evidence = JSON.parse(await readFile(resolve(directory, validationFile), 'utf8'));
  if (evidence.schema !== 1 || evidence.sourceRevision !== sourceRevision || !Array.isArray(evidence.checks)
    || requiredChecks.some(check => !evidence.checks.includes(check))) throw new Error('Publication lacks matching successful-check evidence');
  const actual = await outputDigests(directory);
  const keys = Object.keys(actual);
  if (!evidence.files || keys.length !== Object.keys(evidence.files).length
    || keys.some(key => actual[key] !== evidence.files[key])) throw new Error('Compiled documentation changed after validation');
  return evidence;
}
