// Smoke test for the web app: serves web/, opens it in headless Chromium at phone and desktop
// sizes, waits for the Rust engine, then changes a setting, edits and breaks the recipe, switches
// templates and reloads, checking that nothing errored.
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
      const status = () => page.textContent('#status');
      await page.waitForFunction(() => /Engine ready/.test(document.getElementById('status').textContent), null, { timeout: 30000 });

      // A slider value is written into the recipe text.
      const hudBefore = await page.textContent('#hud');
      await page.fill('#set-twist', '180');
      await page.dispatchEvent('#set-twist', 'change');
      await page.waitForFunction((before) => document.getElementById('hud').textContent !== before, hudBefore, { timeout: 15000 });
      await page.click('#tab-recipe');
      await page.waitForFunction(() => /param twist\s+= 180 deg/.test(document.getElementById('recipe').value), null, { timeout: 15000 });
      const steps = await page.locator('#steps li').count();
      if (steps !== 6) failures.push(`${name}: expected 6 steps in the twisted vase, got ${steps}`);

      // Editing the recipe and pressing Run rebuilds the settings window.
      const edited = (await page.inputValue('#recipe')).replace(/param height(\s+)= 160 mm/, 'param height$1= 120 mm');
      await page.fill('#recipe', edited);
      await page.click('#runRecipe');
      await page.waitForFunction(() => document.getElementById('set-height')?.value === '120', null, { timeout: 15000 });

      // A broken recipe shows its problem by line, without crashing anything.
      await page.fill('#recipe', 'x = (1 +\n');
      await page.click('#runRecipe');
      await page.waitForFunction(() => document.querySelectorAll('#problems li').length === 1, null, { timeout: 15000 });
      if (!/problem/.test(await status())) failures.push(`${name}: status should report the recipe problem, got "${await status()}"`);
      if (shots) await page.screenshot({ path: path.join(shots, `smoke-${name}-recipe-error.png`), fullPage: name === 'phone' });

      // Templates replace the project; the print estimate follows.
      await page.click('#tab-settings');
      await page.selectOption('#template', 'ribbed-cylinder');
      // The project was edited, so the page asks before replacing it.
      if (!(await page.isVisible('#replaceConfirm'))) failures.push(`${name}: no confirmation before replacing an edited project`);
      await page.click('#replaceYes');
      await page.waitForFunction(() => document.getElementById('projectName').textContent === 'Ribbed cylinder', null, { timeout: 15000 });
      await page.waitForFunction(() => /Engine ready/.test(document.getElementById('status').textContent), null, { timeout: 15000 });
      if (shots) await page.screenshot({ path: path.join(shots, `smoke-${name}-ribbed.png`), fullPage: name === 'phone' });
      await page.selectOption('#template', 'twisted-vase');
      await page.waitForFunction(() => document.getElementById('projectName').textContent === 'Twisted star vase', null, { timeout: 15000 });
      await page.waitForFunction(() => /Engine ready/.test(document.getElementById('status').textContent), null, { timeout: 15000 });
      if (shots) await page.screenshot({ path: path.join(shots, `smoke-${name}-settings.png`), fullPage: name === 'phone' });

      await page.click('button[data-mode="path"]');
      await page.click('#tab-print');
      const stats = await page.locator('#stats .stat').count();
      if (stats !== 6) failures.push(`${name}: expected 6 print stats, got ${stats}`);

      // The project survives a reload (remembered on the device).
      await page.reload();
      await page.waitForFunction(() => document.getElementById('projectName').textContent === 'Twisted star vase', null, { timeout: 30000 });

      const overflow = await page.evaluate(() => document.documentElement.scrollWidth > window.innerWidth);
      if (overflow) failures.push(`${name}: page scrolls sideways`);

      if (shots) await page.screenshot({ path: path.join(shots, `smoke-${name}.png`), fullPage: name === 'phone' });
      console.log(`${name}: ${await status()} · ${await page.textContent('#hud')}`);
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
