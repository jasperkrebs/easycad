// The owner's approved prints (fixtures/prints/): each project file, as saved from EasyCAD with
// its printer settings, and the G-code it printed, which the owner approved. The engine must
// still make exactly that G-code: a change to the engine that changes it fails here, so the
// G-code of a print that worked never changes by accident. When a change is meant (a fix, a
// better default the owner asked for), write the new G-code with
//
//   UPDATE_PRINTS=1 node --test scripts/prints.test.mjs
//
// and say so in the pull request: the owner approves the new G-code with it.
//
// Runs on the built engine (web/pkg, scripts/build-web.sh), the way the app makes G-code: the
// printer's settings from the file through the Print tab's own checks (print.js).

import assert from 'node:assert/strict';
import { existsSync, readFileSync, readdirSync, writeFileSync } from 'node:fs';
import { test } from 'node:test';
import { PRINTERS, printerFor, profileFrom } from '../web/js/print.js';
import { withoutColors, withoutFileNote, withoutPrinter } from '../web/js/templates.js';

const dir = new URL('../fixtures/prints/', import.meta.url);
const pkg = new URL('../web/pkg/', import.meta.url);
const update = process.env.UPDATE_PRINTS === '1';

/** The engine, as the app loads it. */
async function engine() {
  assert.ok(existsSync(new URL('easycad_wasm_bg.wasm', pkg)), 'build the engine first: scripts/build-web.sh');
  const wasm = await import(new URL('easycad_wasm.js', pkg).href);
  await wasm.default({ module_or_path: readFileSync(new URL('easycad_wasm_bg.wasm', pkg)) });
  return wasm;
}

/** The G-code file of part `i` (its name in the fixtures: `name.gcode`, or `name-i.gcode`). */
const gcodeFile = (name, i) => new URL(`${name}${i ? `-${i}` : ''}.gcode`, dir);

const projects = readdirSync(dir).filter((f) => f.endsWith('.easycad'));

test('there are approved prints to check', () => {
  assert.ok(projects.length > 0);
});

for (const file of projects) {
  const name = file.replace(/\.easycad$/, '');
  test(`${name}: the engine still makes the approved G-code`, async () => {
    const { exportFile } = await engine();
    const { text: rest } = withoutColors(withoutFileNote(readFileSync(new URL(file, dir), 'utf8')));
    const { text: source, printer } = withoutPrinter(rest);
    assert.ok(printer, `${file} has no printer settings (a #~ printer line): save it from EasyCAD again`);
    const id = /** @type {keyof typeof PRINTERS} */ (printer.printer);
    assert.ok(id in PRINTERS, `unknown printer ${id}`);
    const named = source.match(/^printer\s+"([^"]*)"/m)?.[1];
    assert.equal(printerFor(named ?? ''), id, `the recipe names ${named}, the settings are for ${id}`);
    const profile = profileFrom(id, printer.settings);
    for (let i = 0; ; i++) {
      let made;
      try {
        made = exportFile(source, {}, `print:${i}`, profile);
      } catch (err) {
        assert.ok(i > 0, `${name}: ${err.message || err}`);
        assert.match(String(err.message || err), /no such part/);
        break;
      }
      const gcode = new TextDecoder().decode(made.data);
      const ref = gcodeFile(name, i);
      if (update || !existsSync(ref)) {
        assert.ok(update, `${name}: no approved G-code yet (${ref.pathname}); write it with UPDATE_PRINTS=1`);
        writeFileSync(ref, gcode);
        continue;
      }
      const want = readFileSync(ref, 'utf8');
      if (gcode === want) continue;
      const [a, b] = [gcode.split('\n'), want.split('\n')];
      const at = a.findIndex((l, k) => l !== b[k]);
      assert.fail(`${name}${i ? ` part ${i}` : ''}: the G-code changed from the approved print at line ${at + 1}:\n  now:      ${a[at]}\n  approved: ${b[at]}\n(${a.length} lines now, ${b.length} approved). Meant? UPDATE_PRINTS=1 and say so in the pull request.`);
    }
  });
}
