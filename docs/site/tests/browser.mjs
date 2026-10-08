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
const layouts = [];
const errors = [];
const outbound = [];
const contexts = [];
const pageFor = async options => {
  const context = await browser.newContext({ viewport: { width: 1440, height: 1000 }, ...options });
  contexts.push(context);
  const page = await context.newPage();
  page.setDefaultTimeout(15000);
  page.setDefaultNavigationTimeout(30000);
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
    await localized.locator(`#starlight__sidebar a[href="${prefix}/guide/sources/"]`).click();
    await localized.waitForURL(`${base}${prefix}/guide/sources/`);
    assert.equal(await localized.locator('#starlight__sidebar a[aria-current="page"]').getAttribute('href'), `${prefix}/guide/sources/`);
    checks.push(`${language} mobile navigation`);
  }
  const keyboard = await pageFor({ locale: 'en' });
  for (const language of ['en', 'zh-CN']) {
    const prefix = language === 'en' ? '' : '/zh-cn';
    const home = await pageFor({ locale: language, colorScheme: 'dark' });
    await home.goto(`${base}${prefix}/`);
    await home.waitForFunction(() => document.documentElement.dataset.theme === 'dark');
    const expectedTitle = language === 'en' ? 'AI usage dashboard' : 'AI 用量看板';
    assert.equal(await home.locator('h1').innerText(), expectedTitle);
    assert.equal(await home.locator('.site-title span').innerText(), expectedTitle);
    const download = home.locator('.hero .actions a').filter({ hasText: language === 'en' ? /^Download$/ : /^下载$/ });
    assert.equal(await download.getAttribute('href'), `${prefix}/start/installation/#downloads`);
    await download.click();
    await home.waitForURL(`${base}${prefix}/start/installation/#downloads`);
    assert.equal(await home.locator('a[href="https://github.com/owent/llm-usage/actions/workflows/ci.yml"]').count(), 1);
    await home.goto(`${base}${prefix}/`);
    checks.push(`${language} homepage title and download instructions`);
    const figures = home.locator('figure.app-screenshot');
    assert.equal(await figures.count(), 5, 'All five application pages have homepage examples');
    for (const theme of ['light', 'dark']) {
      await home.locator('starlight-theme-select select').first().selectOption(theme);
      await home.waitForFunction(theme => document.documentElement.dataset.theme === theme, theme);
      for (const [index, section] of ['overview', 'trend', 'details', 'sources', 'settings'].entries()) {
        const figure = figures.nth(index);
        await figure.scrollIntoViewIfNeeded();
        assert.equal(await figure.getAttribute('data-language'), language);
        const original = `/screenshots/${language}/${section}-${theme}.png`;
        const link = figure.locator('a:visible');
        assert.equal(await link.count(), 1, 'Only the selected theme is visible and focusable');
        assert.equal(await link.getAttribute('href'), original);
        const image = link.locator('img');
        assert.equal(await image.getAttribute('src'), original);
        assert.ok(await image.getAttribute('alt'), 'The screenshot has a readable description');
        await home.waitForFunction(path => {
          const img = document.querySelector(`figure.app-screenshot img[src="${path}"]`);
          return img?.complete && img.naturalWidth === 2880 && img.naturalHeight === 2000;
        }, original);
        const size = await image.evaluate(img => ({ width: img.clientWidth, height: img.clientHeight }));
        assert.ok(Math.abs(size.width / size.height - 2880 / 2000) < .02, 'The screenshot retains its original aspect ratio');
      }
      await home.evaluate(() => scrollTo(0, 0));
      await home.screenshot({ path: join(artifacts, `${language}-home-${theme}.png`), fullPage: true });
    }
    checks.push(`${language} homepage: five localized screenshots, manual theme selection, original links and PNG dimensions`);
    for (const width of [320, 390, 768]) {
      await home.setViewportSize({ width, height: 900 });
      await figures.last().scrollIntoViewIfNeeded();
      assert.ok(await home.evaluate(() => document.documentElement.scrollWidth <= innerWidth + 1), `Homepage has no document-wide overflow at ${width}px`);
      for (const figure of await figures.all()) {
        const box = await figure.boundingBox();
        assert.ok(box.x >= 0 && box.x + box.width <= width + 1, 'Screenshots and captions fit the viewport');
      }
      await home.evaluate(() => scrollTo(0, 0));
      await home.screenshot({ path: join(artifacts, `${language}-home-${width}.png`) });
    }
    await home.setViewportSize({ width: 1440, height: 1000 });
    const originalLink = figures.first().locator('a:visible');
    await originalLink.focus();
    await home.keyboard.press('Enter');
    await home.waitForURL(`${base}/screenshots/${language}/overview-dark.png`);
    await home.waitForFunction(() => document.querySelector('img')?.naturalWidth === 2880);
    checks.push(`${language} homepage: 320/390/768px layouts and keyboard access to the original image`);
    for (const [route, section] of [['reference/repository/readme', 'overview'], ['guide/dashboard', 'details']]) {
      await home.goto(`${base}${prefix}/${route}/`);
      const image = home.locator(`.sl-markdown-content img[src="/screenshots/${language}/${section}-light.png"]`);
      assert.equal(await image.count(), 1, 'The guide or README uses its matching-language screenshot');
      await image.scrollIntoViewIfNeeded();
      await home.waitForFunction(path => {
        const img = document.querySelector(`img[src="${path}"]`);
        return img?.complete && img.naturalWidth === 2880;
      }, `/screenshots/${language}/${section}-light.png`);
      assert.equal(await image.locator('..').getAttribute('href'), `/screenshots/${language}/${section}-light.png`);
    }
    checks.push(`${language} README and dashboard: localized examples and original-image links`);
    for (const width of [800, 1024, 1152, 1440, 1920, 2560]) {
      await home.setViewportSize({ width, height: 1000 });
      await home.goto(`${base}${prefix}/guide/dashboard/`);
      assert.ok(await home.evaluate(() => document.documentElement.scrollWidth <= innerWidth + 1), `Guide fits ${width}px`);
      const bodyWidth = await home.locator('.sl-markdown-content').evaluate(el => el.getBoundingClientRect().width);
      layouts.push({ language, viewport: width, content: bodyWidth });
      if (width >= 1920) assert.ok(bodyWidth > width * .6, 'Guide content expands on wide screens');
      if (width >= 1152) {
        const body = await home.locator('.main-pane').boundingBox();
        const toc = await home.locator('.right-sidebar-container').boundingBox();
        assert.ok(toc.x >= body.x + body.width - 1, 'Table of contents does not cover the guide');
      }
      if (width === 2560) await home.screenshot({ path: join(artifacts, `${language}-guide-wide.png`) });
    }
    await home.goto(`${base}${prefix}/`);
    assert.ok(await home.locator('figure.app-screenshot').first().evaluate(el => el.getBoundingClientRect().width) >= 1500, 'Homepage examples use wide-screen space');
    await home.screenshot({ path: join(artifacts, `${language}-home-wide.png`) });
    checks.push(`${language} wide-screen guides and homepage, breakpoint overflow and table of contents`);
    await home.setViewportSize({ width: 1440, height: 1000 });
    await home.goto(`${base}${prefix}/guide/dashboard/`);
    const groups = home.locator('#starlight__sidebar .top-level > li > details');
    assert.equal(await groups.count(), 7);
    for (const group of await groups.all()) {
      assert.ok(await group.locator('a').count() > 0, 'Every menu category contains links');
      const summary = group.locator(':scope > summary');
      const before = await group.evaluate(el => el.open);
      await summary.click();
      assert.equal(await group.evaluate(el => el.open), !before, 'Pointer opens and closes a category');
      await summary.focus();
      await home.keyboard.press('Enter');
      assert.equal(await group.evaluate(el => el.open), before, 'Keyboard toggles the category');
    }
    await home.locator(`#starlight__sidebar a[href="${prefix}/guide/sources/"]`).click();
    await home.waitForURL(`${base}${prefix}/guide/sources/`);
    assert.equal(await home.locator('#starlight__sidebar a[aria-current="page"]').getAttribute('href'), `${prefix}/guide/sources/`);
    checks.push(`${language} seven sidebar categories, pointer/keyboard toggling and page navigation`);
  }
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
  await writeFile(join(artifacts, 'report.json'), JSON.stringify({ checks, layouts, errors, outbound }, null, 2));
  await Promise.all(contexts.map(context => context.close()));
  await browser.close();
  await new Promise(done => server.close(done));
}
