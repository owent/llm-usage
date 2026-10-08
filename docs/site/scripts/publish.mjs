import { execFileSync } from 'node:child_process';
import { mkdir, readFile, writeFile, readdir, rm, cp, realpath, lstat } from 'node:fs/promises';
import { resolve, join, sep } from 'node:path';
import { configurePages, configuredPages, configurePublicationEnvironment } from './pages-config.mjs';
import { verifyPublication } from './publication-checks.mjs';

const root = resolve(import.meta.dirname, '../../..');
const args = process.argv.slice(2);
if (args.includes('--help')) {
  console.log('Publish a checked static site to gh-pages and request/verify its GitHub Pages build.\nUsage: node docs/site/scripts/publish.mjs [--dry-run] [--configure-pages] [--configure-environment] [--source-revision <full-sha>]\nRequires an authorized origin and GitHub CLI authentication. Writes the gh-pages branch; no force push.\n--configure-pages requires an administrator for first-time Pages setup.\n--configure-environment separately authorizes adding the exact main branch to custom github-pages deployment rules; other rules and protections remain unchanged.\nOrdinary CI only uses contents/pages permissions. Output must already exist in build/documentation-site/dist/. Temporary worktree stays under root build/.');
  process.exit(0);
}
const dryRun = args.includes('--dry-run');
const initialConfiguration = args.includes('--configure-pages');
const environmentConfiguration = args.includes('--configure-environment');
const sourceIndex = args.indexOf('--source-revision');
const sourceRevision = sourceIndex < 0 ? process.env.GITHUB_SHA : args[sourceIndex + 1];
if (!/^[0-9a-f]{40}$/.test(sourceRevision ?? '')) throw new Error('Provide a full source revision');
const exec = (command, argv, cwd = root, timeout = 60000) => execFileSync(command, argv, {
  cwd, encoding: 'utf8', timeout, stdio: ['ignore', 'pipe', 'pipe'],
  env: { ...process.env, GIT_TERMINAL_PROMPT: '0', GH_PROMPT_DISABLED: '1' },
}).trim();
const git = (argv, cwd) => exec('git', argv, cwd);
const origin = git(['remote', 'get-url', 'origin']);
if (!/^(git@github\.com:owent\/llm-usage\.git|https:\/\/github\.com\/owent\/llm-usage(?:\.git)?)$/.test(origin)) throw new Error('Unexpected publication origin');
const distribution = await realpath(resolve(root, 'build/documentation-site/dist'));
const buildRoot = await realpath(resolve(root, 'build'));
if (!distribution.startsWith(buildRoot + sep)) throw new Error('Distribution escaped the build directory');
await verifyPublication(distribution, sourceRevision);
for (const file of ['index.html', 'zh-cn/index.html', 'CNAME', '.nojekyll']) await readFile(join(distribution, file));
if ((await readFile(join(distribution, 'CNAME'), 'utf8')).trim() !== 'llm-usage.atframe.work') throw new Error('Unexpected custom domain');
const head = git(['ls-remote', 'origin', 'refs/heads/main']).split(/\s/)[0];
if (head !== sourceRevision) { console.log('Skipping outdated documentation build; main has moved.'); process.exit(0); }
if (!initialConfiguration) configuredPages();
if (dryRun) { console.log(`Ready to publish checked output for ${sourceRevision} to owent/llm-usage gh-pages and request a Pages build.`); process.exit(0); }
const task = resolve(root, 'build/documentation-site/publication', String(Date.now()));
await mkdir(task, { recursive: true });
const worktree = join(task, 'gh-pages');
const remote = git(['ls-remote', '--heads', 'origin', 'gh-pages']);
let registered = false;
try {
  if (remote) {
    git(['fetch', 'origin', 'gh-pages']);
    git(['worktree', 'add', '--detach', worktree, 'FETCH_HEAD']);
    registered = true;
    const marker = JSON.parse(await readFile(join(worktree, '.llm-usage-docs.json'), 'utf8'));
    if (marker.owner !== 'owent/llm-usage-documentation') throw new Error('Existing gh-pages is not owned by this publisher');
    const actual = await realpath(worktree);
    if (!actual.startsWith(buildRoot + sep)) throw new Error('Publication worktree escaped the build directory');
    for (const entry of await readdir(worktree)) {
      if (entry === '.git') continue;
      const target = join(worktree, entry);
      if ((await lstat(target)).isSymbolicLink()) throw new Error('Refusing a symbolic link in publication output');
      await rm(target, { recursive: true, force: true });
    }
  } else {
    // The isolated worktree inherits checkout authentication without reading or copying credentials.
    git(['worktree', 'add', '--orphan', '-b', 'gh-pages', worktree]);
    registered = true;
  }
  await cp(distribution, worktree, { recursive: true, force: true });
  await writeFile(join(worktree, '.llm-usage-docs.json'), JSON.stringify({ owner: 'owent/llm-usage-documentation', sourceRevision }, null, 2) + '\n');
  // Compiled branch assets must be ordinary files: GitHub Pages does not expand LFS pointers.
  await writeFile(join(worktree, '.gitattributes'), '* -filter\n');
  git(['add', '--all'], worktree);
  if (git(['status', '--porcelain'], worktree)) git(['-c', 'user.name=github-actions[bot]',
    '-c', 'user.email=41898282+github-actions[bot]@users.noreply.github.com',
    'commit', '-m', `Publish documentation from ${sourceRevision}`], worktree);
  const published = git(['rev-parse', 'HEAD'], worktree);
  // No force push: a concurrent unexpected update fails instead of discarding its history.
  git(['push', 'origin', 'HEAD:refs/heads/gh-pages'], worktree);
  const actual = git(['ls-remote', 'origin', 'refs/heads/gh-pages']).split(/\s/)[0];
  if (actual !== published) throw new Error('Published branch revision did not match');
  if (initialConfiguration) configurePages();
  if (environmentConfiguration) configurePublicationEnvironment();
  exec('gh', ['api', '--method', 'POST', 'repos/owent/llm-usage/pages/builds']);
  const deadline = Date.now() + 600000;
  for (let attempt = 0; attempt < 60; attempt++) {
    const remaining = deadline - Date.now();
    if (remaining <= 0) throw new Error('Pages build did not complete within ten minutes; inspect existing build before retrying');
    const build = JSON.parse(exec('gh', ['api', 'repos/owent/llm-usage/pages/builds/latest'], root, Math.min(60000, remaining)));
    if (build.commit === published && build.status === 'built') {
      console.log(`Published gh-pages ${published}; Pages build completed for source ${sourceRevision}.`);
      process.exitCode = 0;
      break;
    }
    if (build.commit === published && build.status === 'errored') throw new Error(`Pages build failed: ${build.error?.message ?? 'unknown error'}`);
    if (attempt === 59) throw new Error('Pages build did not complete within ten minutes; inspect existing build before retrying');
    await new Promise(done => setTimeout(done, Math.min(10000, Math.max(0, deadline - Date.now()))));
  }
} finally {
  if (registered) git(['worktree', 'remove', '--force', worktree]);
}
