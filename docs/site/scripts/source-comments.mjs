import ts from 'typescript';
import { extname } from 'node:path';
import { execFileSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import { Parser } from 'yaml';

export function scriptComments(body, filename, offset = 0) {
  const source = ts.createSourceFile(filename, body, ts.ScriptTarget.Latest, true,
    /\.[cm]?js$/.test(filename) ? ts.ScriptKind.JS : filename.endsWith('.tsx') ? ts.ScriptKind.TSX : ts.ScriptKind.TS);
  const ranges = new Map();
  const add = (start, end, kind) => {
    ranges.set(start, { start: offset + start, end: offset + end,
      kind: kind === ts.SyntaxKind.SingleLineCommentTrivia ? 'line' : 'block' });
  };
  const walk = node => {
    ts.forEachLeadingCommentRange(body, node.pos, add);
    ts.forEachTrailingCommentRange(body, node.pos, add);
    ts.forEachTrailingCommentRange(body, node.end, add);
    for (const child of node.getChildren(source)) walk(child);
  };
  walk(source);
  return [...ranges.values()].sort((a, b) => a.start - b.start);
}

export function cComments(body, { rust = false, line = true, offset = 0 } = {}) {
  const result = [];
  let at = 0;
  while (at < body.length) {
    if (rust && !/[\w]/.test(body[at - 1] ?? '')) {
      const raw = body.slice(at).match(/^(?:br|cr|r)(#{0,255})"/);
      if (raw) {
        const end = body.indexOf(`"${raw[1]}`, at + raw[0].length);
        if (end < 0) throw new Error('Unclosed Rust raw string');
        at = end + raw[1].length + 1;
        continue;
      }
    }
    const character = body[at];
    if (character === '"' || character === "'") {
      if (rust && character === "'") {
        const literal = body.slice(at).match(/^'(?:\\(?:u\{[0-9a-fA-F_]+\}|x[0-9a-fA-F]{2}|.)|[^'\\\r\n])'/u);
        at += literal ? literal[0].length : 1;
        continue;
      }
      const quote = character;
      at++;
      while (at < body.length) {
        if (body[at] === '\\') at += 2;
        else if (body[at++] === quote) break;
      }
      continue;
    }
    if (line && body.startsWith('//', at)) {
      const newline = body.indexOf('\n', at);
      const end = newline < 0 ? body.length : newline;
      result.push({ start: offset + at, end: offset + (body[end - 1] === '\r' ? end - 1 : end), kind: 'line' });
      at = end;
      continue;
    }
    if (body.startsWith('/*', at)) {
      const start = at;
      let depth = 1;
      at += 2;
      while (at < body.length && depth) {
        if (rust && body.startsWith('/*', at)) { depth++; at += 2; }
        else if (body.startsWith('*/', at)) { depth--; at += 2; }
        else at++;
      }
      if (depth) throw new Error('Unclosed block comment');
      result.push({ start: offset + start, end: offset + at, kind: 'block' });
      continue;
    }
    at++;
  }
  return result;
}

export function markupComments(body, filename) {
  const scripts = [...body.matchAll(/<(script|style)\b[^>]*>([\s\S]*?)<\/\1\s*>/gi)];
  const blocked = scripts.map(match => ({ start: match.index, end: match.index + match[0].length }));
  const result = [];
  if (filename.endsWith('.astro')) {
    const frontmatter = body.match(/^---\r?\n([\s\S]*?)\r?\n---/);
    if (frontmatter) {
      const start = frontmatter[0].indexOf(frontmatter[1]);
      result.push(...scriptComments(frontmatter[1], filename + '.ts', start));
      blocked.push({ start: 0, end: frontmatter[0].length });
    }
  }
  for (const match of scripts) {
    const offset = match.index + match[0].indexOf('>') + 1;
    result.push(...(match[1].toLowerCase() === 'style'
      ? cComments(match[2], { line: false, offset })
      : scriptComments(match[2], filename + '.ts', offset)));
  }
  for (const match of body.matchAll(/<!--[\s\S]*?-->/g)) {
    if (blocked.some(range => match.index >= range.start && match.index < range.end)) continue;
    result.push({ start: match.index, end: match.index + match[0].length, kind: 'markup' });
  }
  return result.sort((a, b) => a.start - b.start);
}

export function commentRanges(body, filename) {
  const extension = extname(filename);
  if (['.ts', '.tsx', '.js', '.mjs', '.cjs'].includes(extension)) return scriptComments(body, filename);
  if (extension === '.rs') return cComments(body, { rust: true });
  if (['.css', '.scss'].includes(extension)) return cComments(body, { line: false });
  if (['.svelte', '.astro', '.html', '.svg'].includes(extension)) return markupComments(body, filename);
  if (extension === '.jsonc') return cComments(body);
  if (['.yml', '.yaml'].includes(extension)) return yamlComments(body);
  if (extension === '.toml') return delimitedComments(body, 'toml');
  if (['.nsi', '.nsh'].includes(extension)) return delimitedComments(body, 'nsis');
  if (extension === '.sql') return delimitedComments(body, 'sql');
  if (extension === '.py') {
    const python = process.env.LLM_USAGE_DOCS_PYTHON ?? (process.platform === 'win32' ? 'python' : 'python3');
    return JSON.parse(execFileSync(python, [fileURLToPath(new URL('./python-comments.py', import.meta.url))],
      { input: JSON.stringify(body), encoding: 'utf8', env: { ...process.env, PYTHONIOENCODING: 'utf-8' } }));
  }
  if (extension === '.ps1') {
    return JSON.parse(execFileSync('pwsh', ['-NoLogo', '-NoProfile', '-NonInteractive', '-File',
      fileURLToPath(new URL('./powershell-comments.ps1', import.meta.url))], { input: JSON.stringify(body), encoding: 'utf8' }));
  }
  if (['.gitignore', '.gitattributes', '.editorconfig'].some(name => filename.endsWith(name))) {
    return [...body.matchAll(/^\s*(#[^\r\n]*)/gm)].map(match => ({ start: match.index + match[0].indexOf('#'),
      end: match.index + match[0].length, kind: 'line' }));
  }
  if (extension === '.sh') {
    // The current shell files have no standalone hash lines; reject new ones until shell parsing is supplied.
    if (/^\s*#/m.test(body)) throw new Error(`Shell comment parsing needs verification: ${filename}`);
    return [];
  }
  throw new Error(`No verified comment parser for ${filename}`);
}

export function yamlComments(body) {
  const result = [];
  const walk = value => {
    if (!value || typeof value !== 'object') return;
    if (value.type === 'comment') result.push({ start: value.offset, end: value.offset + value.source.length, kind: 'line' });
    for (const child of Object.values(value)) {
      if (Array.isArray(child)) child.forEach(walk);
      else if (child && typeof child === 'object') walk(child);
    }
  };
  for (const token of new Parser().parse(body)) walk(token);
  return result.sort((a, b) => a.start - b.start);
}

export function delimitedComments(body, language) {
  const result = [];
  let at = 0;
  while (at < body.length) {
    if (language === 'nsis' && body.startsWith('{{', at)) {
      const close = body.startsWith('{{{', at) ? '}}}' : '}}';
      at += close.length;
      let found = false;
      while (at < body.length) {
        if (body[at] === '"' || body[at] === "'") {
          const templateQuote = body[at++];
          while (at < body.length) {
            if (body[at] === '\\') at += 2;
            else if (body[at++] === templateQuote) break;
          }
        } else if (body.startsWith(close, at)) {
          at += close.length;
          found = true;
          break;
        } else at++;
      }
      if (!found) throw new Error('Unclosed NSIS template expression');
      continue;
    }
    const quote = body[at];
    if (quote === '"' || quote === "'" || (language === 'nsis' && quote === '`')) {
      const delimiter = language === 'toml' && body.startsWith(quote.repeat(3), at) ? quote.repeat(3) : quote;
      at += delimiter.length;
      while (at < body.length) {
        if (language === 'nsis' && body.startsWith('$\\', at)) { at += 3; continue; }
        if (language === 'toml' && quote === '"' && body[at] === '\\') { at += 2; continue; }
        if (language === 'sql' && body.startsWith(quote.repeat(2), at)) { at += 2; continue; }
        if (body.startsWith(delimiter, at)) { at += delimiter.length; break; }
        at++;
      }
      continue;
    }
    const line = language === 'toml' ? body[at] === '#'
      : language === 'nsis' ? ['#', ';'].includes(body[at]) : body.startsWith('--', at);
    if (line) {
      let end = body.indexOf('\n', at);
      if (end < 0) end = body.length;
      if (language === 'nsis') {
        while (/\\\r?$/.test(body.slice(at, end)) && end < body.length) {
          const next = body.indexOf('\n', end + 1);
          end = next < 0 ? body.length : next;
        }
      }
      result.push({ start: at, end: body[end - 1] === '\r' ? end - 1 : end, kind: 'line' });
      at = end;
    } else if (language !== 'toml' && body.startsWith('/*', at)) {
      const end = body.indexOf('*/', at + 2);
      if (end < 0) throw new Error('Unclosed block comment');
      result.push({ start: at, end: end + 2, kind: 'block' });
      at = end + 2;
    } else at++;
  }
  return result;
}

export function withoutComments(body, ranges) {
  let previous = 0;
  let result = '';
  for (const { start, end } of ranges) {
    if (start < previous || end < start || end > body.length) throw new Error('Invalid or overlapping comment range');
    result += body.slice(previous, start);
    previous = end;
  }
  return result + body.slice(previous);
}
