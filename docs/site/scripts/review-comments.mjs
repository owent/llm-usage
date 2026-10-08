import { readFile, writeFile } from 'node:fs/promises';
import { resolve } from 'node:path';
import { root, repoPath } from './content.mjs';
import { sourceFiles, sourceRecord } from './comments-registry.mjs';

const args = process.argv.slice(2);
if (!args.length || args.includes('--help')) {
  console.log('Record manually reviewed English source comments and Chinese translations.\nUsage: npm run docs:comments:review -- <source-file> --translations <JSON-file>\nThe JSON file must contain one Chinese string per source comment, in source order.\nReview the implementation and both languages first. Updates the registry only; no source mutation or publication.');
  process.exit(0);
}
const flag = args.indexOf('--translations');
if (flag < 0 || !args[flag + 1]) throw new Error('Supply a JSON array of reviewed Chinese comments with --translations');
const path = resolve(root, args[0]);
if (!(await sourceFiles()).includes(path)) throw new Error('Not a first-party repository source file');
const record = sourceRecord(await readFile(path, 'utf8'), repoPath(path));
const translations = JSON.parse(await readFile(resolve(root, args[flag + 1]), 'utf8'));
if (!Array.isArray(translations) || translations.length !== record.comments.length) throw new Error('Chinese translations must match the exact source comment count');
for (const [index, comment] of record.comments.entries()) {
  if (/\p{Script=Han}/u.test(comment.en)) throw new Error(`Untranslated source comment at line ${comment.line}`);
  const chinese = translations[index];
  if (typeof chinese !== 'string' || !/\p{Script=Han}/u.test(chinese)) throw new Error(`Missing Chinese translation at line ${comment.line}`);
  comment.zh = chinese.replace(/\r\n?/g, '\n');
}
const filename = resolve(root, 'docs/source-comments.json');
const registry = JSON.parse(await readFile(filename, 'utf8'));
if (registry.schema !== 1) throw new Error('Unknown comment registry schema');
if (record.comments.length) registry.files[repoPath(path)] = record;
else delete registry.files[repoPath(path)];
registry.files = Object.fromEntries(Object.entries(registry.files).sort(([a], [b]) => a.localeCompare(b, 'en')));
await writeFile(filename, JSON.stringify(registry, null, 2) + '\n');
console.log(`Recorded ${record.comments.length} reviewed comment pairs for ${repoPath(path)}.`);
