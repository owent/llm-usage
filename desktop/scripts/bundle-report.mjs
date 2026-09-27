import { createHash } from 'node:crypto';
import { createReadStream } from 'node:fs';
import { readdir, stat, writeFile } from 'node:fs/promises';
import { basename, dirname, join, relative, resolve } from 'node:path';
import { spawnSync } from 'node:child_process';
import { pathToFileURL } from 'node:url';

// macOS .app is a directory. Archive it before upload to preserve modes/symlinks.
export async function reportBundles(directory, revision) {
  const root = resolve(directory);
  const artifacts = [];
  async function visit(dir) {
    for (const entry of await readdir(dir, { withFileTypes: true })) {
      const path = join(dir, entry.name);
      if (entry.isDirectory() && entry.name.endsWith('.app')) {
        const archive = `${path}.tar.gz`;
        // A relative archive name avoids Windows drive-colon interpretation
        // without GNU-only flags (macOS and Windows ship BSD tar).
        const result = spawnSync('tar', ['-czf', basename(archive), basename(path)], {
          cwd: dirname(path), encoding: 'utf8', timeout: 120_000, windowsHide: true,
        });
        if (result.error || result.status !== 0) throw new Error(`App archive failed: ${result.error?.message ?? result.stderr}`);
        artifacts.push(archive);
      } else if (entry.isDirectory()) {
        await visit(path);
      } else if (entry.isFile() && /\.(?:exe|msi|deb|AppImage|dmg|tar\.gz)$/.test(entry.name)) {
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
  const report = { revision, bundles };
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
