import { readFile } from 'node:fs/promises';
import { resolve, relative } from 'node:path';
import { createHash } from 'node:crypto';
import { root, files, repositoryDocuments, chinesePath, repoPath, pairedAnchors } from './content.mjs';
import { unified } from 'unified';
import remarkParse from 'remark-parse';
import remarkGfm from 'remark-gfm';
import { visit } from 'unist-util-visit';
import { screenshotProblems } from './screenshots-check.mjs';
import { commentProblems } from './comments-registry.mjs';

const hash = body => createHash('sha256').update(body.replace(/\r\n?/g, '\n')).digest('hex');
const problems = [];
problems.push(...await screenshotProblems());
problems.push(...await commentProblems(JSON.parse(await readFile(resolve(root, 'docs/source-comments.json'), 'utf8'))));
const manifest = JSON.parse(await readFile(resolve(root, 'docs/translations.json'), 'utf8'));
if (manifest.schema !== 1) throw new Error('Unknown translation manifest schema');
function checkPair(key, english, chinese, entry) {
  if (!entry) problems.push(`Missing translation review: ${key}`);
  else {
    if (entry.en !== hash(english)) problems.push(`English changed without translation review: ${key}`);
    if (entry.zh !== hash(chinese)) problems.push(`Chinese changed without translation review: ${key}`);
  }
  let prose = '';
  visit(unified().use(remarkParse).use(remarkGfm).parse(english), 'text', node => { prose += node.value; });
  if (/\p{Script=Han}/u.test(prose)) problems.push(`Untranslated English prose: ${key}`);
  if (!/\p{Script=Han}/u.test(chinese)) problems.push(`Chinese translation has no Chinese content: ${key}`);
  try { pairedAnchors(english, chinese); }
  catch (error) { problems.push(`${key}: ${error.message}`); }
}
const documents = await repositoryDocuments();
for (const path of documents) {
  const key = repoPath(path);
  try {
    const english = await readFile(path, 'utf8');
    const chinese = await readFile(chinesePath(path), 'utf8');
    checkPair(key, english, chinese, manifest.documents[key]);
  } catch (error) { problems.push(`Missing or unreadable Chinese document: ${key} (${error.code})`); }
}
const documentKeys = new Set(documents.map(repoPath));
for (const key of Object.keys(manifest.documents)) if (!documentKeys.has(key)) problems.push(`Orphaned repository review: ${key}`);
const mirror = resolve(root, 'docs/zh-CN');
for (const path of (await files(mirror)).filter(path => /\.mdx?$/.test(path))) {
  const key = relative(mirror, path).replaceAll('\\', '/');
  if (!documentKeys.has(key)) problems.push(`Orphaned Chinese document: ${key}`);
}
const authored = resolve(root, 'docs/site/content');
const pages = (await files(authored)).filter(path => /\.mdx?$/.test(path));
const englishPages = pages.filter(path => !relative(authored, path).replaceAll('\\', '/').startsWith('zh-cn/'));
for (const page of englishPages) {
  const key = relative(authored, page);
  try {
    const english = await readFile(page, 'utf8');
    const chinese = await readFile(resolve(authored, 'zh-cn', key), 'utf8');
    checkPair(repoPath(page), english, chinese, manifest.guides?.[repoPath(page)]);
  }
  catch { problems.push(`Missing Chinese guide: ${key}`); }
}
const guideKeys = new Set(englishPages.map(repoPath));
for (const key of Object.keys(manifest.guides ?? {})) if (!guideKeys.has(key)) problems.push(`Orphaned guide review: ${key}`);
for (const page of pages.filter(page => relative(authored, page).replaceAll('\\', '/').startsWith('zh-cn/'))) {
  const english = resolve(authored, relative(resolve(authored, 'zh-cn'), page));
  if (!guideKeys.has(repoPath(english))) problems.push(`Orphaned Chinese guide: ${repoPath(page)}`);
}
for (const page of pages) {
  const body = await readFile(page, 'utf8');
  if (!/^---\r?\n/.test(body) || !/^title: .+/m.test(body)) problems.push(`Missing guide title: ${repoPath(page)}`);
  const locale = relative(authored, page).replaceAll('\\', '/').startsWith('zh-cn/') ? 'zh-CN' : 'en';
  for (const [, attributes] of body.matchAll(/<AppScreenshot\b([^>]*)>/g)) {
    if (attributes.match(/\blanguage="([^"]+)"/)?.[1] !== locale) {
      problems.push(`Screenshot component language mismatch: ${repoPath(page)}`);
    }
    if (!/\balt="[^"]+"/.test(attributes)) problems.push(`Missing screenshot description: ${repoPath(page)}`);
  }
  for (const [, image] of body.matchAll(/\]\((\/screenshots\/[^)]+)\)/g)) {
    if (!image.startsWith(`/screenshots/${locale}/`)) problems.push(`Screenshot language mismatch: ${repoPath(page)} → ${image}`);
  }
}
if (problems.length) {
  console.error(problems.join('\n'));
  process.exitCode = 1;
} else console.log(`Checked ${Object.keys(manifest.documents).length} repository translations and ${englishPages.length} paired guides.`);
