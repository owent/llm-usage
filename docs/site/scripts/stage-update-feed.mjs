import { readFile, writeFile, mkdir } from 'node:fs/promises';
import { resolve } from 'node:path';
import { output } from './content.mjs';
import { validateFeed, snapshotPath, refreshedPath } from './update-feed.mjs';

let text;
try {
  text = await readFile(refreshedPath, 'utf8');
} catch (error) {
  if (error.code !== 'ENOENT') throw error;
  text = await readFile(snapshotPath, 'utf8');
}
const feed = validateFeed(JSON.parse(text), { fresh: false });
const directory = resolve(output, 'updates');
await mkdir(directory, { recursive: true });
await writeFile(resolve(directory, 'latest.json'), JSON.stringify(feed, null, 2) + '\n');
console.log(`Included verified update metadata for ${feed.release.tag_name}.`);
