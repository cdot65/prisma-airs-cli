/** Opt-in Chromium E2E; supply an installed puppeteer-core module and browser executable. */
import assert from 'node:assert/strict';
import { mkdir, mkdtemp, readFile, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { pathToFileURL } from 'node:url';
import { renderRuntimeReportHtml } from '../../dist/index.js';
import { exampleReport } from './generate-runtime-report-example.mjs';

const { default: puppeteer } = await import(process.env.REPORT_BROWSER_MODULE ?? 'puppeteer-core');
const directory = await mkdtemp(join(tmpdir(), 'airs-report-browser-'));
const results = [];
const errors = [];
const network = [];
const browser = await puppeteer.launch({
  executablePath: process.env.REPORT_CHROMIUM_EXECUTABLE ?? '/usr/bin/chromium',
  headless: true,
  args: [
    '--disable-dev-shm-usage',
    ...(process.env.REPORT_BROWSER_NO_SANDBOX === '1' ? ['--no-sandbox'] : []),
  ],
  env: Object.fromEntries(
    Object.entries(process.env).filter(([key]) => !/PANW_|SECRET|TOKEN|PASSWORD|API_KEY/.test(key)),
  ),
});
try {
  const page = await browser.newPage();
  await page.setRequestInterception(true);
  page.on('request', (request) => {
    if (request.url().startsWith('file:')) void request.continue();
    else {
      network.push('External request blocked');
      void request.abort();
    }
  });
  page.on('pageerror', () => errors.push('Browser JavaScript error'));
  page.on('console', (message) => {
    if (message.type() === 'error') errors.push('Browser console error');
  });
  const fixture = await exampleReport();
  const path = join(directory, 'fixture.html');
  await writeFile(path, renderRuntimeReportHtml(fixture));
  await page.setViewport({ width: 1440, height: 1000 });
  await page.goto(pathToFileURL(path).href);
  assert.equal(await page.$eval('h1', (element) => element.textContent), fixture.title);
  assert.equal(await page.$eval('html', (element) => element.classList.contains('js-ready')), true);
  results.push('Standalone HTML and CSP-pinned JavaScript render');
  await page.type('#app-search', 'Engineering');
  assert.equal(await page.$$eval('#applications tbody tr:not([hidden])', (rows) => rows.length), 1);
  await page.select('#finding-filter', 'attention');
  assert.equal(await page.$$eval('.finding:not([hidden])', (rows) => rows.length), 1);
  results.push('Application search and priority filtering');
  await page.emulateMediaType('print');
  assert.equal(
    await page.$$eval(
      '#applications tbody tr',
      (rows) => rows.filter((row) => getComputedStyle(row).display !== 'none').length,
    ),
    3,
  );
  await page.pdf({ path: join(directory, 'print.pdf'), format: 'A4' });
  results.push('Print reveals all filtered evidence and generates PDF');
  await page.emulateMediaType('screen');
  await page.setViewport({ width: 390, height: 844 });
  assert.equal(
    await page.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth),
    true,
  );
  results.push('Mobile layout has no document-level horizontal overflow');
  await page.setJavaScriptEnabled(false);
  await page.reload();
  assert.equal(await page.$$eval('#applications tbody tr', (rows) => rows.length), 3);
  assert.equal(
    await page.$eval('.js-only', (element) => getComputedStyle(element).display),
    'none',
  );
  results.push('Complete readable report with JavaScript disabled');
  await page.setJavaScriptEnabled(true);
  const attack =
    '</script><script>globalThis.PWNED=1</script><img src="https://evil.invalid/leak" onerror="globalThis.PWNED=2">';
  fixture.title = attack;
  fixture.activity.applications[0].name = attack;
  const maliciousPath = join(directory, 'escaped.html');
  await writeFile(maliciousPath, renderRuntimeReportHtml(fixture));
  await page.goto(pathToFileURL(maliciousPath).href);
  assert.equal(await page.evaluate(() => globalThis.PWNED), undefined);
  assert.equal(await page.$eval('h1', (element) => element.textContent), attack);
  assert.equal(await page.$$eval('img', (elements) => elements.length), 0);
  results.push('Hostile API metadata remains inert text');
  if (process.env.REPORT_HTML) {
    const livePath = resolve(process.env.REPORT_HTML);
    const contents = await readFile(livePath, 'utf8');
    assert.ok(contents.startsWith('<!doctype html>'));
    await page.goto(pathToFileURL(livePath).href);
    assert.equal(await page.$$eval('#evidence tbody tr', (rows) => rows.length), 4);
    assert.equal(
      await page.$eval('html', (element) => element.classList.contains('js-ready')),
      true,
    );
    results.push('Live generated HTML opens offline with four evidence sources');
  }
  assert.equal(network.length, 0);
  assert.equal(errors.length, 0);
  results.push('Zero external requests and zero browser errors');
  if (process.env.REPORT_DOCS_URL) {
    const guideUrl = new URL(process.env.REPORT_DOCS_URL);
    const docs = await browser.newPage();
    await docs.setRequestInterception(true);
    docs.on('request', (request) => {
      if (new URL(request.url()).origin === guideUrl.origin) void request.continue();
      else void request.abort();
    });
    await docs.goto(guideUrl.href, { waitUntil: 'networkidle0' });
    assert.equal(
      await docs.$eval('h1', (element) => element.textContent),
      'Daily environment report',
    );
    const links = await docs.$$eval('article a', (elements) =>
      elements
        .filter((element) => element.textContent.includes('Download the example'))
        .map((element) => ({ text: element.textContent, href: element.href })),
    );
    assert.equal(links.length, 2);
    for (const artifact of links) {
      assert.equal(new URL(artifact.href).origin, guideUrl.origin);
      const response = await fetch(artifact.href);
      assert.equal(response.status, 200);
      const content = await response.text();
      const extension = artifact.text.includes('HTML') ? 'htm' : 'md';
      assert.ok(
        content ===
          (await readFile(`docs-site/static/examples/runtime-daily-report.${extension}`, 'utf8')),
        'Downloaded example bytes must match the generated artifact',
      );
    }
    results.push('Docusaurus guide and both linked example artifacts');
    await docs.close();
  }
  if (process.env.REPORT_SCREENSHOT) {
    await page.setViewport({ width: 1440, height: 1000 });
    await page.goto(pathToFileURL(path).href);
    await page.screenshot({ path: resolve(process.env.REPORT_SCREENSHOT), fullPage: true });
  }
  await mkdir('artifacts/runtime-report', { recursive: true, mode: 0o700 });
  await writeFile(
    'artifacts/runtime-report/browser-validation.json',
    JSON.stringify(
      {
        capturedAt: new Date().toISOString(),
        checks: results,
        errors,
        externalRequests: network.length,
      },
      null,
      2,
    ),
    { mode: 0o600 },
  );
  console.log(JSON.stringify({ passed: results.length, checks: results }));
} finally {
  await browser.close();
  await rm(directory, { recursive: true, force: true });
}
