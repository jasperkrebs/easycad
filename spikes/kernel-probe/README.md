# Spike: kernel probe (curvo + manifold-rust)

Throwaway experiment from the research phase. It is **not** EasyCAD code; it answers one question:

> Can pure-Rust geometry libraries do a Grasshopper-style model natively **and** in WebAssembly,
> fast and small enough for a phone?

It builds a **twisted 7-point star vase** two ways:

1. **NURBS** (`curvo`): smooth periodic star curve, 24 sections that scale, rotate and lift, then
   loft and adaptive tessellation.
2. **Solid** (`manifold-rust`): twisted, scaled extrusion of the star, hollowed by a boolean
   difference with a smaller copy (a watertight, printable shell).

## Results (14 Oct 2026, cloud server CPU, Rust 1.97, Node 22)

| | |
|---|---|
| Compiles to `wasm32-unknown-unknown` unchanged | yes |
| WASM size (`opt-level = "s"`, LTO, no `wasm-opt`) | 1.88 MB raw, **0.44 MB gzipped** |
| Native, whole solve (loft ≈ 11k triangles + boolean shell 3,720 triangles) | ~31–33 ms |
| WASM in Node, steady state | **~50 ms** (first call ~100 ms) |
| Loft at normal tolerance 0.001 / 0.01 / 0.05 / 0.1 | 754k / 409k / 22k / 11k triangles |

Takeaway: the kernels are fast and tiny. **Tessellation tolerance dominates the cost**, so the
real engine needs separate preview and export resolutions.

## Run it

```sh
cargo run --release --bin probe                      # native timings at 4 tolerances
rustup target add wasm32-unknown-unknown
cargo build --release --lib --target wasm32-unknown-unknown
node run.mjs target/wasm32-unknown-unknown/release/probe.wasm
```

`run.mjs` stubs the wasm-bindgen imports. They come from `getrandom` (pulled in by `curvo`'s
optimizer dependency) and are never called by this code.
