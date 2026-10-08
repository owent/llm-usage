import { readFile } from 'node:fs/promises';
import { createHash } from 'node:crypto';
import { extname, resolve } from 'node:path';
import { execFileSync } from 'node:child_process';
import { root, repoPath } from './content.mjs';
import { commentRanges, withoutComments } from './source-comments.mjs';

export const sourceExtensions = new Set(['.rs', '.ts', '.tsx', '.js', '.mjs', '.cjs', '.css', '.scss',
  '.svelte', '.astro', '.html', '.svg', '.jsonc', '.yml', '.yaml', '.toml', '.sql', '.py', '.ps1', '.nsi', '.nsh', '.sh']);
const normalize = body => body.replace(/\r\n?/g, '\n');
export const digest = body => createHash('sha256').update(normalize(body)).digest('hex');

export async function sourceFiles() {
  const paths = execFileSync('git', ['ls-files', '--cached', '--others', '--exclude-standard', '-z'], { cwd: root, encoding: 'utf8' });
  return [...new Set(paths.split('\0').filter(Boolean).map(path => resolve(root, path)))].sort().filter(path => {
    const key = repoPath(path);
    return !key.startsWith('docs/zh-CN/') && !key.startsWith('previous-draft/dashboard/assets/')
      && key !== 'previous-draft/dashboard/echarts.min.js'
      && !key.endsWith('package-lock.json') && (sourceExtensions.has(extname(path))
        || ['.gitignore', '.gitattributes', '.editorconfig'].some(name => key.endsWith(name)));
  });
}

export function sourceRecord(body, filename) {
  const ranges = commentRanges(body, filename);
  const occurrences = new Map();
  return { codeHash: digest(withoutComments(body, ranges)), comments: ranges.map(range => ({
    line: body.slice(0, range.start).split('\n').length,
    kind: range.kind,
    en: normalize(body.slice(range.start, range.end)),
  })).map(comment => {
    const occurrence = occurrences.get(comment.en) ?? 0;
    occurrences.set(comment.en, occurrence + 1);
    return { id: digest(`${filename}\0${comment.en}\0${occurrence}`).slice(0, 16), ...comment };
  }) };
}

export function commentTranslationProblems(comment, entry) {
  const problems = [];
  if (!entry.zh || !/\p{Script=Han}/u.test(entry.zh)) problems.push('Missing Chinese comment');
  const chineseBody = (entry.zh ?? '').replace(/^(?:\/\/[!/]?|#|;|--|\/\*+|<!--)\s*/, '')
    .replace(/\s*(?:\*\/|-->)$/, '').trim();
  if (chineseBody === '（空行）' && /[\p{L}\p{N}]/u.test(comment.en)) {
    problems.push('Nonempty source comment has an empty Chinese reference');
  }
  return problems;
}

export async function commentProblems(registry) {
  if (registry.schema !== 1) throw new Error('Unknown source-comment registry schema');
  const problems = [];
  const known = new Set();
  for (const path of await sourceFiles()) {
    const key = repoPath(path);
    known.add(key);
    let actual;
    try { actual = sourceRecord(await readFile(path, 'utf8'), key); }
    catch (error) { problems.push(`Cannot parse source comments: ${key} (${error.message})`); continue; }
    if (!actual.comments.length) {
      if (registry.files[key]) problems.push(`Obsolete comment reference: ${key}`);
      continue;
    }
    const reviewed = registry.files[key];
    if (!reviewed) { problems.push(`Missing Chinese comment reference: ${key}`); continue; }
    if (reviewed.codeHash !== actual.codeHash) problems.push(`Code changed without comment-reference review: ${key}`);
    if (reviewed.comments.length !== actual.comments.length) { problems.push(`Comment count changed: ${key}`); continue; }
    for (const [index, comment] of actual.comments.entries()) {
      const entry = reviewed.comments[index];
      if (entry.en !== comment.en || entry.kind !== comment.kind || entry.id !== comment.id || entry.line !== comment.line) {
        problems.push(`Comment changed without bilingual review: ${key}:${comment.line}`);
      }
      if (/\p{Script=Han}/u.test(comment.en)) problems.push(`Untranslated source comment: ${key}:${comment.line}`);
      for (const problem of commentTranslationProblems(comment, entry)) problems.push(`${problem}: ${key}:${comment.line}`);
    }
  }
  for (const key of Object.keys(registry.files)) if (!known.has(key)) problems.push(`Orphaned comment reference: ${key}`);
  return problems;
}
