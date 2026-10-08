import assert from 'node:assert/strict';
import { createServer } from 'node:http';
import { once } from 'node:events';
import { mkdir, readFile, writeFile, stat } from 'node:fs/promises';
import { resolve, join, extname, sep } from 'node:path';
import { chromium } from '../../../desktop/node_modules/playwright/index.mjs';
import { output, root } from '../scripts/content.mjs';

const artifacts = resolve(root, 'build/documentation-site/browser', String(Date.now()));
await mkdir(artifacts, { recursive: true });
const mime = { '.html': 'text/html; charset=utf-8', '.js': 'text/javascript', '.css': 'text/css', '.json': 'application/json', '.svg': 'image/svg+xml', '.png': 'image/png', '.wasm': 'application/wasm', '.xml': 'application/xml' };
const server = createServer(async (request, response) => {
  try {
    const pathname = decodeURIComponent(new URL(request.url, 'http://localhost').pathname);
    let path = resolve(output, `.${pathname}`);
    if (path !== output && !path.startsWith(output + sep)) throw new Error('Invalid path');
    if ((await stat(path)).isDirectory()) path = join(path, 'index.html');
    const body = await readFile(path);
    response.writeHead(200, { 'Content-Type': mime[extname(path)] ?? 'application/octet-stream' });
    response.end(body);
  } catch { response.writeHead(404); response.end('Not found'); }
});
server.listen(0, '127.0.0.1');
await once(server, 'listening');
const base = `http://127.0.0.1:${server.address().port}`;
const browser = await chromium.launch(process.platform === 'win32' ? { channel: 'msedge', headless: true } : { headless: true });
const checks = [];
const errors = [];
const outbound = [];
const contexts = [];
const pageFor = async options => {
  const context = await browser.newContext({ viewport: { width: 1440, height: 1000 }, ...options });
  contexts.push(context);
  const page = await context.newPage();
  page.on('pageerror', error => errors.push(error.message));
  page.on('request', request => { if (!request.url().startsWith(base) && !request.url().startsWith('data:')) outbound.push(request.url()); });
  return page;
};
try {
  for (const [locale, expected] of [['en-US', '/'], ['zh-CN', '/zh-cn/'], ['zh-TW', '/zh-cn/'], ['fr-FR', '/']]) {
    const page = await pageFor({ locale });
    await page.goto(`${base}/`);
    await page.waitForURL(`${base}${expected}`);
    assert.equal(await page.locator('html').getAttribute('lang'), expected === '/' ? 'en' : 'zh-CN');
    checks.push(`initial locale ${locale} → ${expected}`);
  }
  const page = await pageFor({ locale: 'zh-CN' });
  await page.goto(`${base}/guide/dashboard/?example=1#tokens-calls-and-observations`);
  assert.equal(new URL(page.url()).pathname, '/guide/dashboard/', 'An explicit English deep link is retained');
  await page.locator('starlight-lang-select select').first().selectOption('/zh-cn/guide/dashboard/');
  await page.waitForURL(/\/zh-cn\/guide\/dashboard\//);
  assert.equal(await page.evaluate(() => localStorage.getItem('llm-usage.docs.language')), 'zh-CN');
  assert.equal(new URL(page.url()).search, '?example=1');
  assert.equal(new URL(page.url()).hash, '#tokens-calls-and-observations');
  await page.locator('starlight-lang-select select').first().selectOption('/guide/dashboard/');
  await page.waitForURL(/(?<!zh-cn)\/guide\/dashboard\//);
  await page.goto(`${base}/`);
  assert.equal(new URL(page.url()).pathname, '/', 'Explicit English preference overrides Chinese browser locale');
  checks.push('deep links, paired-language switching, query/fragment and saved preference');

  const noStorage = await pageFor({ locale: 'zh-CN' });
  await noStorage.addInitScript(() => {
    Storage.prototype.getItem = () => { throw new DOMException('Unavailable', 'SecurityError'); };
    Storage.prototype.setItem = () => { throw new DOMException('Unavailable', 'SecurityError'); };
  });
  await noStorage.goto(`${base}/`);
  await noStorage.waitForURL(`${base}/zh-cn/`);
  await noStorage.locator('starlight-lang-select select').first().selectOption('/');
  await noStorage.waitForURL(`${base}/?lang=en`);
  assert.equal(await noStorage.locator('html').getAttribute('lang'), 'en');
  checks.push('storage denial preserves deliberate language choice without a redirect loop');

  const noScript = await pageFor({ locale: 'zh-CN', javaScriptEnabled: false });
  await noScript.goto(`${base}/`);
  assert.equal(await noScript.locator('html').getAttribute('lang'), 'en');
  checks.push('no-JavaScript root remains English');

  for (const language of ['en', 'zh-CN']) {
    const prefix = language === 'en' ? '' : '/zh-cn';
    const localized = await pageFor({ locale: language, colorScheme: 'dark' });
    await localized.goto(`${base}${prefix}/guide/dashboard/`);
    await localized.waitForFunction(() => document.documentElement.dataset.theme === 'dark');
    for (const theme of ['light', 'dark']) {
      await localized.locator('starlight-theme-select select').first().selectOption(theme);
      await localized.waitForFunction(theme => document.documentElement.dataset.theme === theme, theme);
      await localized.screenshot({ path: join(artifacts, `${language}-${theme}.png`), fullPage: true });
    }
    await localized.locator('site-search button[data-open-modal]').click();
    await localized.locator('.pagefind-ui__search-input').fill(language === 'en' ? 'cache' : '缓存');
    await localized.locator('.pagefind-ui__result-link').first().waitFor();
    const href = await localized.locator('.pagefind-ui__result-link').first().getAttribute('href');
    assert.ok(language === 'en' ? !href.includes('/zh-cn/') : href.includes('/zh-cn/'), 'Search results use the selected language');
    await localized.keyboard.press('Escape');
    checks.push(`${language} themes and localized local search`);
    await localized.setViewportSize({ width: 390, height: 844 });
    await localized.reload();
    assert.ok(await localized.evaluate(() => document.documentElement.scrollWidth <= innerWidth + 1), 'Mobile page has no document-wide overflow');
    await localized.locator('button[popovertarget="starlight__sidebar"]').click();
    await localized.locator('#starlight__sidebar').waitFor({ state: 'visible' });
    await localized.screenshot({ path: join(artifacts, `${language}-mobile.png`) });
    checks.push(`${language} mobile navigation`);
  }
  const keyboard = await pageFor({ locale: 'en' });
  await keyboard.goto(`${base}/guide/dashboard/`);
  await keyboard.keyboard.press('Tab');
  assert.equal(await keyboard.evaluate(() => document.activeElement?.getAttribute('href')), '#_top', 'Skip link is first keyboard target');
  await keyboard.keyboard.press('Enter');
  await keyboard.keyboard.press('Control+k');
  await keyboard.locator('site-search dialog').waitFor({ state: 'visible' });
  await keyboard.keyboard.press('Escape');
  checks.push('keyboard skip link and search shortcut');
  assert.deepEqual(errors, [], 'No browser JavaScript errors');
  assert.deepEqual(outbound, [], 'Documentation makes no third-party runtime requests');
  console.log(`Passed ${checks.length} documentation browser checks.`);
} finally {
  await writeFile(join(artifacts, 'report.json'), JSON.stringify({ checks, errors, outbound }, null, 2));
  await Promise.all(contexts.map(context => context.close()));
  await browser.close();
  await new Promise(done => server.close(done));
}
