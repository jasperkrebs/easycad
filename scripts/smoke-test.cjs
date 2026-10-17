// Smoke test for the web app: serves web/, opens it in headless Chromium at phone and desktop
// sizes, waits for the Rust engine, then changes a setting, edits and breaks the recipe, switches
// templates, reloads, and saves files: an STL, and the spiral disc's G-code, which must match the
// G-code EasyGCode itself writes (fixtures/easygcode/). Checks that nothing errored.
//
//   node scripts/smoke-test.cjs [screenshot-dir]
//
// Needs Playwright (`npm i -g playwright`, or NODE_PATH pointing at a global install) and a
// built engine (scripts/build-web.sh). Exits non-zero on any failure.

const { chromium } = require('playwright');
const { spawn } = require('node:child_process');
const fs = require('node:fs');
const path = require('node:path');

const root = path.join(__dirname, '..', 'web');
const port = 8000 + Math.floor(Math.random() * 1000);
const shots = process.argv[2];

/** G-code commands without comments or blank lines. */
const commands = (text) =>
  text
    .split('\n')
    .map((l) => l.replace(/;.*/, '').trim())
    .filter(Boolean);

/** The same command, with numbers equal to the last printed digit (as the Rust golden test). */
function sameCommand(a, b) {
  if (a === b) return true;
  const ta = a.split(' ');
  const tb = b.split(' ');
  return (
    ta.length === tb.length &&
    ta.every((x, i) => {
      const y = tb[i];
      if (x === y) return true;
      if (x[0] !== y[0] || !'XYZE'.includes(x[0])) return false;
      return Math.abs(Number(x.slice(1)) - Number(y.slice(1))) <= (x[0] === 'E' ? 1.01e-5 : 1.01e-3);
    })
  );
}

/** Clicks a button that saves a file, and returns the file's name and bytes. */
async function saveFrom(page, selector) {
  const [download] = await Promise.all([page.waitForEvent('download', { timeout: 30000 }), page.click(selector)]);
  return { name: download.suggestedFilename(), data: fs.readFileSync(await download.path()) };
}

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

      // The shape saves as a closed STL mesh.
      const stl = await saveFrom(page, '#exportButtons button:text("twisted-vase.stl")');
      const triangles = stl.data.readUInt32LE(80);
      if (stl.name !== 'twisted-vase.stl' || stl.data.length !== 84 + 50 * triangles || triangles < 1000) {
        failures.push(`${name}: bad STL ${stl.name}: ${stl.data.length} bytes, ${triangles} triangles`);
      }

      await page.click('button[data-mode="path"]');
      await page.click('#tab-print');
      const stats = await page.locator('#stats .stat').count();
      if (stats !== 6) failures.push(`${name}: expected 6 print stats, got ${stats}`);
      if ((await page.inputValue('#printer')) !== 'p1p') failures.push(`${name}: the vase should pick the P1P`);

      // The project survives a reload (remembered on the device).
      await page.reload();
      await page.waitForFunction(() => document.getElementById('projectName').textContent === 'Twisted star vase', null, { timeout: 30000 });

      // EasyGCode's spiral disc, rebuilt as a recipe from general steps, prints EasyGCode's G-code.
      await page.click('#tab-settings');
      await page.selectOption('#template', 'spiral-disc');
      await page.waitForFunction(() => document.getElementById('projectName').textContent === 'Spiral disc', null, { timeout: 15000 });
      await page.waitForFunction(() => /Engine ready/.test(document.getElementById('status').textContent), null, { timeout: 15000 });
      if (shots) await page.screenshot({ path: path.join(shots, `smoke-${name}-disc-settings.png`), fullPage: name === 'phone' });
      const gcode = await saveFrom(page, '#exportButtons button.primary');
      const want = commands(fs.readFileSync(path.join(__dirname, '..', 'fixtures', 'easygcode', 'sdisc-flow.gcode'), 'utf8'));
      const got = commands(gcode.data.toString('utf8'));
      const differ = got.findIndex((c, i) => !sameCommand(c, want[i] ?? ''));
      if (gcode.name !== 'spiral-disc.gcode' || got.length !== want.length || differ >= 0) {
        failures.push(
          `${name}: spiral disc G-code ${gcode.name} differs from EasyGCode's: ${got.length} vs ${want.length} commands, first difference at ${differ}: "${got[differ]}" vs "${want[differ]}"`,
        );
      }
      // So do the thread connector and the spoon (EasyGCode's defaults, its pellet printer).
      for (const [id, title, fixture] of [
        ['thread-connector', 'Thread connector', 'thread-default.gcode'],
        ['spoon', 'Spoon', 'spoon-flow.gcode'],
      ]) {
        await page.click('#tab-settings');
        await page.selectOption('#template', id);
        await page.waitForFunction((t) => document.getElementById('projectName').textContent === t, title, { timeout: 15000 });
        await page.waitForFunction(() => /Engine ready/.test(document.getElementById('status').textContent), null, { timeout: 15000 });
        if (shots) await page.screenshot({ path: path.join(shots, `smoke-${name}-${id}.png`), fullPage: name === 'phone' });
        const file = await saveFrom(page, '#exportButtons button.primary');
        const expected = commands(fs.readFileSync(path.join(__dirname, '..', 'fixtures', 'easygcode', fixture), 'utf8'));
        const ours = commands(file.data.toString('utf8'));
        const at = ours.findIndex((c, i) => !sameCommand(c, expected[i] ?? ''));
        if (file.name !== `${id}.gcode` || ours.length !== expected.length || at >= 0) {
          failures.push(
            `${name}: ${id} G-code ${file.name} differs from EasyGCode's: ${ours.length} vs ${expected.length} commands, first difference at ${at}: "${ours[at]}" vs "${expected[at]}"`,
          );
        }
      }

      await page.click('#tab-print');
      if ((await page.inputValue('#printer')) !== 'pellet') failures.push(`${name}: the disc should pick the pellet printer`);
      await page.click('#panel-print summary');
      const fields = await page.locator('#printerSettings .setting').count();
      if (fields < 10) failures.push(`${name}: expected the printer settings, got ${fields} fields`);
      if (shots) await page.screenshot({ path: path.join(shots, `smoke-${name}-disc-print.png`), fullPage: name === 'phone' });

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
