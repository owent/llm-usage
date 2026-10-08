import { readFile, writeFile } from 'node:fs/promises';
import { resolve, relative } from 'node:path';
import { createHash } from 'node:crypto';
import { root, repoPath, chinesePath, repositoryDocuments, files } from './content.mjs';
import { unified } from 'unified';
import remarkParse from 'remark-parse';
import remarkGfm from 'remark-gfm';
import { visit } from 'unist-util-visit';

const args = process.argv.slice(2);
if (!args.length || args.includes('--help')) {
  console.log('Record a manually reviewed English/Chinese document pair.\nUsage: node docs/site/scripts/review-translations.mjs <repository-file> [...]\nRead and review both files before using this command. It refuses Chinese prose in the English edition and missing Chinese prose.\nUpdates docs/translations.json only; it does not translate, approve semantics or publish.');
  process.exit(0);
}
const filename = resolve(root, 'docs/translations.json');
let manifest;
try { manifest = JSON.parse(await readFile(filename, 'utf8')); }
catch (error) { if (error.code !== 'ENOENT') throw error; manifest = { schema: 1, documents: {} }; }
const hash = body => createHash('sha256').update(body.replace(/\r\n?/g, '\n')).digest('hex');
const known = new Set(await repositoryDocuments());
const authored = resolve(root, 'docs/site/content');
const guides = new Set((await files(authored)).filter(path => /\.mdx?$/.test(path)
  && !relative(authored, path).replaceAll('\\', '/').startsWith('zh-cn/')));
manifest.guides ??= {};
for (const name of args) {
  const source = resolve(root, name);
  if (!known.has(source) && !guides.has(source)) throw new Error(`Not an English repository document or guide: ${name}`);
  const english = await readFile(source, 'utf8');
  const chinese = await readFile(guides.has(source)
    ? resolve(authored, 'zh-cn', relative(authored, source)) : chinesePath(source), 'utf8');
  let englishProse = '';
  visit(unified().use(remarkParse).use(remarkGfm).parse(english), 'text', node => englishProse += node.value);
  if (/\p{Script=Han}/u.test(englishProse)) throw new Error(`Untranslated Chinese prose remains in ${name}`);
  if (!/\p{Script=Han}/u.test(chinese)) throw new Error(`Chinese edition has not been translated: ${name}`);
  (guides.has(source) ? manifest.guides : manifest.documents)[repoPath(source)] = { en: hash(english), zh: hash(chinese) };
}
manifest.documents = Object.fromEntries(Object.entries(manifest.documents).sort(([a], [b]) => a.localeCompare(b, 'en')));
manifest.guides = Object.fromEntries(Object.entries(manifest.guides).sort(([a], [b]) => a.localeCompare(b, 'en')));
await writeFile(filename, JSON.stringify(manifest, null, 2) + '\n');
console.log(`Recorded ${args.length} reviewed pairs; ${Object.keys(manifest.documents).length} repository pairs and ${Object.keys(manifest.guides).length} guide pairs overall.`);
