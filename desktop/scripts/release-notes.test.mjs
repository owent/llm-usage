import assert from 'node:assert/strict';
import test from 'node:test';
import { tagReleaseBody } from './release-notes.mjs';

const repo = { owner: 'test-owner', repo: 'test-repo' };
const commit = '1'.repeat(40);
const tag = 'v0.3.3';

function api(message, { target = { type: 'commit', sha: commit }, lightweight = false, nested = false } = {}) {
  return {
    rest: {
      git: {
        async getRef(input) {
          assert.deepEqual(input, { ...repo, ref: `tags/${tag}` });
          return { data: { object: lightweight ? target : { type: 'tag', sha: 'annotation' } } };
        },
        async getTag(input) {
          assert.deepEqual({ owner: input.owner, repo: input.repo }, repo);
          if (input.tag_sha === 'annotation') {
            return { data: { message, object: nested ? { type: 'tag', sha: 'inner' } : target } };
          }
          assert.equal(input.tag_sha, 'inner');
          return { data: { message: 'Older tag notes must not replace the release annotation.', object: target } };
        },
      },
    },
  };
}

test('preserves the complete annotation, including Markdown, Unicode and literal expressions', async () => {
  const message = '## CHANGELOG\n\n- Fix `tokens`.\n- 保留换行。\n\n${{ github.token }} $(echo literal)\n';
  assert.equal(await tagReleaseBody(api(message), repo, tag, commit), message);
});

test('uses the outer tag message when resolving nested tags', async () => {
  const message = '## CHANGELOG\n\n- Current release.\n';
  assert.equal(await tagReleaseBody(api(message, { nested: true }), repo, tag, commit), message);
});

test('rereads the annotation when rerunning the same release', async () => {
  const first = '## CHANGELOG\n\n- First notes.\n';
  const second = '## CHANGELOG\n\n- Updated notes.\n';
  assert.equal(await tagReleaseBody(api(first), repo, tag, commit), first);
  assert.equal(await tagReleaseBody(api(second), repo, tag, commit), second);
});

test('rejects a lightweight tag instead of substituting a commit message', async () => {
  await assert.rejects(tagReleaseBody(api(undefined, { lightweight: true }), repo, tag, commit), /annotated tag/);
});

test('rejects missing, empty and whitespace-only annotations', async () => {
  for (const message of [undefined, null, '', ' \r\n\t']) {
    await assert.rejects(tagReleaseBody(api(message), repo, tag, commit), /nonempty message/);
  }
});

test('rejects a moved tag or a non-commit target', async () => {
  for (const target of [{ type: 'commit', sha: '2'.repeat(40) }, { type: 'tree', sha: commit }]) {
    await assert.rejects(tagReleaseBody(api('Notes', { target, nested: true }), repo, tag, commit), /Tag changed/);
  }
});

test('does not replace a failed GitHub read with generated notes', async () => {
  const github = api('Notes');
  github.rest.git.getTag = async () => { throw new Error('GitHub unavailable'); };
  await assert.rejects(tagReleaseBody(github, repo, tag, commit), /GitHub unavailable/);
});
