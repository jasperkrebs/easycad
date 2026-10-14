// Smoke test for the web app: serves web/, opens it in headless Chromium at phone and desktop
// sizes, waits for the Rust engine, changes a setting, and checks that nothing errored.
//
//   node scripts/smoke-test.cjs [screenshot-dir]
//
// Needs Playwright (`npm i -g playwright`, or NODE_PATH pointing at a global install) and a
// built engine (scripts/build-web.sh). Exits non-zero on any failure.

const { chromium } = require('playwright');
const { spawn } = require('node:child_process');
const path = require('node:path');

const root = path.join(__dirname, '..', 'web');
const port = 8000 + Math.floor(Math.random() * 1000);
const shots = process.argv[2];

async function waitForServer(url) {
  for (let i = 0; i < 50; i++) {
    try {
      const r = await fetch(url);
      if (r.ok) return;
    } catch {
      // not up yet
    }
    await new Promise((r) => setTimeout(r, 100));
  }
  throw new Error('web server did not start');
}

(async () => {
  const server = spawn('python3', ['-m', 'http.server', String(port), '--bind', '127.0.0.1'], { cwd: root, stdio: 'ignore' });
  const failures = [];
  let browser;
  try {
    const url = `http://127.0.0.1:${port}/index.html`;
    await waitForServer(url);
    browser = await chromium.launch();
    for (const [name, opts] of [
      ['phone', { viewport: { width: 390, height: 844 }, isMobile: true, hasTouch: true, deviceScaleFactor: 2 }],
      ['desktop', { viewport: { width: 1366, height: 860 } }],
    ]) {
      const ctx = await browser.newContext(opts);
      const page = await ctx.newPage();
      page.on('pageerror', (e) => failures.push(`${name}: page error: ${e.message}`));
      page.on('console', (m) => {
        if (m.type() === 'error') failures.push(`${name}: console error: ${m.text()}`);
      });
      await page.goto(url);
      await page.waitForFunction(() => /Engine ready/.test(document.getElementById('status').textContent), null, { timeout: 30000 });

      const hudBefore = await page.textContent('#hud');
      await page.fill('#n-twist', '180');
      await page.dispatchEvent('#n-twist', 'change');
      await page.waitForFunction((before) => document.getElementById('hud').textContent !== before, hudBefore, { timeout: 15000 });

      await page.click('button[data-mode="path"]');
      await page.click('#tab-print');
      const stats = await page.locator('#stats .stat').count();
      if (stats !== 6) failures.push(`${name}: expected 6 print stats, got ${stats}`);

      const overflow = await page.evaluate(() => document.documentElement.scrollWidth > window.innerWidth);
      if (overflow) failures.push(`${name}: page scrolls sideways`);

      if (shots) await page.screenshot({ path: path.join(shots, `smoke-${name}.png`), fullPage: name === 'phone' });
      console.log(`${name}: ${await page.textContent('#status')} · ${await page.textContent('#hud')}`);
      await ctx.close();
    }
  } catch (err) {
    failures.push(String(err && err.stack ? err.stack : err));
  } finally {
    if (browser) await browser.close();
    server.kill();
  }
  if (failures.length) {
    console.error(failures.join('\n'));
    process.exit(1);
  }
  console.log('smoke test passed');
})();
