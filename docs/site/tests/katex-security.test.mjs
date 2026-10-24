import assert from 'node:assert/strict';
import test from 'node:test';
import katex from 'katex';

test('KaTeX renderToString renders ordinary math', () => {
  const html = katex.renderToString('x^2', { displayMode: true });
  assert.ok(html.includes('<span'));
  assert.ok(html.includes('katex'));
});

test('KaTeX ignores an inherited trust option', () => {
  Object.prototype.trust = true;
  try {
    const html = katex.renderToString('\\href{https://example.com}{x}', { displayMode: false });
    assert.equal(html.includes('<a '), false, 'inherited trust must not enable links');
  } finally {
    delete Object.prototype.trust;
  }
});
