import test from 'node:test';
import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import { commentRanges, withoutComments } from '../scripts/source-comments.mjs';
import { commentTranslationProblems } from '../scripts/comments-registry.mjs';

test('Chinese empty-line references cannot replace a source path, formula or explanation', () => {
  const empty = { zh: '//!（空行）' };
  assert.deepEqual(commentTranslationProblems({ en: '//!' }, empty), []);
  assert.deepEqual(commentTranslationProblems({ en: '// ----' }, { zh: '//（空行）' }), []);
  for (const en of ['//! See architecture.md#adapter-layout.', '/// total=input+output.', '// Keep unknown values.']) {
    assert.deepEqual(commentTranslationProblems({ en }, empty),
      ['Nonempty source comment has an empty Chinese reference']);
  }
  assert.deepEqual(commentTranslationProblems({ en: '//! See architecture.md.' }, { zh: '//! 参见 architecture.md。' }), []);
  assert.deepEqual(commentTranslationProblems({ en: '// Empty marker' }, { zh: '// “（空行）”仅标识空注释。' }), []);
  assert.deepEqual(commentTranslationProblems({ en: '// Explanation' }, { zh: '// Explanation' }), ['Missing Chinese comment']);
});

test('Rust comments stay separate from raw strings, byte strings, chars, lifetimes and nested blocks', () => {
  const body = 'let raw = br###"// 中文字符串 /* data */"###;\n'
    + 'let escaped = "\\\" // unchanged"; let c = \'/\'; let borrowed: &\'a str;\n'
    + '/* 外层 /* 内层 */ 结束 */\n// 注释\nlet value = 42;';
  const ranges = commentRanges(body, 'test.rs');
  assert.deepEqual(ranges.map(range => body.slice(range.start, range.end)),
    ['/* 外层 /* 内层 */ 结束 */', '// 注释']);
  const translated = body.replace('/* 外层 /* 内层 */ 结束 */', '/* Outer /* inner */ end */').replace('// 注释', '// Comment');
  assert.equal(withoutComments(body, ranges), withoutComments(translated, commentRanges(translated, 'test.rs')));
  const windows = '// 注释\r\nlet value = 42;';
  assert.equal(windows.slice(commentRanges(windows, 'test.rs')[0].start, commentRanges(windows, 'test.rs')[0].end), '// 注释');
});

test('TypeScript comments do not consume regular expressions or template text', () => {
  const body = 'const pattern = /\\/\\//; const value = `// 字符串 ${ /* 真注释 */ 1}`; // 末尾\n';
  const ranges = commentRanges(body, 'test.ts');
  assert.deepEqual(ranges.map(range => body.slice(range.start, range.end)), ['/* 真注释 */', '// 末尾']);
  const empty = 'try {} catch { /* 空块注释 */ }';
  assert.deepEqual(commentRanges(empty, 'empty.ts').map(range => empty.slice(range.start, range.end)), ['/* 空块注释 */']);
  const consecutive = '// First\r\n// Second\r\nconst x = 1;\r\n// Third\r\n// Fourth\r\n';
  assert.deepEqual(commentRanges(consecutive, 'consecutive.ts').map(range => consecutive.slice(range.start, range.end)),
    ['// First', '// Second', '// Third', '// Fourth']);
});

test('Svelte isolates scripts, styles, HTML comments and user-facing strings', () => {
  const body = '<script lang="ts">const label = "<!--原文-->"; // 代码注释\n</script>\n'
    + '<style>/* 样式注释 */ .x { content: "/*原文*/"; }</style>\n<!--标记注释--><p>中文界面</p>';
  const ranges = commentRanges(body, 'test.svelte');
  assert.deepEqual(ranges.map(range => body.slice(range.start, range.end)),
    ['// 代码注释', '/* 样式注释 */', '<!--标记注释-->']);
  assert.ok(withoutComments(body, ranges).includes('<p>中文界面</p>'));
});

test('Astro frontmatter is code while ordinary HTML is markup', () => {
  const body = '---\nconst value = "// 原文"; // 代码注释\n---\n<!--标记注释--><p>{value}</p>';
  const ranges = commentRanges(body, 'test.astro');
  assert.deepEqual(ranges.map(range => body.slice(range.start, range.end)), ['// 代码注释', '<!--标记注释-->']);
});

test('YAML block scalars, quoted hashes and plain embedded hashes are data', () => {
  const body = 'name: "# 原文" # 注释\nlink: example#part\nrun: |\n  # shell data\n  echo "value"\n# 末尾\n';
  const ranges = commentRanges(body, 'workflow.yml');
  assert.deepEqual(ranges.map(range => body.slice(range.start, range.end)), ['# 注释', '# 末尾']);
});

test('TOML strings, NSIS escapes/continued comments and SQLite quoting retain executable text', () => {
  const toml = 'one = "# data" # 注释\ntwo = """\n# data\n"""\n';
  assert.deepEqual(commentRanges(toml, 'Cargo.toml').map(range => toml.slice(range.start, range.end)), ['# 注释']);
  const nsis = 'Name "$\\"; # data$\\"" ; 注释\n# continued \\\n  注释\n';
  assert.deepEqual(commentRanges(nsis, 'installer.nsi').map(range => nsis.slice(range.start, range.end)), ['; 注释', '# continued \\\n  注释']);
  const sql = "SELECT '-- data', 'it''s /* data */'; -- 注释\n/* 块 */";
  assert.deepEqual(commentRanges(sql, 'fixture.sql').map(range => sql.slice(range.start, range.end)), ['-- 注释', '/* 块 */']);
});

test('Python tokenization identifies docstrings and preserves other string values with Unicode offsets', () => {
  const body = '"""模块说明"""\nvalue = "# 原文😀" # 注释\ndef f():\n    """函数说明"""\n    return "原文"\n';
  assert.deepEqual(commentRanges(body, 'test.py').map(range => body.slice(range.start, range.end)),
    ['"""模块说明"""', '# 注释', '"""函数说明"""']);
});

test('NSIS comment parsing preserves Handlebars controls, values and quoted delimiters', () => {
  const body = '{{#if plugins}}\r\n!addplugindir "{{plugins}}" ; 注释\r\n{{/if}}\r\n'
    + '{{#each resources as |file| ~}}\r\nFile {{file}}\r\n{{/each}}\r\n'
    + '{{helper "}};#"}} {{{value}}}\r\nName "demo" # 末尾\r\n';
  const ranges = commentRanges(body, 'installer.nsi');
  assert.deepEqual(ranges.map(range => body.slice(range.start, range.end)), ['; 注释', '# 末尾']);
  const translated = body.replace('; 注释', '; Comment').replace('# 末尾', '# End');
  assert.equal(withoutComments(body, ranges), withoutComments(translated, commentRanges(translated, 'installer.nsi')));
  assert.notEqual(withoutComments(body, ranges), withoutComments(body.replace('{{#if plugins}}', '{{#if changed}}'),
    commentRanges(body.replace('{{#if plugins}}', '{{#if changed}}'), 'installer.nsi')));
  assert.throws(() => commentRanges('{{#if incomplete', 'installer.nsi'), /Unclosed NSIS template/);
});

test('the real installer template retains every conditional and loop during comment removal', async () => {
  const body = await readFile(new URL('../../../desktop/src-tauri/installer/windows.nsi', import.meta.url), 'utf8');
  const expressions = [...body.matchAll(/\{\{[#/][^}]+\}\}/g)];
  assert.ok(expressions.length >= 32);
  const code = withoutComments(body, commentRanges(body, 'installer.nsi'));
  for (const expression of expressions) assert.ok(code.includes(expression[0]), expression[0]);
});

test('PowerShell uses its native parser to distinguish comments, strings and here-strings', () => {
  const body = "$value = '# 原文' # 注释\n$text = @'\n# 原文\n'@\n<# 块注释 #>\n";
  assert.deepEqual(commentRanges(body, 'test.ps1').map(range => body.slice(range.start, range.end)), ['# 注释', '<# 块注释 #>']);
  const windows = body.replaceAll('\n', '\r\n');
  assert.deepEqual(commentRanges(windows, 'test.ps1').map(range => windows.slice(range.start, range.end)), ['# 注释', '<# 块注释 #>']);
});
