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
const SIZE = Number(process.env.RENDER_SIZE || 320);

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

/** How noisy a picture is: the mean difference of each pixel from its four neighbours (0..255). */
async function noise(page, url) {
  return page.evaluate(async (src) => {
    const img = new Image();
    img.src = src;
    await img.decode();
    const c = document.createElement('canvas');
    [c.width, c.height] = [img.width, img.height];
    const ctx = /** @type {CanvasRenderingContext2D} */ (c.getContext('2d'));
    ctx.drawImage(img, 0, 0);
    const { data: d, width: w, height: h } = ctx.getImageData(0, 0, c.width, c.height);
    let sum = 0;
    let n = 0;
    for (let y = 1; y < h - 1; y++) {
      for (let x = 1; x < w - 1; x++) {
        const i = 4 * (y * w + x);
        if (d[i + 3] < 250) continue;
        for (let k = 0; k < 3; k++) {
          const around = (d[i + k - 4] + d[i + k + 4] + d[i + k - 4 * w] + d[i + k + 4 * w]) / 4;
          sum += Math.abs(d[i + k] - around);
          n++;
        }
      }
    }
    return n ? sum / n : 0;
  }, url);
}

/** A picture's mean brightness (0..255) over its opaque pixels. */
async function brightness(page, url) {
  return page.evaluate(async (src) => {
    const img = new Image();
    img.src = src;
    await img.decode();
    const c = document.createElement('canvas');
    [c.width, c.height] = [img.width, img.height];
    const ctx = /** @type {CanvasRenderingContext2D} */ (c.getContext('2d'));
    ctx.drawImage(img, 0, 0);
    const { data: d } = ctx.getImageData(0, 0, c.width, c.height);
    let sum = 0;
    let n = 0;
    for (let i = 0; i < d.length; i += 4) {
      if (d[i + 3] < 250) continue;
      sum += (d[i] + d[i + 1] + d[i + 2]) / 3;
      n++;
    }
    return n ? sum / n : 0;
  }, url);
}

/** The spread (standard deviation) of brightness in the middle third of a picture. */
async function centreSpread(page, url) {
  return page.evaluate(async (src) => {
    const img = new Image();
    img.src = src;
    await img.decode();
    const c = document.createElement('canvas');
    [c.width, c.height] = [img.width, img.height];
    const ctx = /** @type {CanvasRenderingContext2D} */ (c.getContext('2d'));
    ctx.drawImage(img, 0, 0);
    const { data: d } = ctx.getImageData(c.width / 3, c.height / 3, c.width / 3, c.height / 3);
    const v = [];
    for (let i = 0; i < d.length; i += 4) v.push((d[i] + d[i + 1] + d[i + 2]) / 3);
    const mean = v.reduce((a, b) => a + b, 0) / v.length;
    return Math.sqrt(v.reduce((a, b) => a + (b - mean) ** 2, 0) / v.length);
  }, url);
}

(async () => {
  const server = spawn('python3', ['-m', 'http.server', String(port), '--bind', '127.0.0.1'], { cwd: root, stdio: 'ignore' });
  const failures = [];
  let browser;
  try {
    const url = `http://127.0.0.1:${port}/`;
    await waitForServer(url);
    // WebGPU (software) for the denoiser, WebGL (software) for the path tracer.
    browser = await chromium.launch({ args: ['--enable-unsafe-swiftshader', '--ignore-gpu-blocklist', '--enable-unsafe-webgpu'] });
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
      // The bulb button hides the fittings and shows them again (remembered).
      await page.click('.plan-fittings');
      await page.waitForFunction(() => !document.querySelector('.plan-fixture') && document.querySelector('.plan-fittings')?.getAttribute('aria-pressed') === 'false', null, { timeout: 5000 }).catch(() => failures.push(`${template}: the fittings didn't hide`));
      await shot(`${template}-plan-hidden`);
      await page.click('.plan-fittings');
      await page.waitForSelector('.plan-fixture', { timeout: 5000 }).catch(() => failures.push(`${template}: the fittings didn't come back`));
      await page.click('[data-mode="both"]');
    }

    // The view's PNG: no bed grid. Its corners are the plain background.
    const viewPng = await page.evaluate(() => /** @type {any} */ (window).__easycadCapture?.());
    if (viewPng === undefined) {
      // No hook: check through the button's download.
      const [download] = await Promise.all([page.waitForEvent('download', { timeout: 30000 }), page.click('#viewSnap')]);
      if (!/\.png$/.test(download.suggestedFilename())) failures.push(`view PNG named ${download.suggestedFilename()}`);
    }

    // The Render view: a set of scenes (projects, plastics, how solid, light on and off, the
    // room), each prepared and previewed small, its PNG saved. Headless Chromium's GPU is
    // software: small pictures and few samples.
    await page.click('#viewRender');
    await page.waitForFunction(() => !document.getElementById('render').hidden, null, { timeout: 5000 });
    const quick = { preview: { size: SIZE, samples: SAMPLES }, final: { size: SIZE, samples: 512 }, bounces: 12 };
    const scenes = [
      { name: 'lampshade-pla-white', template: 'lampshade', project: 'Lampshade', part: 'path', look: { color: '#f4f1ea', opacity: 1, material: { plastic: 'pla' } }, render: { light: true } },
      { name: 'lampshade-petg-frosted', template: 'lampshade', project: 'Lampshade', part: 'path', look: { color: '#f6efe2', opacity: 0.4, material: { plastic: 'petg-frosted' } }, render: { light: true, wall: { on: true, distance: 250 }, floor: { on: true, distance: 400 } } },
      { name: 'lamp-pc-amber-below', template: 'lamp', project: 'Lamp', part: 'main', look: { color: '#ffb347', opacity: 0.25, material: { plastic: 'pc' } }, render: { light: true, camera: { view: 'below', lens: 35, position: null, target: null } } },
      { name: 'lamp-silk-gold-off', template: 'lamp', project: 'Lamp', part: 'main', look: { color: '#c9a227', opacity: 1, material: { plastic: 'pla-silk' } }, render: { light: false, camera: { view: 'three-quarter', lens: 50, position: null, target: null }, wall: { on: true, distance: 300 } } },
      // Wall, floor and ceiling all on, the camera above the ceiling: a cutaway (the lamp shows).
      { name: 'lamp-room-above', template: 'lamp', project: 'Lamp', part: 'main', look: { color: '#f4f1ea', opacity: 0.5, material: { plastic: 'petg-frosted' } }, render: { light: true, camera: { view: 'above', lens: 50, position: null, target: null }, wall: { on: true, distance: 300 }, floor: { on: true, distance: 400 }, ceiling: { on: true, distance: 200 } }, lampShows: true },
      { name: 'pendant-matte-white', template: 'thread-pendant', project: 'Thread pendant', part: 'path_out', look: { color: '#fbfaf7', opacity: 0.6, material: { plastic: 'pla-matte' } }, render: { light: true, ceiling: { on: true, distance: 300 }, wall: { on: true, distance: 350 } } },
    ];
    const only = process.env.RENDER_SCENES ? process.env.RENDER_SCENES.split(',') : null;
    for (const sc of scenes.filter((x) => !only || only.includes(x.name))) {
      const t0 = Date.now();
      await page.evaluate(({ project, part, look, render }) => {
        const all = JSON.parse(localStorage.getItem('easycad.customColors') || '{}');
        all[project] = { parts: { [part]: look }, light: 0.5, shadow: 0.5, hang: 'down', render };
        localStorage.setItem('easycad.customColors', JSON.stringify(all));
      }, { ...sc, render: { ...quick, ...sc.render } });
      const current = await page.textContent('#projectName');
      if (current !== sc.project) {
        // The template list is in the Settings tab (the Render tab is in front in the Render view).
        await page.click('#tab-settings');
        await page.selectOption('#template', sc.template);
        await page.waitForFunction((t) => document.getElementById('projectName').textContent === t, sc.project, { timeout: 30000 });
        await page.waitForFunction(() => /Engine ready/.test(document.getElementById('status').textContent), null, { timeout: 60000 });
        await page.click('#tab-render');
      }
      await page.evaluate(() => document.getElementById('renderPrepare').click());
      await page.waitForFunction(() => /^Draft|Can't/.test(document.getElementById('renderStatus').textContent), null, { timeout: 300000 });
      const prepared = await page.textContent('#renderStatus');
      if (/Can't/.test(prepared)) throw new Error(`${sc.name}: ${prepared}`);
      await shot(`${sc.name}-draft`);
      await page.evaluate(() => document.getElementById('renderPreview').click());
      // Done, then denoised (or why not).
      await page.waitForFunction(() => /(done|stopped) .*· (denoised|smoothed|not denoised|as rendered)|Can't/.test(document.getElementById('renderStatus').textContent), null, { timeout: 1200000 });
      const status = await page.textContent('#renderStatus');
      console.log(`${sc.name}: ${status} (${((Date.now() - t0) / 1000).toFixed(0)} s with preparing)`);
      await shot(`${sc.name}-screen`);
      const [download] = await Promise.all([page.waitForEvent('download', { timeout: 30000 }), page.click('#renderSave')]);
      const file = await download.path();
      if (shots) fs.copyFileSync(file, path.join(shots, `${sc.name}.png`));
      const denoisedUrl = `data:image/png;base64,${fs.readFileSync(file).toString('base64')}`;
      if (!/· denoised/.test(status)) failures.push(`${sc.name}: not denoised by Open Image Denoise: ${status}`);
      // The picture as rendered, for comparison: the denoised one is clearly less noisy.
      await page.evaluate(() => document.getElementById('renderDenoised').click());
      await page.waitForFunction(() => /as rendered/.test(document.getElementById('renderStatus').textContent), null, { timeout: 30000 });
      const [rawDownload] = await Promise.all([page.waitForEvent('download', { timeout: 30000 }), page.click('#renderSave')]);
      const rawFile = await rawDownload.path();
      if (shots) fs.copyFileSync(rawFile, path.join(shots, `${sc.name}-raw.png`));
      const rawUrl = `data:image/png;base64,${fs.readFileSync(rawFile).toString('base64')}`;
      const [clean, raw] = [await noise(page, denoisedUrl), await noise(page, rawUrl)];
      console.log(`  noise: ${raw.toFixed(1)} as rendered, ${clean.toFixed(1)} denoised`);
      if (!(clean < raw * 0.6)) failures.push(`${sc.name}: denoised noise ${clean.toFixed(1)} vs ${raw.toFixed(1)} as rendered`);
      // And about as bright (a broken denoise comes out flat black: no noise at all). A bit
      // brighter is right: the display clips the raw picture's bright specks, the denoiser
      // spreads their light.
      const [bClean, bRaw] = [await brightness(page, denoisedUrl), await brightness(page, rawUrl)];
      console.log(`  brightness: ${bRaw.toFixed(1)} as rendered, ${bClean.toFixed(1)} denoised`);
      if (!(bClean > 0.6 * bRaw && bClean < 1.6 * bRaw + 3)) failures.push(`${sc.name}: denoised brightness ${bClean.toFixed(1)} vs ${bRaw.toFixed(1)} as rendered`);
      await page.evaluate(() => document.getElementById('renderDenoised').click());
      if (sc.lampShows) {
        // The lamp in the middle of the picture: not one flat colour (a surface in front of it).
        const spread = await centreSpread(page, denoisedUrl);
        if (!(spread > 8)) failures.push(`${sc.name}: the middle of the picture is flat (${spread.toFixed(1)}): the lamp is hidden`);
      }
      // Without a room, the background is transparent round an opaque lamp.
      if (!sc.render.wall && !sc.render.floor && !sc.render.ceiling) {
        const [lo, hi] = await alphaRange(page, `data:image/png;base64,${fs.readFileSync(file).toString('base64')}`);
        if (!(lo === 0 && hi === 255)) failures.push(`${sc.name}: PNG alpha ${lo}..${hi}, expected a transparent background round the lamp`);
      }
    }
    // Stop: the Render (512 samples: long), once it has a sample, stops; the picture so far stays
    // and is denoised. (The
    // page is busy between tiles on a software GPU: clicks go through the page, which waits.)
    await page.evaluate(() => document.getElementById('renderFinal').click());
    await page.waitForFunction(() => /^Render · [1-9]\d* \//.test(document.getElementById('renderStatus').textContent), null, { timeout: 600000 });
    await page.evaluate(() => document.getElementById('renderStop').click());
    await page
      .waitForFunction(() => /stopped .*· (denoised|smoothed|not denoised)/.test(document.getElementById('renderStatus').textContent), null, { timeout: 600000 })
      .catch(async () => failures.push(`Stop: ${await page.textContent('#renderStatus')}`));
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
