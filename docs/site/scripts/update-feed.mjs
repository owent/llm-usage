import { mkdir, readFile, writeFile } from 'node:fs/promises';
import { resolve, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';

export const releaseApi = 'https://api.github.com/repos/owent/llm-usage/releases/latest';
export const feedUrl = 'https://llm-usage.atframe.work/updates/latest.json';
export const maxFeedAge = 7 * 86400000;
const maxBytes = 1024 * 1024;
const root = fileURLToPath(new URL('../../../', import.meta.url));
export const snapshotPath = resolve(root, 'docs/site/public/updates/latest.json');
export const refreshedPath = resolve(root, 'build/documentation-site/update-feed/latest.json');

export function publishedRelease(value) {
  if (
    !value ||
    value.draft !== false ||
    value.prerelease !== false ||
    !/^v(?:0|[1-9]\d*)\.(?:0|[1-9]\d*)\.(?:0|[1-9]\d*)$/.test(value.tag_name) ||
    !Array.isArray(value.assets)
  )
    throw new Error('invalid_stable_release');
  const version = value.tag_name.slice(1);
  if (version.split('.').some((part) => !Number.isSafeInteger(Number(part))))
    throw new Error('invalid_release_version');
  const names = ['windows', 'linux', 'macos'].flatMap((os) =>
    ['x64', 'arm64'].map((arch) => `LLMUsage-${version}-${os}-${arch}-portable.tar.zst`)
  );
  names.push(`LLMUsage_${version}_x64-setup.exe`);
  const assets = names.map((name) => {
    const matches = value.assets.filter((asset) => asset.name === name);
    if (matches.length !== 1) throw new Error('missing_or_duplicate_release_asset');
    const asset = matches[0];
    const url = `https://github.com/owent/llm-usage/releases/download/v${version}/${name}`;
    if (
      asset.state !== 'uploaded' ||
      !Number.isSafeInteger(asset.size) ||
      asset.size <= 0 ||
      asset.size > 512 * 1024 * 1024 ||
      !/^sha256:[a-f0-9]{64}$/.test(asset.digest) ||
      asset.browser_download_url !== url
    )
      throw new Error('invalid_release_asset');
    return { name, state: 'uploaded', size: asset.size, digest: asset.digest, browser_download_url: url };
  });
  return { tag_name: value.tag_name, draft: false, prerelease: false, assets };
}

export function validateFeed(value, { now = Date.now(), fresh = true } = {}) {
  if (
    value?.schema !== 1 ||
    value.repository !== 'owent/llm-usage' ||
    Object.keys(value).some((key) => !['schema', 'repository', 'generated_at', 'release'].includes(key))
  )
    throw new Error('invalid_update_feed');
  const generated = Date.parse(value.generated_at);
  if (
    !Number.isFinite(generated) ||
    new Date(generated).toISOString() !== value.generated_at ||
    generated > now + 300000 ||
    (fresh && now - generated > maxFeedAge)
  )
    throw new Error('stale_or_invalid_update_feed');
  return { ...value, release: publishedRelease(value.release) };
}

export function createFeed(release, now = Date.now()) {
  return validateFeed(
    {
      schema: 1,
      repository: 'owent/llm-usage',
      generated_at: new Date(now).toISOString(),
      release: publishedRelease(release),
    },
    { now }
  );
}

export async function requestJson(
  url,
  authenticated = false,
  {
    fetcher = fetch,
    token = process.env.GITHUB_ACTIONS === 'true' ? process.env.GITHUB_TOKEN : undefined,
  } = {}
) {
  const headers = { Accept: 'application/json', 'User-Agent': 'llm-usage-documentation-update-feed' };
  if (authenticated && url === releaseApi && token) headers.Authorization = `Bearer ${token}`;
  const response = await fetcher(url, { headers, redirect: 'error', signal: AbortSignal.timeout(30000) });
  if (response.status !== 200) throw new Error('update_feed_http_' + response.status);
  const chunks = [];
  let size = 0;
  for await (const chunk of response.body) {
    size += chunk.length;
    if (size > maxBytes) {
      await response.body.cancel().catch(() => {});
      throw new Error('update_feed_too_large');
    }
    chunks.push(chunk);
  }
  return JSON.parse(Buffer.concat(chunks).toString('utf8'));
}

export async function refreshFeed({ request = requestJson, now = Date.now(), snapshot } = {}) {
  try {
    return createFeed(await request(releaseApi, true), now);
  } catch (primary) {
    try {
      return validateFeed(await request(feedUrl, false), { now });
    } catch {
      if (snapshot) return validateFeed(snapshot, { now });
      throw new Error('update_feed_refresh_failed', { cause: primary });
    }
  }
}

async function main() {
  const args = process.argv.slice(2);
  const at = args.indexOf('--output');
  const output = at < 0 ? refreshedPath : resolve(args[at + 1]);
  let snapshot;
  try {
    snapshot = JSON.parse(await readFile(snapshotPath, 'utf8'));
  } catch {}
  const feed = await refreshFeed({ snapshot });
  await mkdir(dirname(output), { recursive: true });
  await writeFile(output, JSON.stringify(feed, null, 2) + '\n');
  console.log(
    `Stored ${feed.release.tag_name}: seven verified package references, generated ${feed.generated_at}.`
  );
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url))
  main().catch(() => {
    console.error(
      'Could not refresh verified public update metadata. Existing publication remains unchanged.'
    );
    process.exitCode = 1;
  });
