import { readdir, readFile } from 'node:fs/promises';
import { resolve, relative, extname, dirname } from 'node:path';
import { unified } from 'unified';
import remarkParse from 'remark-parse';
import remarkGfm from 'remark-gfm';
import { visit } from 'unist-util-visit';
import Slugger from 'github-slugger';

export const root = resolve(import.meta.dirname, '../../..');
export const generated = resolve(root, 'build/documentation-site/content');
export const output = resolve(root, 'build/documentation-site/dist');
const excluded = new Set(['.git', '.kilo', '.codex', '.astro', '.venv', '__pycache__', 'node_modules', 'target', 'build', 'isolated-temp']);

export async function files(directory) {
  const entries = await readdir(directory, { withFileTypes: true });
  const result = [];
  for (const entry of entries) {
    if (excluded.has(entry.name)) continue;
    const path = resolve(directory, entry.name);
    if (entry.isDirectory()) result.push(...await files(path));
    else if (entry.isFile()) result.push(path);
  }
  return result.sort();
}

export function repoPath(path) { return relative(root, path).replaceAll('\\', '/'); }
export function chinesePath(path) { return resolve(root, 'docs/zh-CN', repoPath(path)); }

export function documentationId(entry) {
  return entry.replaceAll('\\', '/').replace(/\.(md|mdx)$/, '').replace(/\/index$/, '');
}

export function bilingualDocument(key) {
  return /\.mdx?$/.test(key) && !key.startsWith('docs/zh-CN/')
    && !key.startsWith('docs/site/') && !key.startsWith('.agents/')
    && key !== 'AGENTS.md' && key !== 'Plan.md'
    && key !== 'docs/design/desktop-usage/execution.md';
}

export async function repositoryDocuments() {
  return (await files(root)).filter(path => ['.md', '.mdx'].includes(extname(path))
    && bilingualDocument(repoPath(path)));
}

export function route(path) {
  const name = repoPath(path).replace(/\.(md|mdx)$/, '').replaceAll('_expectations', 'expectations');
  if (name.startsWith('docs/design/desktop-usage/')) return `reference/design/${name.slice('docs/design/desktop-usage/'.length)}`.toLowerCase();
  if (name.startsWith('docs/design/')) return `reference/design/${name.slice('docs/design/'.length)}`.toLowerCase();
  if (name.startsWith('docs/validation/desktop-usage/')) return `reference/evidence/${name.slice('docs/validation/desktop-usage/'.length)}`.toLowerCase();
  const fixture = 'desktop/src-tauri/crates/core/tests/fixtures/';
  if (name.startsWith(fixture)) return `reference/fixtures/${name.slice(fixture.length)}`.toLowerCase();
  return `reference/repository/${name.replace(/^\.agents\//, 'agents/')}`.toLowerCase();
}

export function title(body, fallback) {
  return body.match(/^#\s+(.+)$/m)?.[1].replaceAll('`', '') ?? fallback;
}

export function rewriteLinks(body, source, documents) {
  const sourceDirectory = dirname(source);
  const edits = [];
  visit(unified().use(remarkParse).use(remarkGfm).parse(body), ['link', 'image', 'definition'], node => {
    const target = node.url;
    const start = node.position.start.offset;
    const end = node.position.end.offset;
    if (node.type === 'image' && /^https?:/.test(target)) {
      // Preserve reference badge text without sending requests to third-party image hosts.
      edits.push({ start, end, text: node.alt ?? '' });
      return;
    }
    if (/^(https?:|mailto:|data:|\/|#)/.test(target)) return;
    const [file, fragment = ''] = target.split('#');
    const physical = resolve(sourceDirectory, decodeURIComponent(file));
    const mirror = resolve(root, 'docs/zh-CN');
    const mirrorRelative = relative(mirror, physical);
    const isMirror = mirrorRelative !== '..' && !mirrorRelative.startsWith(`..${process.platform === 'win32' ? '\\' : '/'}`) && !/^(?:[a-z]:|\/)/i.test(mirrorRelative);
    const resolved = isMirror ? resolve(root, mirrorRelative) : physical;
    const publicAsset = repoPath(resolved);
    const screenshot = publicAsset.startsWith('docs/site/public/screenshots/')
      ? `/${publicAsset.slice('docs/site/public/'.length)}` : null;
    const document = documents.has(resolved) ? resolved : documents.has(resolve(resolved, 'README.md')) ? resolve(resolved, 'README.md') : null;
    const href = screenshot ?? (document
      ? `${isMirror ? '/zh-cn/' : '/'}${route(document)}/${fragment ? `#${fragment}` : ''}`
      : `https://github.com/owent/llm-usage/blob/main/${repoPath(resolved)}${fragment ? `#${fragment}` : ''}`);
    const raw = body.slice(start, end);
    const offset = raw.lastIndexOf(target);
    if (offset < 0) throw new Error(`Cannot locate Markdown URL in ${repoPath(source)}: ${target}`);
    edits.push({ start: start + offset, end: start + offset + target.length, text: href });
  });
  for (const edit of edits.sort((a, b) => b.start - a.start)) body = body.slice(0, edit.start) + edit.text + body.slice(edit.end);
  return body;
}

export async function readDocument(path) { return readFile(path, 'utf8'); }

export function pairedAnchors(english, chinese) {
  const headings = body => {
    const result = [];
    const slugger = new Slugger();
    const metadata = body.match(/^---\r?\n[\s\S]*?\r?\n---(?:\r?\n|$)/)?.[0] ?? '';
    const text = node => node.type === 'text' || node.type === 'inlineCode' ? node.value : (node.children ?? []).map(text).join('');
    const tree = unified().use(remarkParse).use(remarkGfm).parse(body.slice(metadata.length));
    visit(tree, 'heading', node => {
      result.push({ start: metadata.length + node.position.start.offset, end: metadata.length + node.position.end.offset,
        at: metadata.length + (node.depth === 1 ? node.position.end.offset : node.position.start.offset),
        after: node.depth === 1, id: slugger.slug(text(node)), aliases: [] });
    });
    const explicit = new Set();
    const onlyAnchors = value => !value.replace(/(?:<a\s+id="[^"]+"\s*>|<\/a>)/g, '').trim();
    visit(tree, 'html', node => {
      for (const match of node.value.matchAll(/<a\s+id="([^"]+)"\s*>/g)) {
        const start = metadata.length + node.position.start.offset + match.index;
        const end = start + match[0].length;
        explicit.add(match[1]);
        const following = result.findIndex(heading => heading.start >= end);
        if (following >= 0 && onlyAnchors(body.slice(end, result[following].start))) {
          result[following].aliases.push(match[1]);
        } else {
          const preceding = result.findLast(heading => heading.end <= start);
          if (preceding?.after && onlyAnchors(body.slice(preceding.end, start))) preceding.aliases.push(match[1]);
        }
      }
    });
    return { list: result, explicit };
  };
  const en = headings(english);
  const zh = headings(chinese);
  if (en.list.length !== zh.list.length) throw new Error(`Translated heading structure differs (${en.list.length} English, ${zh.list.length} Chinese)`);
  const add = (body, own, other) => {
    const known = new Set([...own.list.map(heading => heading.id), ...own.explicit]);
    for (let i = own.list.length - 1; i >= 0; i--) {
      const aliases = [other.list[i].id, ...other.list[i].aliases].filter(alias => {
        if (known.has(alias)) return false;
        known.add(alias);
        return true;
      });
      if (!aliases.length) continue;
      const anchors = aliases.map(alias => `<a id="${alias}"></a>`).join('\n\n');
      const insertion = own.list[i].after ? `\n\n${anchors}` : `${anchors}\n\n`;
      body = body.slice(0, own.list[i].at) + insertion + body.slice(own.list[i].at);
    }
    return body;
  };
  return [add(english, en, zh), add(chinese, zh, en)];
}

export function referenceBody(body) {
  const metadata = body.match(/^---\r?\n([\s\S]*?)\r?\n---(?:\r?\n|$)/);
  const content = metadata ? body.slice(metadata[0].length) : body;
  // Skill metadata is source documentation, not Starlight's page configuration.
  const prefix = metadata ? `\`\`\`yaml\n${metadata[1]}\n\`\`\`\n\n` : '';
  const heading = content.match(/^#\s+([^\n]+)\r?\n/m);
  const id = heading ? new Slugger().slug(heading[1].replaceAll('`', '').trim()) : null;
  const anchor = id && !content.includes(`<a id="${id}">`) ? `<a id="${id}"></a>\n\n` : '';
  return prefix + anchor + content.replace(/^#\s+[^\n]+\r?\n/m, '');
}
