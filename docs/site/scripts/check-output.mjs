import { readFile } from 'node:fs/promises';
import { resolve, dirname, relative } from 'node:path';
import { output, files } from './content.mjs';
import { screenshotProblems } from './screenshots-check.mjs';
import { validateFeed } from './update-feed.mjs';

const paths = await files(output);
const known = new Set(paths);
const failures = [];
try {
  validateFeed(JSON.parse(await readFile(resolve(output, 'updates/latest.json'), 'utf8')), { fresh: false });
} catch {
  failures.push('Missing or invalid public software update metadata');
}
failures.push(...await screenshotProblems(output));
const html = new Map();
const entities = value => value.replace(/&(?:amp|quot|apos|lt|gt|#\d+|#x[0-9a-f]+);/gi, match => {
  const named = { '&amp;': '&', '&quot;': '"', '&apos;': "'", '&lt;': '<', '&gt;': '>' };
  if (named[match]) return named[match];
  const numeric = match.slice(2, -1);
  return String.fromCodePoint(numeric.startsWith('x') ? parseInt(numeric.slice(1), 16) : parseInt(numeric, 10));
});
for (const path of paths.filter(path => path.endsWith('.html'))) {
  const body = await readFile(path, 'utf8');
  html.set(path, { body, ids: new Set([...body.matchAll(/\bid="([^"]+)"/g)].map(match => entities(match[1]))) });
}
for (const path of paths.filter(path => path.endsWith('.html'))) {
  const { body } = html.get(path);
  const language = relative(output, path).replaceAll('\\', '/').startsWith('zh-cn/') ? 'zh-CN' : 'en';
  const sidebar = body.match(/<sl-sidebar-pane\b[\s\S]*?<\/sl-sidebar-pane>/)?.[0];
  if (sidebar) {
    const prefix = language === 'en' ? '' : '/zh-cn';
    for (const required of ['start/installation', 'guide/dashboard', 'guide/sources', 'development/setup',
      'reference/design/documentation-site', 'reference/evidence/current-acceptance', 'reference/repository/readme']) {
      if (!sidebar.includes(`href="${prefix}/${required}/"`)) failures.push(`${path}: missing sidebar page ${required}`);
    }
  }
  for (const [, imageLanguage] of body.matchAll(/<img\b[^>]*\bsrc="\/screenshots\/([^/]+)\//g)) {
    if (imageLanguage !== language) failures.push(`${path}: screenshot language differs from page language`);
  }
  if (body.includes('version https://git-lfs.github.com/spec/v1')) failures.push(`LFS pointer rendered in ${path}`);
  for (const [, encoded] of body.matchAll(/(?:href|src)="([^"]*)"/g)) {
    const target = entities(encoded);
    if (/^(?:[a-z][a-z\d+.-]*:|\/\/)/i.test(target)) continue;
    const [location, fragment] = target.split('#');
    const clean = decodeURIComponent(location.split('?')[0]);
    if (!clean && !fragment) continue;
    const candidate = clean.startsWith('/') ? resolve(output, `.${clean}`) : resolve(dirname(path), clean);
    const document = clean === '' ? path : known.has(candidate) ? candidate : resolve(candidate, 'index.html');
    if (!known.has(document)) failures.push(`${path}: broken local target ${target}`);
    else if (fragment && html.has(document) && !html.get(document).ids.has(decodeURIComponent(fragment))) {
      failures.push(`${path}: missing fragment ${target}`);
    }
  }
}
const cname = (await readFile(resolve(output, 'CNAME'), 'utf8')).trim();
if (cname !== 'llm-usage.atframe.work') failures.push('Incorrect custom domain');
if (!known.has(resolve(output, '.nojekyll'))) failures.push('Missing .nojekyll');
if (!known.has(resolve(output, 'index.html')) || !known.has(resolve(output, 'zh-cn/index.html'))) failures.push('Missing language home page');
if (!paths.some(path => path.includes('pagefind') && path.endsWith('.js'))) failures.push('Missing local search index');
if (failures.length) { console.error([...new Set(failures)].join('\n')); process.exitCode = 1; }
else console.log(`Checked ${paths.length} published files, local links and fragments, screenshots, language roots, search, update metadata and domain markers.`);
