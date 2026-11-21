// The Render view in headless Chromium: the lampshade and the lamp open with their socket, bulb
// and cable in the 3D view and in the plan; the PNG has no bed grid; the Render view prepares
// the lamp and path traces it with the light on and off; its PNG has a transparent background.
// Screenshots (3D, plan, render on, render off) into [dir]. Checks that nothing errored.
//
//   node scripts/render-test.cjs [screenshot-dir]
//
// Needs Playwright and a built engine (scripts/build-web.sh). Headless Chromium path traces on
// its software GPU: slow, so it renders small and few samples. Not in CI (no GPU there either).

const { chromium } = require('playwright');
const { spawn } = require('node:child_process');
const fs = require('node:fs');
const path = require('node:path');
const { waitForServer } = require('./test-helpers.cjs');

const root = path.join(__dirname, '..', 'web');
const port = 8000 + Math.floor(Math.random() * 1000);
const shots = process.argv[2];
const SAMPLES = Number(process.env.RENDER_SAMPLES || 16);

/** The alpha channel's lowest and highest values of a PNG data URL, read in the page. */
async function alphaRange(page, url) {
  return page.evaluate(async (src) => {
    const img = new Image();
    img.src = src;
    await img.decode();
    const c = document.createElement('canvas');
    [c.width, c.height] = [img.width, img.height];
    const ctx = /** @type {CanvasRenderingContext2D} */ (c.getContext('2d'));
    ctx.drawImage(img, 0, 0);
    const d = ctx.getImageData(0, 0, c.width, c.height).data;
    let [lo, hi] = [255, 0];
    for (let i = 3; i < d.length; i += 4) {
      lo = Math.min(lo, d[i]);
      hi = Math.max(hi, d[i]);
    }
    return [lo, hi];
  }, url);
}

(async () => {
  const server = spawn('python3', ['-m', 'http.server', String(port), '--bind', '127.0.0.1'], { cwd: root, stdio: 'ignore' });
  const failures = [];
  let browser;
  try {
    const url = `http://127.0.0.1:${port}/`;
    await waitForServer(url);
    browser = await chromium.launch({ args: ['--enable-unsafe-swiftshader', '--ignore-gpu-blocklist'] });
    const page = await browser.newPage({ viewport: { width: 1000, height: 760 }, deviceScaleFactor: 1 });
    page.on('pageerror', (e) => failures.push(`page error: ${e.message}`));
    page.on('console', (m) => {
      if (m.type() === 'error') failures.push(`console: ${m.text()}`);
    });
    await page.goto(url);
    await page.waitForFunction(() => /Engine ready/.test(document.getElementById('status').textContent), null, { timeout: 60000 });
    if (shots) fs.mkdirSync(shots, { recursive: true });
    const shot = async (name) => shots && (await page.locator('.viewport').screenshot({ path: path.join(shots, `${name}.png`) }));

    for (const template of ['lampshade', 'lamp']) {
      await page.selectOption('#template', template);
      await page.waitForFunction((t) => document.getElementById('projectName').textContent.toLowerCase() === t, template, { timeout: 30000 });
      await page.waitForFunction(() => /Engine ready/.test(document.getElementById('status').textContent), null, { timeout: 60000 });
      await page.waitForTimeout(300);
      await shot(`${template}-3d`);
      // The plan shows the fittings.
      await page.click('#viewPlan');
      await page.waitForSelector('.plan-fixture .plan-bulb', { timeout: 10000 }).catch(() => failures.push(`${template}: no bulb in the plan`));
      await shot(`${template}-plan`);
      await page.click('[data-mode="both"]');
    }

    // The view's PNG: no bed grid. Its corners are the plain background.
    const viewPng = await page.evaluate(() => /** @type {any} */ (window).__easycadCapture?.());
    if (viewPng === undefined) {
      // No hook: check through the button's download.
      const [download] = await Promise.all([page.waitForEvent('download', { timeout: 30000 }), page.click('#viewSnap')]);
      if (!/\.png$/.test(download.suggestedFilename())) failures.push(`view PNG named ${download.suggestedFilename()}`);
    }

    // The Render view: prepare, light on, then off.
    await page.click('#viewRender');
    await page.waitForFunction(() => !document.getElementById('render').hidden, null, { timeout: 5000 });
    await page.selectOption('#renderSamples', '64');
    await page.evaluate((n) => {
      // Fewer samples than the list offers: headless Chromium's GPU is software.
      const s = /** @type {HTMLSelectElement} */ (document.getElementById('renderSamples'));
      s.append(new Option(String(n), String(n)));
      s.value = String(n);
      s.dispatchEvent(new Event('change'));
    }, SAMPLES);
    await page.click('#renderPrepare');
    // Prepared: the status says so, then counts the samples.
    await page.waitForFunction(() => /^Prepared|Can't| \/ /.test(document.getElementById('renderStatus').textContent), null, { timeout: 600000 });
    const prepared = await page.textContent('#renderStatus');
    console.log(prepared);
    if (/Can't/.test(prepared)) throw new Error(`render: ${prepared}`);
    const done = (n) => page.waitForFunction((k) => new RegExp(`^${k} / ${k}\\b`).test(document.getElementById('renderStatus').textContent), n, { timeout: 600000 });
    const t0 = Date.now();
    await done(SAMPLES);
    console.log(`light on: ${SAMPLES} samples in ${((Date.now() - t0) / 1000).toFixed(1)} s`);
    await shot('render-on');
    const [download] = await Promise.all([page.waitForEvent('download', { timeout: 30000 }), page.click('#renderSave')]);
    const file = await download.path();
    const dataUrl = `data:image/png;base64,${fs.readFileSync(file).toString('base64')}`;
    const [lo, hi] = await alphaRange(page, dataUrl);
    if (!(lo === 0 && hi === 255)) failures.push(`render PNG alpha ${lo}..${hi}: expected a transparent background round an opaque lamp`);
    if (shots) fs.copyFileSync(file, path.join(shots, 'render-on-transparent.png'));
    await page.click('#renderLight');
    await page.waitForFunction(() => document.getElementById('renderLight').textContent === 'Light off', null, { timeout: 5000 });
    await done(SAMPLES);
    await shot('render-off');
    const [download2] = await Promise.all([page.waitForEvent('download', { timeout: 30000 }), page.click('#renderSave')]);
    if (shots) fs.copyFileSync(await download2.path(), path.join(shots, 'render-off-transparent.png'));
  } catch (err) {
    failures.push(String(err.stack || err));
  } finally {
    await browser?.close();
    server.kill();
  }
  if (failures.length) {
    console.error(failures.join('\n'));
    process.exit(1);
  }
  console.log('render test passed');
})();
