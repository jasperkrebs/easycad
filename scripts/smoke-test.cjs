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
      // So do the thread connector, the spoon, the bend stool and the lampshade (EasyGCode's
      // defaults, its pellet printer).
      for (const [id, title, fixture] of [
        ['thread-connector', 'Thread connector', 'thread-default.gcode'],
        ['spoon', 'Spoon', 'spoon-flow.gcode'],
        ['bend-stool', 'Bend stool', 'stool-flow.gcode'],
        ['lampshade', 'Lampshade', 'lampshade-flow.gcode'],
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

      // The vessel is built from general steps, not checked line by line against EasyGCode:
      // the app must still make its G-code (one line from the bottom's centre to the rim).
      await page.click('#tab-settings');
      await page.selectOption('#template', 'vessel');
      await page.waitForFunction(() => document.getElementById('projectName').textContent === 'Vessel', null, { timeout: 15000 });
      await page.waitForFunction(() => /Engine ready/.test(document.getElementById('status').textContent), null, { timeout: 15000 });
      if (shots) await page.screenshot({ path: path.join(shots, `smoke-${name}-vessel.png`), fullPage: name === 'phone' });
      const vessel = (await saveFrom(page, '#exportButtons button.primary')).data.toString('utf8');
      const travels = commands(vessel).filter((c) => c.startsWith('G0 X')).length;
      if (!/^; EasyCAD — Vessel/.test(vessel) || travels !== 1) failures.push(`${name}: vessel G-code: ${travels} travels`);

      await page.click('#tab-print');
      if ((await page.inputValue('#printer')) !== 'pellet') failures.push(`${name}: the disc should pick the pellet printer`);
      await page.click('#panel-print summary');
      const fields = await page.locator('#printerSettings .setting').count();
      if (fields < 10) failures.push(`${name}: expected the printer settings, got ${fields} fields`);
      // The pellet printer is the Ginger G1: its bed, and checks from its wiki. The
      // vessel's 20 mm³/s is in the G1's low-flow range, so the upper zone should run cooler;
      // colorFabb FOAM PLA's profile does that.
      const checksText = () => page.textContent('#checks');
      if (!/On the 1[.,]?000 × 1[.,]?000 mm bed/.test(await checksText())) failures.push(`${name}: the G1's bed isn't checked: ${await checksText()}`);
      if (!/barrel inlet can plug/.test(await checksText())) failures.push(`${name}: no low-flow warning for the G1: ${await checksText()}`);
      if (!/vase-mode spiral with pressure advance 0[.,]3/.test(await checksText())) failures.push(`${name}: no pressure-advance note for the vessel's spiral`);
      await page.selectOption('#printer-material', 'colorFabb FOAM PLA');
      const zones = await Promise.all(['up', 'mid', 'down', 'bed'].map((k) => page.inputValue(`#printer-pellet-${k}`)));
      if (zones.join('/') !== '185/210/210/70') failures.push(`${name}: the FOAM PLA preset set ${zones.join('/')}`);
      await page
        .waitForFunction(() => /upper zone 25 °C below the others/.test(document.getElementById('checks').textContent), null, { timeout: 15000 })
        .catch(() => failures.push(`${name}: the low-flow check didn't see the cooler upper zone`));
      if (shots) await page.screenshot({ path: path.join(shots, `smoke-${name}-disc-print.png`), fullPage: name === 'phone' });
      await page.click('#printerSettings button:text("Back to the defaults")');
      if ((await page.inputValue('#printer-pellet-up')) !== '210') failures.push(`${name}: back to the defaults didn't reset the zones`);
      // Choosing a printer names it in the recipe, so the project opens with it next time.
      await page.selectOption('#printer', 'p1p');
      await page
        .waitForFunction(() => /^printer "Bambu P1P 0\.4"$/m.test(document.getElementById('recipe').value), null, { timeout: 15000 })
        .catch(() => failures.push(`${name}: choosing the P1P didn't rewrite the recipe's printer line`));
      if (!/G-code for the Bambu P1P/.test(await page.textContent('#exportButtons'))) failures.push(`${name}: the G-code button didn't follow the printer`);

      // The salt & pepper shaker: two parts (a container and its lid) from one set of settings,
      // each its own G-code file; its body follows a list setting, edited as a table.
      await page.click('#tab-settings');
      await page.selectOption('#template', 'salt-shaker');
      // Choosing the P1P edited the vessel's printer line, so the page asks first.
      if (await page.isVisible('#replaceConfirm')) await page.click('#replaceYes');
      await page.waitForFunction(() => document.getElementById('projectName').textContent === 'Salt & pepper shaker', null, { timeout: 15000 });
      await page.waitForFunction(() => /Engine ready/.test(document.getElementById('status').textContent), null, { timeout: 15000 });
      const parts = await page.locator('#exportButtons button.primary').allTextContents();
      if (parts.length !== 2 || !/Ginger G1: base$/.test(parts[0]) || !/Ginger G1: lid$/.test(parts[1])) failures.push(`${name}: shaker G-code buttons: ${parts.join(' | ')}`);
      const lidFile = await saveFrom(page, '#exportButtons button.primary >> nth=1');
      if (lidFile.name !== 'salt-pepper-shaker-lid.gcode' || !/^; EasyCAD — Salt & pepper shaker — lid\n/.test(lidFile.data.toString('utf8'))) {
        failures.push(`${name}: the lid's G-code is ${lidFile.name}: ${lidFile.data.toString('utf8').slice(0, 60)}`);
      }
      if (shots) await page.screenshot({ path: path.join(shots, `smoke-${name}-shaker.png`), fullPage: name === 'phone' });
      // Its parts show and hide like layers: a toggle per part in the 3D view.
      const toggles = page.locator('#viewParts button');
      if ((await toggles.allTextContents()).join() !== 'base,lid') failures.push(`${name}: part toggles: ${(await toggles.allTextContents()).join()}`);
      await toggles.nth(1).click();
      if ((await toggles.nth(1).getAttribute('aria-pressed')) !== 'false') failures.push(`${name}: the lid's toggle didn't hide it`);
      await toggles.nth(1).click();
      if ((await toggles.nth(1).getAttribute('aria-pressed')) !== 'true') failures.push(`${name}: the lid's toggle didn't show it again`);
      // A decimal comma, as an iPhone in a comma language types it, is taken; and a field that can
      // go negative has a ± button (the phone's decimal keypad has no minus key).
      await page.fill('#set-line', '2,5');
      await page.dispatchEvent('#set-line', 'change');
      await page
        .waitForFunction(() => /^param line\s+= 2\.5 mm/m.test(document.getElementById('recipe').value) && document.getElementById('set-line').value === '2.5', null, { timeout: 15000 })
        .catch(async () => failures.push(`${name}: "2,5" wasn't taken as 2.5: ${(await page.inputValue('#recipe')).match(/^param line.*$/m)}`));
      await page.fill('#set-line', '2');
      await page.dispatchEvent('#set-line', 'change');
      if ((await page.getAttribute('#set-line', 'inputmode')) !== 'decimal') failures.push(`${name}: number fields should ask for the decimal keypad`);
      await page.fill('#set-fit', '0,2');
      await page.dispatchEvent('#set-fit', 'change');
      await page.click('#set-fit >> xpath=preceding-sibling::button[contains(@class, "sign")]');
      await page
        .waitForFunction(() => document.getElementById('set-fit').value === '-0.2', null, { timeout: 15000 })
        .catch(async () => failures.push(`${name}: ± didn't flip the fit: ${await page.inputValue('#set-fit')}`));
      await page.fill('#set-fit', '0');
      await page.dispatchEvent('#set-fit', 'change');
      // A row added to the list setting, given a radius, then removed: the recipe follows.
      const body = (src) => (src.match(/^param body\s+= (\[.*\])/m) || [])[1];
      const rows = () => page.locator('.setting.list .list-row').count();
      await page.click('.setting.list .list-add button');
      if ((await rows()) !== 4) failures.push(`${name}: adding a row gave ${await rows()} rows`);
      await page.fill('#set-body-3-1', '30');
      await page.dispatchEvent('#set-body-3-1', 'change');
      await page
        .waitForFunction(() => /^param body\s+= \[\[0, 33\], \[0\.45, 38\], \[1, 32\], \[1, 30\]\]/m.test(document.getElementById('recipe').value), null, { timeout: 15000 })
        .catch(async () => failures.push(`${name}: the new row didn't reach the recipe: ${body(await page.inputValue('#recipe'))}`));
      await page.click('.setting.list .list-row >> nth=3 >> button');
      await page
        .waitForFunction(() => /^param body\s+= \[\[0, 33\], \[0\.45, 38\], \[1, 32\]\] /m.test(document.getElementById('recipe').value), null, { timeout: 15000 })
        .catch(async () => failures.push(`${name}: removing the row didn't reach the recipe: ${body(await page.inputValue('#recipe'))}`));
      // The spiral discs' own bead (0: the walls'): typed in, written into the recipe, run.
      const hudDisc = await page.textContent('#hud');
      for (const [id, v] of [['disc_layer', '0.3'], ['disc_line', '3'], ['fade_in', '12'], ['fade_out', '12']]) {
        await page.fill(`#set-${id}`, v);
        await page.dispatchEvent(`#set-${id}`, 'change');
      }
      await page
        .waitForFunction(() => /^param disc_layer\s+= 0\.3 mm/m.test(document.getElementById('recipe').value) && /^param disc_line\s+= 3 mm/m.test(document.getElementById('recipe').value), null, { timeout: 15000 })
        .catch(async () => failures.push(`${name}: the disc's bead didn't reach the recipe`));
      await page
        .waitForFunction((before) => document.getElementById('hud').textContent !== before, hudDisc, { timeout: 15000 })
        .catch(() => failures.push(`${name}: the disc's bead didn't change the print`));
      const discProblems = await page.locator('#problems li').count();
      if (discProblems) failures.push(`${name}: ${discProblems} problem(s) with the disc's own bead`);
      // With the settings scrolled down that far, the page itself hasn't moved: only the panels
      // scroll, and the 3D view is still in sight.
      if (await page.evaluate(() => document.scrollingElement.scrollHeight > window.innerHeight + 1)) failures.push(`${name}: the page scrolls; only the panels should`);
      const sideScrolled = await page.evaluate(() => /** @type {HTMLElement} */ (document.querySelector('.side')).scrollTop);
      const view = await page.locator('.viewport').boundingBox();
      const tall = page.viewportSize()?.height ?? 0;
      if (!(sideScrolled > 0) || !view || view.y < 0 || view.y + view.height > tall + 1) failures.push(`${name}: panels scrolled ${sideScrolled}, the 3D view at ${JSON.stringify(view)}`);
      if (shots) await page.screenshot({ path: path.join(shots, `smoke-${name}-shaker-disc.png`), fullPage: name === 'phone' });
      await page.click('#tab-print');
      const partTitles = await page.locator('#stats .part-title').allTextContents();
      if (partTitles.join() !== 'base,lid') failures.push(`${name}: the Print tab's parts: ${partTitles.join()}`);
      if (shots) await page.screenshot({ path: path.join(shots, `smoke-${name}-shaker-print.png`), fullPage: name === 'phone' });

      const overflow = await page.evaluate(() => document.documentElement.scrollWidth > window.innerWidth);
      if (overflow) failures.push(`${name}: page scrolls sideways`);

      if (shots) await page.screenshot({ path: path.join(shots, `smoke-${name}.png`), fullPage: name === 'phone' });
      console.log(`${name}: ${await status()} · ${await page.textContent('#hud')}`);
      await ctx.close();
    }

    // A project left on the device from an older version of its template (here the shaker
    // with its old step out to the fillet, and the line width changed): the app offers the
    // current template and carries the setting over.
    {
      const current = fs.readFileSync(path.join(root, 'templates/salt-shaker.easycad'), 'utf8');
      const older = current
        .replace('c_disc    = link_rings(reverse(c_rings), "spiral", disc_w)', 'c_disc    = polyline(flatten([vertices(link_rings(reverse(c_rings), "spiral", disc_w)), [point(0, flat_r)]]))')
        .replace(/^(param line\s+=) 2 mm/m, '$1 2.4 mm');
      if (older === current || !/^param line\s+= 2\.4 mm/m.test(older)) failures.push('update test: the older recipe is the same as the current one');
      const ctx = await browser.newContext({ viewport: { width: 1366, height: 860 } });
      await ctx.addInitScript((source) => {
        if (!sessionStorage.getItem('seeded')) {
          localStorage.setItem('easycad.project', JSON.stringify({ source, printer: 'p1p' }));
          sessionStorage.setItem('seeded', '1');
        }
      }, older);
      const page = await ctx.newPage();
      page.on('pageerror', (e) => failures.push(`update: page error: ${e.message}`));
      await page.goto(url);
      await page.waitForFunction(() => !document.getElementById('templateUpdate').hidden, null, { timeout: 30000 }).catch(() => failures.push('update: no offer to update the older shaker'));
      if (!/differs from the current “Salt & pepper shaker” template/.test(await page.textContent('#templateUpdateText'))) failures.push(`update: ${await page.textContent('#templateUpdateText')}`);
      if (shots) await page.screenshot({ path: path.join(shots, 'smoke-update-offer.png') });
      await page.click('#templateUpdateYes');
      await page.waitForFunction(() => /carried over/.test(document.getElementById('templateUpdated').textContent), null, { timeout: 30000 }).catch(() => failures.push('update: no word that it was updated'));
      const recipe = await page.inputValue('#recipe');
      if (!recipe.includes('c_disc    = link_rings(reverse(c_rings), "spiral", disc_w)')) failures.push('update: the recipe is still the older one');
      if (!/^param line\s+= 2\.4 mm/m.test(recipe)) failures.push(`update: the line width didn't carry over: ${(recipe.match(/^param line.*$/m) || [''])[0]}`);
      // Up to date now: after a reload there's nothing to offer.
      await page.reload();
      await page.waitForFunction(() => /Engine ready/.test(document.getElementById('status').textContent), null, { timeout: 30000 });
      await page.waitForTimeout(1500);
      if (!(await page.locator('#templateUpdate').isHidden())) failures.push('update: offered again after updating');
      console.log('update: an older shaker was brought up to date, its line width kept');

      // Saved, the project file starts with a note for whoever (or whichever AI) gets it; opened
      // again, the note is left out, so it's still the same recipe and still up to date.
      const saved = await saveFrom(page, '#saveProject');
      const file = saved.data.toString('utf8');
      if (!/^#\| EasyCAD project \(http/.test(file) || !file.includes('Copy for AI')) failures.push(`save: no note at the top of the project file: ${file.slice(0, 80)}`);
      const before = await page.inputValue('#recipe');
      if (!file.endsWith(before)) failures.push('save: the file is not the note plus the recipe');
      await page.setInputFiles('#projectFile', { name: saved.name, mimeType: 'text/plain', buffer: saved.data });
      await page.waitForTimeout(1500);
      if ((await page.inputValue('#recipe')) !== before) failures.push('save: opening the saved file changed the recipe (the note stayed in?)');
      if (!(await page.locator('#templateUpdate').isHidden())) failures.push('save: opening the saved file offers a template update');
      const settingsFile = JSON.parse((await saveFrom(page, '#saveSettings')).data.toString('utf8'));
      if (!/project file/.test(settingsFile.about || '') || typeof settingsFile.values !== 'object') failures.push(`save: the settings file has no note: ${JSON.stringify(settingsFile).slice(0, 120)}`);
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
