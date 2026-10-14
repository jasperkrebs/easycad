import { readFileSync } from 'node:fs';
const mod = await WebAssembly.compile(readFileSync(process.argv[2]));
const imports = {};
for (const i of WebAssembly.Module.imports(mod)) {
  (imports[i.module] ??= {})[i.name] = () => { throw new Error('unexpected JS import call: ' + i.name); };
}
const inst = await WebAssembly.instantiate(mod, imports);
for (let i = 0; i < 5; i++) {
  const t = performance.now();
  const v = inst.exports.probe_run();
  console.log(`wasm run ${i}: result ${v.toFixed(1)} in ${(performance.now() - t).toFixed(1)} ms`);
}
