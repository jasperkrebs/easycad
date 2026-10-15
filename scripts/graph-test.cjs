// End-to-end test of the Graph tab: serves web/, opens the app in headless Chromium (desktop
// with a mouse, phone with touch) and edits a project only through the graph:
// - select a component and type an input;
// - add a component from the palette, wire it by dragging, show it in the 3D view;
// - rename, undo and remove it;
// - add a slider and change its range, and move a slider;
// - pan and zoom.
// After each step it checks the recipe text, which must change exactly as the command says.
// Checks that nothing errored and that the page never scrolls sideways.
//
//   node scripts/graph-test.cjs [screenshot-dir]
//
// Needs Playwright and a built engine (scripts/build-web.sh). Exits non-zero on any failure.

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

/** The recipe text, as the Recipe tab shows it. */
const recipe = (page) => page.$eval('#recipe', (e) => /** @type {HTMLTextAreaElement} */ (e).value);

/** Waits until the recipe text passes `test`. */
async function recipeWhere(page, test, what) {
  const deadline = Date.now() + 15000;
  for (;;) {
    const text = await recipe(page);
    if (test(text)) return text;
    if (Date.now() > deadline) throw new Error(`the recipe never ${what}:\n${text}`);
    await page.waitForTimeout(100);
  }
}

/** The centre of an element on screen. */
async function centre(page, selector) {
  const box = await (await page.waitForSelector(selector, { timeout: 15000 })).boundingBox();
  if (!box) throw new Error(`${selector} is not on screen`);
  return { x: box.x + box.width / 2, y: box.y + box.height / 2, box };
}

async function inspectorInput(page, name) {
  return page.waitForSelector(`#graphInspector .ginput:has(> label:text-matches("^${name}\\\\b")) input[type="text"]`, { timeout: 15000 });
}

(async () => {
  const server = spawn('python3', ['-m', 'http.server', String(port), '--bind', '127.0.0.1'], { cwd: root, stdio: 'ignore' });
  const failures = [];
  let browser;
  try {
    const url = `http://127.0.0.1:${port}/index.html`;
    await waitForServer(url);
    browser = await chromium.launch();

    // ----------------------------------------------------------------- desktop, with a mouse
    {
      const ctx = await browser.newContext({ viewport: { width: 1366, height: 860 } });
      const page = await ctx.newPage();
      page.on('pageerror', (e) => failures.push(`desktop: page error: ${e.message}`));
      page.on('console', (m) => {
        if (m.type() === 'error') failures.push(`desktop: console error: ${m.text()}`);
      });
      await page.goto(url);
      await page.waitForFunction(() => /Engine ready/.test(document.getElementById('status').textContent), null, { timeout: 30000 });
      await page.click('#tab-graph');
      await page.waitForSelector('.gnode[data-id="outline"]');
      const counts = await page.evaluate(() => [document.querySelectorAll('.gnode').length, document.querySelectorAll('.gwire').length]);
      if (counts[0] < 15 || counts[1] < 15) failures.push(`desktop: expected the vase as ≥ 15 components and wires, got ${counts}`);

      // Select a component, type an input.
      const head = await centre(page, '.gnode[data-id="outline"] .gnode-title');
      await page.mouse.click(head.x, head.y);
      await page.waitForFunction(() => /** @type {HTMLInputElement} */ (document.querySelector('#graphInspector .ginsp-name'))?.value === 'outline');
      const inner = await inspectorInput(page, 'inner');
      await inner.fill('radius * 0.5');
      await inner.press('Enter');
      await recipeWhere(page, (t) => t.includes('star(points, outer: radius, inner: radius * 0.5) |> smooth'), 'got the typed input');

      // Add a component from the palette.
      await page.click('#graphAdd');
      await page.fill('#graphSearch', 'offset');
      await page.press('#graphSearch', 'Enter');
      await recipeWhere(page, (t) => t.includes('offset1 = offset(distance: 10)'), 'got the new component');
      await page.waitForSelector('.gnode[data-id="offset1"] .grow[data-input="curve"]');

      // Wire it by dragging from outline's output to its `curve` input.
      const from = await centre(page, '.gnode[data-id="outline"] .gport.out');
      const to = await centre(page, '.gnode[data-id="offset1"] .grow[data-input="curve"]');
      await page.mouse.move(from.x, from.y);
      await page.mouse.down();
      await page.mouse.move((from.x + to.x) / 2, (from.y + to.y) / 2, { steps: 5 });
      await page.mouse.move(to.x, to.y, { steps: 5 });
      await page.mouse.up();
      await recipeWhere(page, (t) => t.includes('offset1 = offset(distance: 10, curve: outline)'), 'got the dragged wire');
      await page.waitForFunction(() => document.querySelector('.gnode[data-id="offset1"]')?.classList.contains('ok'));

      // Show it in the 3D view.
      await page.click('.gnode[data-id="offset1"] .gnode-eye');
      await recipeWhere(page, (t) => /^show vase, offset1$/m.test(t), 'showed the new component');

      // Rename it, undo that, then remove it.
      const offsetHead = await centre(page, '.gnode[data-id="offset1"] .gnode-title');
      await page.mouse.click(offsetHead.x, offsetHead.y);
      await page.waitForFunction(() => /** @type {HTMLInputElement} */ (document.querySelector('#graphInspector .ginsp-name'))?.value === 'offset1');
      await page.fill('#graphInspector .ginsp-name', 'ring');
      await page.press('#graphInspector .ginsp-name', 'Enter');
      await recipeWhere(page, (t) => t.includes('ring = offset(distance: 10, curve: outline)') && /^show vase, ring$/m.test(t), 'renamed the component');
      await page.click('#graphUndo');
      await recipeWhere(page, (t) => t.includes('offset1 = offset(') && !t.includes('ring ='), 'undid the rename');
      const again = await centre(page, '.gnode[data-id="offset1"] .gnode-title');
      await page.mouse.click(again.x, again.y);
      await page.click('#graphInspector button:text("Remove")');
      await recipeWhere(page, (t) => !t.includes('offset1') && /^show vase$/m.test(t), 'removed the component');

      // Add a slider and change its range.
      await page.click('#graphSlider');
      await recipeWhere(page, (t) => t.includes('param slider1 = 1 in 0..10 "Slider 1"'), 'got the slider');
      await page.waitForSelector('#graphInspector [data-key="max"]');
      await page.fill('#graphInspector [data-key="max"]', '20');
      await page.click('#graphInspector button:text("Apply")');
      await recipeWhere(page, (t) => t.includes('param slider1 = 1 in 0..20 "Slider 1"'), 'changed the slider range');

      // Move a slider in the graph: the recipe and the Settings tab follow.
      await page.$eval('.gnode[data-id="height"] input[type="range"]', (e) => {
        const r = /** @type {HTMLInputElement} */ (e);
        r.value = '200';
        r.dispatchEvent(new Event('input', { bubbles: true }));
        r.dispatchEvent(new Event('change', { bubbles: true }));
      });
      await recipeWhere(page, (t) => /^param height\s+= 200 mm/m.test(t), 'took the slider value');
      if ((await page.$eval('#set-height', (e) => /** @type {HTMLInputElement} */ (e).value)) !== '200') failures.push('desktop: the Settings tab did not follow the graph slider');

      // Pan and zoom.
      const before = await page.$eval('#graphWorld', (e) => e.style.transform);
      const canvasBox = (await page.$('#graphCanvas'))?.boundingBox && (await (await page.$('#graphCanvas')).boundingBox());
      await page.mouse.move(canvasBox.x + canvasBox.width - 30, canvasBox.y + canvasBox.height - 30);
      await page.mouse.down();
      await page.mouse.move(canvasBox.x + canvasBox.width - 130, canvasBox.y + canvasBox.height - 80, { steps: 5 });
      await page.mouse.up();
      await page.mouse.wheel(0, -200);
      const after = await page.$eval('#graphWorld', (e) => e.style.transform);
      if (before === after) failures.push('desktop: dragging and scrolling the canvas did not pan or zoom');
      await page.click('#graphFit');

      // Double-click on empty space opens the palette; Escape closes it.
      /** A point on the canvas with no component under it (the page may have scrolled). */
      const emptySpot = async () => {
        const spot = await page.evaluate(() => {
          const r = document.getElementById('graphCanvas').getBoundingClientRect();
          for (let y = r.bottom - 15; y > r.top + 15; y -= 20) {
            for (let x = r.left + 15; x < r.right - 15; x += 20) {
              const hit = document.elementFromPoint(x, y);
              if (hit && !hit.closest('.gnode') && hit.closest('#graphCanvas')) return { x, y };
            }
          }
          return null;
        });
        if (!spot) throw new Error('no empty space on the graph canvas');
        return spot;
      };
      const empty = await emptySpot();
      await page.mouse.dblclick(empty.x, empty.y);
      await page.waitForSelector('#graphPalette:not([hidden])');
      await page.press('#graphSearch', 'Escape');
      await page.waitForSelector('#graphPalette[hidden]', { state: 'attached' });
      // A tap on empty space clears the selection.
      const flowHead = await centre(page, '.gnode[data-id="flow"] .gnode-title');
      await page.mouse.click(flowHead.x, flowHead.y);
      await page.waitForSelector('#graphInspector .ginsp-name');
      const spot = await emptySpot();
      await page.mouse.click(spot.x, spot.y);
      await page.waitForFunction(() => !document.querySelector('#graphInspector .ginsp-name') && !document.querySelector('.gnode.selected'));

      const status = await page.textContent('#status');
      if (!/Engine ready/.test(status || '')) failures.push(`desktop: status is "${status}"`);
      const sideways = await page.evaluate(() => document.documentElement.scrollWidth > document.documentElement.clientWidth);
      if (sideways) failures.push('desktop: the page scrolls sideways');
      if (shots) await page.screenshot({ path: path.join(shots, 'graph-desktop.png') });
      console.log(`desktop: ${counts[0]} components, ${counts[1]} wires; every graph command edited the recipe as expected`);
      await ctx.close();
    }

    // ----------------------------------------------------------------- phone, with touch
    {
      const ctx = await browser.newContext({ viewport: { width: 390, height: 844 }, isMobile: true, hasTouch: true, deviceScaleFactor: 2 });
      const page = await ctx.newPage();
      page.on('pageerror', (e) => failures.push(`phone: page error: ${e.message}`));
      page.on('console', (m) => {
        if (m.type() === 'error') failures.push(`phone: console error: ${m.text()}`);
      });
      await page.goto(url);
      await page.waitForFunction(() => /Engine ready/.test(document.getElementById('status').textContent), null, { timeout: 30000 });
      const tabsFit = await page.$eval('.tabs', (e) => e.scrollWidth <= e.clientWidth + 1);
      if (!tabsFit) failures.push('phone: the tabs do not fit the screen');
      await page.tap('#tab-graph');
      const head = await centre(page, '.gnode[data-id="height"] .gnode-title');
      await page.touchscreen.tap(head.x, head.y);
      await page.waitForFunction(() => /** @type {HTMLInputElement} */ (document.querySelector('#graphInspector .ginsp-name'))?.value === 'height', null, { timeout: 15000 });
      const sideways = await page.evaluate(() => document.documentElement.scrollWidth > document.documentElement.clientWidth);
      if (sideways) failures.push('phone: the page scrolls sideways');
      if (shots) await page.screenshot({ path: path.join(shots, 'graph-phone.png'), fullPage: true });
      console.log('phone: tapping a component selects it');
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
  console.log('graph test passed');
})();
