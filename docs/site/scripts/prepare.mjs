import { mkdir, writeFile, readFile, rm } from 'node:fs/promises';
import { resolve, dirname, relative } from 'node:path';
import { root, generated, files, repositoryDocuments, chinesePath, route, title, rewriteLinks, pairedAnchors, referenceBody } from './content.mjs';

// Generated content is owned exclusively by this build and always stays under root build/.
if (!generated.startsWith(resolve(root, 'build') + '/'.replace('/', process.platform === 'win32' ? '\\' : '/'))) {
  throw new Error('Generated content escaped the repository build directory');
}
await rm(generated, { recursive: true, force: true });
await mkdir(generated, { recursive: true });
const documents = await repositoryDocuments();
const known = new Set(documents);
const routes = new Set();
for (const source of documents) {
  const id = route(source);
  if (routes.has(id)) throw new Error(`Duplicate documentation route: ${id}`);
  routes.add(id);
  const originals = await Promise.all([source, chinesePath(source)].map(path => readFile(path, 'utf8')));
  let bodies;
  try { bodies = pairedAnchors(...originals.map(referenceBody)); }
  catch (error) { throw new Error(`${relative(root, source)}: ${error.message}`); }
  for (const [index, language] of ['en', 'zh-CN'].entries()) {
    const body = bodies[index];
    const target = resolve(generated, language === 'en' ? '' : 'zh-cn', `${id}.md`);
    await mkdir(dirname(target), { recursive: true });
    const actualSource = index === 0 ? source : chinesePath(source);
    const sourceName = relative(root, actualSource).replaceAll('\\', '/');
    const metadata = `---\ntitle: ${JSON.stringify(title(originals[index], id))}\neditUrl: ${JSON.stringify(`https://github.com/owent/llm-usage/edit/main/${sourceName}`)}\n---\n\n`;
    await writeFile(target, metadata + rewriteLinks(body, actualSource, known));
  }
}
const authored = resolve(root, 'docs/site/content');
const commentRegistry = JSON.parse(await readFile(resolve(root, 'docs/source-comments.json'), 'utf8'));
for (const language of ['en', 'zh-cn']) {
  const directory = resolve(generated, language === 'en' ? '' : language, 'reference/comments');
  await mkdir(directory, { recursive: true });
  const entries = [];
  for (const [key, record] of Object.entries(commentRegistry.files)) {
    const id = key.replace(/^\./, '').toLowerCase();
    const target = resolve(directory, `${id}.md`);
    await mkdir(dirname(target), { recursive: true });
    const body = record.comments.map((comment, index) => {
      const label = language === 'en' ? `Comment ${index + 1}, line ${comment.line}` : `第 ${index + 1} 条注释，第 ${comment.line} 行`;
      const text = language === 'en' ? comment.en : comment.zh;
      const fence = '`'.repeat(Math.max(3, ...[...text.matchAll(/`+/g)].map(match => match[0].length)) + 1);
      return `<a id="${comment.id}"></a>\n\n## ${label}\n\n[${key}:${comment.line}](https://github.com/owent/llm-usage/blob/main/${key}#L${comment.line})\n\n${fence}text\n${text}\n${fence}\n`;
    }).join('\n');
    await writeFile(target, `---\ntitle: ${JSON.stringify(key)}\neditUrl: https://github.com/owent/llm-usage/edit/main/docs/source-comments.json\n---\n\n${body}`);
    entries.push(`- [${key}](${language === 'en' ? '/' : '/zh-cn/'}reference/comments/${id}/)`);
  }
  const heading = language === 'en' ? 'Source comment references' : '源码注释参考';
  const explanation = language === 'en' ? 'Reviewed English source comments and their Chinese counterparts. Third-party license notices remain unchanged.'
    : '已审阅的英文源码注释及对应中文版本。第三方许可证声明保持原样。';
  await writeFile(resolve(directory, 'index.md'), `---\ntitle: ${heading}\n---\n\n${explanation}\n\n${entries.join('\n')}\n`);
}
for (const source of (await files(authored)).filter(path => !relative(authored, path).replaceAll('\\', '/').startsWith('zh-cn/'))) {
  const name = relative(authored, source);
  const sources = [source, resolve(authored, 'zh-cn', name)];
  const pair = await Promise.all(sources.map(async path => {
    const body = await readFile(path, 'utf8');
    const editUrl = `https://github.com/owent/llm-usage/edit/main/${relative(root, path).replaceAll('\\', '/')}`;
    return body.replace(/^---\r?\n/, `---\neditUrl: ${JSON.stringify(editUrl)}\n`);
  }));
  const bodies = pairedAnchors(...pair);
  for (const [index, language] of ['en', 'zh-cn'].entries()) {
    const target = resolve(generated, language === 'en' ? '' : language, name);
    await mkdir(dirname(target), { recursive: true });
    await writeFile(target, bodies[index]);
  }
}
console.log(`Prepared ${documents.length} paired repository documents and authored guides.`);
