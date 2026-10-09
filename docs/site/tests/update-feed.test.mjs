import test from 'node:test';
import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import { parse } from 'yaml';
import {
  publishedRelease,
  createFeed,
  validateFeed,
  refreshFeed,
  requestJson,
  releaseApi,
  feedUrl,
  maxFeedAge,
  snapshotPath,
  refreshedPath,
} from '../scripts/update-feed.mjs';

const version = '0.4.0';
const now = Date.parse('2026-10-09T00:00:00.000Z');
function fixture() {
  const names = ['windows', 'linux', 'macos'].flatMap((os) =>
    ['x64', 'arm64'].map((arch) => `LLMUsage-${version}-${os}-${arch}-portable.tar.zst`)
  );
  names.push(`LLMUsage_${version}_x64-setup.exe`);
  return {
    tag_name: 'v' + version,
    draft: false,
    prerelease: false,
    author: { token: 'never copied' },
    body: 'never copied',
    assets: names.map((name) => ({
      name,
      state: 'uploaded',
      size: 123,
      digest: 'sha256:' + 'a'.repeat(64),
      browser_download_url: `https://github.com/owent/llm-usage/releases/download/v${version}/${name}`,
      uploader: { login: 'never copied' },
    })),
  };
}

test('snapshot stores all seven exact packages and only public update fields', () => {
  const feed = createFeed(fixture(), now);
  assert.equal(feed.release.assets.length, 7);
  assert.ok(!JSON.stringify(feed).includes('never copied'));
  assert.deepEqual(Object.keys(feed.release.assets[0]), [
    'name',
    'state',
    'size',
    'digest',
    'browser_download_url',
  ]);
  assert.ok(snapshotPath.replaceAll('\\', '/').endsWith('/llm-usage/docs/site/public/updates/latest.json'));
  assert.ok(
    refreshedPath
      .replaceAll('\\', '/')
      .endsWith('/llm-usage/build/documentation-site/update-feed/latest.json')
  );
});

test('partial uploads, duplicate assets, digests, URLs and unstable tags cannot publish', () => {
  const invalid = [
    (value) => value.assets.pop(),
    (value) => value.assets.push(value.assets[0]),
    (value) => (value.assets[0].digest = null),
    (value) => (value.assets[0].size = 0),
    (value) => (value.assets[0].state = 'new'),
    (value) => (value.assets[0].browser_download_url = 'https://evil.test/package'),
    (value) => (value.tag_name = 'v0.4.0-preview'),
    (value) => (value.prerelease = true),
    (value) => (value.draft = true),
  ];
  for (const change of invalid) {
    const value = fixture();
    change(value);
    assert.throws(() => publishedRelease(value));
  }
});

test('snapshot expiry is bounded without changing the original generation time', () => {
  const feed = createFeed(fixture(), now);
  assert.deepEqual(validateFeed(feed, { now: now + maxFeedAge }), feed);
  assert.throws(() => validateFeed(feed, { now: now + maxFeedAge + 1 }));
  assert.throws(() => validateFeed(feed, { now: now - 300001 }));
  assert.throws(() => validateFeed({ ...feed, repository: 'other/repository' }, { now }));
  assert.throws(() => validateFeed({ ...feed, schema: 2 }, { now }));
  assert.throws(() => validateFeed({ ...feed, extra: true }, { now }));
});

test('primary success reads once; failure reuses a valid site snapshot without renewing it', async () => {
  const requests = [];
  const feed = await refreshFeed({
    now,
    request: async (url, auth) => {
      requests.push([url, auth]);
      return fixture();
    },
  });
  assert.equal(feed.generated_at, new Date(now).toISOString());
  assert.deepEqual(requests, [[releaseApi, true]]);
  const older = createFeed(fixture(), now - 3600000);
  requests.length = 0;
  const reused = await refreshFeed({
    now,
    request: async (url, auth) => {
      requests.push([url, auth]);
      if (url === releaseApi) throw new Error('HTTP 429');
      return older;
    },
  });
  assert.equal(reused.generated_at, older.generated_at);
  assert.deepEqual(requests, [
    [releaseApi, true],
    [feedUrl, false],
  ]);
});

test('failed requests retain only a fresh committed snapshot and otherwise reject publication', async () => {
  const snapshot = createFeed(fixture(), now - 3600000);
  const request = async () => {
    throw new Error('unavailable');
  };
  assert.equal((await refreshFeed({ now, request, snapshot })).generated_at, snapshot.generated_at);
  await assert.rejects(refreshFeed({ now: now + maxFeedAge, request, snapshot }));
});

test('committed public snapshot meets the same seven-package contract', async () => {
  const snapshot = JSON.parse(await readFile(snapshotPath, 'utf8'));
  assert.equal(validateFeed(snapshot, { now: Date.parse(snapshot.generated_at) }).release.assets.length, 7);
});

test('bounded metadata transport keeps the CI token on GitHub and refuses HTTP failures', async () => {
  const requests = [];
  const fetcher = async (url, options) => {
    requests.push([url, options]);
    return new Response(JSON.stringify(fixture()));
  };
  const options = { fetcher, token: 'synthetic-test-token' };
  await requestJson(releaseApi, true, options);
  await requestJson(feedUrl, true, options);
  assert.equal(requests[0][1].headers.Authorization, 'Bearer synthetic-test-token');
  assert.equal(requests[1][1].headers.Authorization, undefined);
  assert.equal(requests[0][1].redirect, 'error');
  assert.ok(requests[0][1].signal instanceof AbortSignal);
  await assert.rejects(
    requestJson(releaseApi, false, { fetcher: async () => new Response('', { status: 429 }) }),
    /http_429/
  );
  await assert.rejects(
    requestJson(feedUrl, false, { fetcher: async () => new Response('x'.repeat(1024 * 1024 + 1)) }),
    /too_large/
  );
});

test('release refresh dispatches main rather than deploying a tag under the Pages environment', async () => {
  const workflow = parse(
    await readFile(new URL('../../../.github/workflows/docs.yml', import.meta.url), 'utf8')
  );
  assert.deepEqual(workflow.on.release.types, ['published', 'edited', 'unpublished', 'deleted']);
  assert.equal(workflow.on.schedule[0].cron, '17 4 * * *');
  assert.equal(workflow.jobs.build.if, "github.event_name != 'release'");
  assert.equal(workflow.jobs.refresh_from_release.permissions.actions, 'write');
  assert.ok(workflow.jobs.refresh_from_release.steps[0].with.script.includes("ref: 'main'"));
  assert.equal(workflow.jobs.publish.environment.name, 'github-pages');
  assert.ok(workflow.jobs.publish.if.includes("github.ref == 'refs/heads/main'"));
  const step = workflow.jobs.build.steps.find(
    (step) => step.name === 'Refresh verified public update metadata'
  );
  assert.equal(step.if, "github.event_name != 'pull_request'");
});
