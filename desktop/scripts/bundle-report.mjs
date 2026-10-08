import { createHash } from 'node:crypto';
import { createReadStream } from 'node:fs';
import { readdir, stat, writeFile } from 'node:fs/promises';
import { join, relative, resolve } from 'node:path';
import { pathToFileURL } from 'node:url';

// Portable archives are validated before reporting; staging directories are excluded.
export async function reportBundles(directory, revision, metadata = {}) {
  const root = resolve(directory);
  const artifacts = [];
  async function visit(dir) {
    for (const entry of await readdir(dir, { withFileTypes: true })) {
      const path = join(dir, entry.name);
      if (entry.isDirectory()) {
        await visit(path);
      } else if (entry.isFile() && /(?:-setup\.exe|-portable\.tar\.zst)$/.test(entry.name)) {
        artifacts.push(path);
      }
    }
  }
  await visit(root);
  if (!artifacts.length) throw new Error('No release bundles found');
  const bundles = [];
  for (const path of [...new Set(artifacts)].sort()) {
    const hash = createHash('sha256');
    for await (const chunk of createReadStream(path)) hash.update(chunk);
    bundles.push({ path: relative(root, path).split('\\').join('/'), bytes: (await stat(path)).size, sha256: hash.digest('hex') });
  }
  const report = { revision, ...metadata, bundles };
  await writeFile(join(root, 'bundle-report.json'), JSON.stringify(report, null, 2) + '\n');
  return report;
}

if (process.argv[1] && import.meta.url === pathToFileURL(resolve(process.argv[1])).href) {
  if (process.argv.length !== 3) {
    console.error('Usage: node desktop/scripts/bundle-report.mjs <bundle-directory>');
    process.exitCode = 1;
  } else {
    try {
      console.log(JSON.stringify(await reportBundles(process.argv[2], process.env.GITHUB_SHA ?? 'local'), null, 2));
    } catch (error) {
      console.error(error.message);
      process.exitCode = 1;
    }
  }
}
