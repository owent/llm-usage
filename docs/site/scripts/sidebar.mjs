import { readFile } from 'node:fs/promises';
import { relative, resolve } from 'node:path';
import { parse } from 'yaml';
import { documentationId, files, repositoryDocuments, root, route } from './content.mjs';

const categories = [
  ['start', 'Start here', '开始使用', false],
  ['guide', 'User guide', '用户指南', false],
  ['development', 'Developer guide', '开发指南', false],
  ['reference/design', 'Design specifications', '设计说明', true],
  ['reference/evidence', 'Development records', '开发验证记录', true],
  ['reference/repository', 'Project reference', '项目说明', true],
  ['reference/fixtures', 'Test data', '测试数据说明', true],
];

export function sidebarGroups(entries) {
  const ids = new Set();
  for (const { id } of entries) {
    if (ids.has(id)) throw new Error(`Duplicate sidebar page: ${id}`);
    ids.add(id);
  }
  const children = (pages, prefix) => {
    const groups = new Map();
    for (const page of pages) {
      const name = page.id.slice(prefix.length + 1).split('/')[0];
      const bucket = groups.get(name) ?? [];
      bucket.push(page);
      groups.set(name, bucket);
    }
    return [...groups].sort(([a, first], [b, second]) =>
      Math.min(...first.map(page => page.order ?? Infinity)) - Math.min(...second.map(page => page.order ?? Infinity))
      || a.localeCompare(b, 'en')).map(([name, pages]) => {
      const directory = `${prefix}/${name}`;
      const index = pages.find(page => page.id === directory);
      const descendants = pages.filter(page => page.id !== directory);
      if (!descendants.length) return { slug: index.id };
      return { label: name, collapsed: true, items: [
        ...(index ? [{ slug: index.id }] : []), ...children(descendants, directory),
      ] };
    });
  };
  return categories.map(([directory, label, chinese, collapsed]) => {
    const pages = entries.filter(page => page.id.startsWith(`${directory}/`));
    if (!pages.length) throw new Error(`Empty sidebar category: ${directory}`);
    return { label, translations: { 'zh-CN': chinese }, collapsed, items: children(pages, directory) };
  });
}

export async function documentationSidebar() {
  const entries = (await repositoryDocuments()).map(path => ({ id: route(path) }));
  const authored = resolve(root, 'docs/site/content');
  for (const path of await files(authored)) {
    const name = relative(authored, path).replaceAll('\\', '/');
    if (name.startsWith('zh-cn/') || !/\.mdx?$/.test(name) || name === 'index.mdx') continue;
    const body = await readFile(path, 'utf8');
    const metadata = parse(body.match(/^---\r?\n([\s\S]*?)\r?\n---/)?.[1] ?? '');
    if (!metadata?.sidebar?.hidden) entries.push({ id: documentationId(name), order: metadata?.sidebar?.order });
  }
  return sidebarGroups(entries);
}
