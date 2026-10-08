import { readFile } from 'node:fs/promises';
import { resolve, relative } from 'node:path';
import { createHash } from 'node:crypto';
import { root, files } from './content.mjs';

export async function screenshotProblems(directory = resolve(root, 'docs/site/public')) {
  const problems = [];
  const metadata = JSON.parse(await readFile(resolve(directory, 'screenshots/provenance.json'), 'utf8'));
  if (!/^\d+\.\d+\.\d+/.test(metadata.version) || !/^[a-f0-9]{64}$/.test(metadata.executableSha256)) {
    problems.push('Screenshot provenance lacks a version or executable digest');
  }
  if (!Number.isFinite(Date.parse(metadata.capturedAt)) || !metadata.provenance.includes('synthetic')) {
    problems.push('Screenshot provenance lacks capture time or synthetic-data disclosure');
  }
  const expectedWidth = metadata.viewport.width * metadata.deviceScaleFactor;
  const expectedHeight = metadata.viewport.height * metadata.deviceScaleFactor;
  const paths = new Set();
  const combinations = new Set();
  for (const entry of metadata.screenshots) {
    if (!['en', 'zh-CN'].includes(entry.language) || !['light', 'dark'].includes(entry.theme)
      || !['overview', 'trend', 'sources', 'details', 'settings'].includes(entry.page)) {
      problems.push(`Unrecognized screenshot identity: ${entry.path}`);
      continue;
    }
    const expected = `screenshots/${entry.language}/${entry.page}-${entry.theme}.png`;
    if (entry.path !== expected || paths.has(entry.path)) problems.push(`Invalid or repeated screenshot path: ${entry.path}`);
    paths.add(entry.path);
    combinations.add(`${entry.language}/${entry.theme}/${entry.page}`);
    const body = await readFile(resolve(directory, expected));
    if (body.subarray(0, 8).toString('hex') !== '89504e470d0a1a0a') {
      problems.push(`Invalid PNG or unresolved Git LFS pointer: ${expected}`);
      continue;
    }
    if (body.readUInt32BE(16) !== expectedWidth || body.readUInt32BE(20) !== expectedHeight) {
      problems.push(`Screenshot dimensions differ from capture provenance: ${expected}`);
    }
    if (createHash('sha256').update(body).digest('hex') !== entry.sha256) problems.push(`Screenshot changed without provenance: ${expected}`);
  }
  if (combinations.size !== 20) problems.push('Missing language, theme or page screenshot');
  for (const path of (await files(resolve(directory, 'screenshots'))).filter(path => path.endsWith('.png'))) {
    const key = relative(directory, path).replaceAll('\\', '/');
    if (!paths.has(key)) problems.push(`Unrecorded screenshot: ${key}`);
  }
  return problems;
}
