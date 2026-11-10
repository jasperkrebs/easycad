# EasyCAD and Grasshopper: what is possible

*Research, 10 Nov 2026. Nothing here is built. The reader is the owner, who knows Grasshopper well
and is not a programmer by trade. Every claim about Rhino or Grasshopper has a link. **[unverified]**
marks something I could not confirm from a primary source, **[inference]** something I worked out
myself. I had no Rhino at hand: nothing below was tried.*

Versions in play: Rhino 8 is current. Rhino 9 and Grasshopper 2 (GH2) are in beta/WIP, with no
release date from McNeel ([forum, 2026](https://discourse.mcneel.com/t/release-date-of-grasshopper-2-commercial-version/217834);
the GH2 beta thread says its SDK is "still evolving" and GH1 plugins do not load in it:
[Grasshopper 2 beta](https://discourse.mcneel.com/t/rhino-beta-feature-grasshopper-2-beta/221336)).
Everything below targets Rhino 8 / GH1. Rhino 7 and GH2 get a line where they differ.

---

## 1. Summary and recommendation

**The wish has two halves, and they are not the same job.**

1. *"Open a toolpath in Grasshopper."* Cheap and safe. Do it first.
2. *"Build inside the app, less by asking an AI."* This is an EasyCAD UI question. Grasshopper
   files do not solve it. Do it in parallel.

**What I recommend, smallest first.** Each stage is usable alone.

| # | Stage | Size | What it unlocks | Main risk |
|---|---|---|---|---|
| 1 | **Toolpath export for Rhino/GH**: a `.3dm` (rhino3dm.js, loaded only on click) and a plain JSON/CSV with per-move data, plus a ~40-line Python 3 script that reads the JSON into polylines and data trees | S | See, measure, bake, post-process any print in Rhino/GH today. Works on Mac, Windows, Rhino 8 | Per-move data (width, height, flow) has no native home in a `.3dm`: it needs the sidecar file (§2) |
| 2 | **The real engine inside a script component**: recipe text + settings in, toolpath and G-code out, through a Python 3 script with a Rust wheel (or Wasmtime). No plugin | M | The recipe runs in Grasshopper with identical results, same Rust code, G-code goldens still apply. Sliders in GH drive the recipe | Packaging on Mac (signing, wheel tags); two experiments (§8) decide it |
| 3 | **Read Grasshopper into EasyCAD, the narrow way**: a script *inside* GH dumps number sliders, toggles and curves to JSON, EasyCAD turns them into `param` lines and curves | M | Take a designer's curves and sliders from GH into a recipe without parsing `.gh` | Only parameters and geometry come across, not logic |
| 4 | **Build a definition inside GH from a recipe** (a script creates the components and wires with the GH_Document API) for the ~65 steps with a native component, a script component for the rest | L | A recipe the user can edit in Grasshopper, step by step | Results differ from the engine; two sources of truth. Do only if stage 2 proves not enough |
| 5 | **Parse `.ghx` in EasyCAD** | L | Open arbitrary definitions | Plugins and scripts are opaque; data trees do not fit the language. Probably never worth it beyond stage 3 |

**What I would not do:** write `.gh`/`.ghx` from the browser (fragile, §5); a C# transpiler as the
main route (a second engine, §3); anything for GH2's file format (§5).

**In parallel (the "build without the AI" half, §7):** a *step list with inline pickers* and
*tap-a-curve-in-the-Plan-or-3D-view, get the steps that accept it*, built on `mark` and on the
types the node table already carries. Not a graph editor.

**G-code rule.** Whatever reaches Grasshopper, G-code for a printer is written only by the
Rust engine, which has the golden tests. A transpiled C# script or a hand-built GH definition
is a *design* tool; it must not emit printable G-code (§3).

---

## 2. Question 1: a finished toolpath into Rhino/Grasshopper (data, not logic)

### What a toolpath holds

From `crates/core/src/toolpath.rs`: per move that ends at a point: position, travel flag, flow
(fraction of a full bead), speed factor, width and layer-height overrides, turn position (spirals),
part index, plus layer start indices, per-part settings and machine events (fan, dwell, temperature).
That is ~8 numbers per point plus a few lists.

### Options

**A. JSON/CSV plus a small GH script (most flexible).**
- Nothing in Rhino stores "width of this move" except user data. A script (Python 3 or C# script
  component, Rhino 8) reads the file and builds what the user wants: polylines per layer/part and
  parallel data trees (width, height, flow, speed, travel).
- Layout: one binary-ish JSON (arrays of numbers) or CSV with columns `part, layer, x, y, z,
  travel, flow, width, height, speed`. [inference] Size: 100k points, CSV ~5 MB (about 1.5 MB
  gzipped), 1M points ~50 MB (a typed binary file, 8 numbers as float32, is 32 MB).
- Effort: S. No dependency, works in Rhino 7 (IronPython/C# components) and 8 (Python 3, C#), on
  Windows and Mac. Grasshopper on Mac exists from Rhino 8
  ([Rhino 8 for Mac feature list](https://www.rhino3d.com/mac-feature-compare), found via search,
  not fetched; I did not verify what Rhino 7 for Mac had).

**B. `.3dm` written in the browser with rhino3dm.js.**
- rhino3dm is MIT-licensed, runs in the browser as WASM, and can read and write `.3dm`
  ([github.com/mcneel/rhino3dm](https://github.com/mcneel/rhino3dm)). Latest on npm is 8.35.0.
- Size, measured today from the npm package: `rhino3dm.wasm` 2.67 MB raw, **about 1.0 MB
  gzipped**; the JS wrapper 114 KB minified
  ([jsDelivr file list](https://data.jsdelivr.com/v1/packages/npm/rhino3dm@8.35.0?structure=flat)).
  EasyCAD's own wasm is 1.2 MB (`web/pkg`), so load rhino3dm only when "Export .3dm" is tapped.
  The README itself says the wasm is "fairly large for browser based applications"
  ([README 8.35.0](https://cdn.jsdelivr.net/npm/rhino3dm@8.35.0/README.md)).
- Browser support: the default build is wasm32. A separate wasm64 build exists (8.32.1+, 16 GB
  limit from 8.32.2) for very large files; Safari does not support wasm64, so McNeel kept wasm32
  as default
  ([McNeel forum, Aug 2026](https://discourse.mcneel.com/t/rhino3dm-js-request-for-wasm64-memory64-build-to-lift-2-gb-heap-limit/218345)).
  The GitHub README mentions Safari only in a Technology Preview for 64-bit memory; read with the
  forum post, I take that to mean wasm64 only. [inference] A user hit the ~2 GB wasm32 limit
  exporting large meshes, so for polylines of 1M points (tens of MB) wasm32 is not the problem.
- What the API offers (read from the package's `rhino3dm.d.ts`, 8.35.0): `File3dm`,
  `objects().addPolyline(points, attributes)`, `PolylineCurve`, `layers().add(layer)`,
  `ObjectAttributes.setUserString(key, value)`, `toByteArray()`, and a `PointCloud` whose points
  can each carry a normal, a colour **and one number** (`addPointNormalColorValue`).
- Where each bit of toolpath data can live:
  - Geometry: one polyline per (part, layer) or per continuous extruding run. Travel moves as a
    separate layer called `travel`.
  - Units: a `.3dm` has a unit system; set millimetres. [unverified] that rhino3dm.js exposes the
    setting; check the `File3dm` settings in the `.d.ts`.
  - Part and layer: Rhino layers (one per part, sub-layers per print layer).
  - Per-object data (part's line width, layer height, flow, printer): `setUserString` on each
    polyline's attributes. User strings are carried through file I/O
    ([McNeel C++ guide](https://developer.rhino3d.com/guides/cpp/adding-user-strings-to-objects/)).
    Reading them back in rhino3dm.py has had problems
    ([forum](https://discourse.mcneel.com/t/rhino3dm-py-cant-read-object-user-strings-from-existing-file/108245)),
    so test reading in Rhino itself.
  - **Per-vertex data** (bead width changing along a move): a polyline has no per-vertex attribute.
    Options: a point cloud with one value per point (width), the rest in a sidecar; or a JSON/base64
    string of arrays in one user string [unverified: no documented size limit found]. I would not
    rely on either. Use the sidecar (option A) for per-move data and treat the `.3dm` as geometry plus
    labels.
- Effort: S to M (one function, plus a test that Rhino opens the file). Same sidecar story.

**C. G-code readers that already exist.**
- Search found no Grasshopper plugin that imports G-code into polylines. Found: a forum thread of
  text-parsing in Grasshopper ([Gcode to Grasshopper](https://discourse.mcneel.com/t/gcode-to-grasshopper/80139)),
  RoboDK's plugin, which loads G-code ([RoboDK toolbar](https://robodk.com/doc/en/Plugin-Rhino-Grasshopper-Toolbar.html)),
  and Ovenbird, which has its own Toolpath type ([from-to-curves](https://yefan-zhi.github.io/ovenbird/io/from-to-curves/from-to-curves.html)).
  Not an established route.
- [inference] G-code carries `E` and `F`, not bead width, layer height or part, so reading our own
  G-code back loses exactly what makes the toolpath ours. Do not change the G-code to add comments:
  it is golden-tested. Use a separate file.

**D. A custom user object / own Toolpath type in GH.** JÜNGERKÜHN's new TOOLPATHS plugin has a
"Toolpath object" that combines geometry and print settings, with speed, flow, temperature
settable per segment or vertex; Windows only as far as the thread says, commercial, G-code output
([beta thread, 30 Sep 2026](https://discourse.mcneel.com/t/toolpaths-3d-printing-plugin-beta-release/223142)).
Mapping our toolpath onto their object is a possible later interop, not a first step. [unverified]
whether it has an import API.

**Recommendation for Q1:** A first (JSON plus the script), B second (a `.3dm` for people who want
to open it in Rhino without Grasshopper). Both are S–M. They also form the base the later stages
reuse, because stage 2's output is the same data.

---

## 3. Question 2: a recipe as a Grasshopper script component

### How script components work in Rhino 8 (and 7)

- **Rhino 8:** the new Script component runs C#, Python 3 (CPython) or IronPython 2; the language is
  chosen from the component's menu
  ([Script Component: Grasshopper](https://developer.rhino3d.com/guides/scripting/scripting-component/),
  updated 29 Sep 2026). Inputs and outputs are added with the zoom-in ⊕/⊖ controls; a *type hint* is
  set per input. The page does not cover item/list/tree access. [inference] from the usual
  component menu: access is set per input there too.
- **Packages:** Python 3 uses pip (`# r: numpy` / `#requirements: numpy`), C# uses NuGet
  (`#r "nuget: RestSharp, 110.2.0"`); pressing Run installs them first. Alternate package indexes
  and git installs are covered by [Script Package References](https://developer.rhino3d.com/guides/scripting/advanced-packages/)
  (linked from that page; I did not read it). Some users reported NuGet references failing before
  Rhino 8 SR14 ([forum](https://discourse.mcneel.com/t/rhino8-scripteditor-where-to-find-nuget-package-locations/185433),
  [forum](https://discourse.mcneel.com/t/script-editor-c-referenced-assemblies/192304)).
- **Pasting a script:** [unverified] I found no documented way to load a `.cs` file into a script
  component other than opening the component's editor and pasting the text. The `#load`
  directive: not found in official docs. Treat "export a recipe as a C# script" as "export text
  the user pastes into a Script component".
- **Does the script live in the `.gh`?** [unverified]. A forum thread suggests a "flat script" mode
  where the code is embedded in the compiled component
  ([forum](https://discourse.mcneel.com/t/rhino-8-python-grasshopper-component-code-property/180653)).
  Test it (§8).
- **Legacy components:** the old Rhino 7 C# component (with "Manage Assemblies") is gone from the
  Rhino 8 C# component; legacy ones are still available by typing `#` on the canvas
  ([forum](https://discourse.mcneel.com/t/what-happened-to-manage-assemblies/174440)). Rhino 7's
  GhPython is IronPython 2, no pip
  ([GhPython guide](https://developer.rhino3d.com/guides/rhinopython/7/ghpython-component/)).
  So: a script written for Rhino 8's Python 3 does not run in Rhino 7.

### Exporting a recipe as one C# or Python script

The target: a script that recomputes the recipe from its text, in RhinoCommon, with the settings
as inputs. What a transpiler needs: a mapping for each node in `crates/core/src/nodes.rs`
(112 nodes in 7 groups: 40 Numbers & lists, 34 Curves, 18 Toolpaths, 7 Transforms, 5 Surfaces,
5 Points & planes, 3 Fields). My estimate from the groups and names, **not a step-by-step audit**:

| Class | Steps | Share | Examples |
|---|---|---|---|
| **Trivial**: same thing exists in .NET / GH, or a few lines | ~65 | ~58% | `range`, `series`, `item`, `reverse`, `shift`, `flatten`, `graft`, math (`sin`, `lerp`, `clamp`…), `point`, `vector`, `plane`, `circle`, `ellipse`, `rectangle`, `arc`, `line`, `polyline`, `polygon`, `star`, `move`, `rotate`, `scale`, `mirror`, `length`, `flip`, `join_curves`, `subcurve`, `tangent`, `normal`, `closest`, `centroid` |
| **Real work, similar not identical**: RhinoCommon has an equivalent but a different algorithm | ~15 | ~13% | `interpolate`, `catmull_rom`, `smooth`, `offset` (and mitre), `fillet`, `tween`, `loft`, `revolve`, `extrude`, `contour`, `divide`, `resample`, `simplify`, `seam`, `evaluate` |
| **Our own code**: nothing to call | ~30 | ~27% | all 18 toolpath steps (`toolpath`, `vase_spiral`, `vase_revolve`, `spiralize`, `join`, `bead`, `adjust`, `accelerate`, `fan`, `dwell`, `temperature`, `speed_override`, `extrusion_override`, `spikes`, `wave`…), `rings`, `link_rings`, `leg_rings`, `interlock`, `spiral`, `graph`, the 3 field steps, and the bead/deposit model |

**List replication.** The language has Grasshopper-style list matching (see the `Access` enum in
`nodes.rs`: item, list, point, tree). In a script you reimplement it once, as a helper that wraps
each step. That is a few hundred lines, written once. [inference] Fields (numbers that vary over
space) have no GH equivalent and need our own small evaluator; Graph Mapper is not scriptable as
a function, so `graph` is our own code too.

**Identical results?** No, and not worth chasing for the middle class. NURBS fits (`interpolate`,
`smooth`, offset, fillet) differ between `curvo` (our engine, pure Rust) and RhinoCommon; even
tiny differences matter for G-code because toolpaths are cut at chord tolerance. The lampshade's
golden test matches EasyGCode "command for command" (`web/templates/lampshade.easycad`). A C#
re-implementation would need the same golden tests, run against C# output, and would need to
reproduce the bead model to the same numbers.

**G-code safety.** CLAUDE.md: never emit printable G-code unless golden-tested. A C# script
that re-implements the toolpath steps is a second implementation of the part that has the golden
tests. So: it may produce **preview toolpaths** (polylines for design in GH), it must not write
G-code; the file labelled "G-code" always comes from the Rust engine. If the recipe's printable
output must be proven, the C# route would need its own golden suite, i.e. it doubles the work
the project is protecting itself from.

**Honest size.** The trivial class and the helper layer: M. The middle class: M, endless
tuning. The own-code class: L, and it is a rewrite of the Rust toolpath code in C#, which is
the opposite of "one engine everywhere". **I do not recommend the transpiler as the main route.**
If a "recipe as a C# component" is still wanted for the sake of portability, the cheap version is
a script that embeds the *recipe text* and calls the engine (§4), so the text is the component.

---

## 4. Question 3: running the Rust engine inside Grasshopper

All four routes keep one engine, so results and G-code are the same Rust code. [inference] Two
caveats: the engine already runs as wasm in the browser and natively in CI, so a floating-point
difference between targets (different `sin`/`cos` implementations) is a property we already live
with. Run the golden tests on each new target before trusting its G-code.

### (a) Native library (cdylib, C ABI), P/Invoke

- Build `easycad_core` as a `cdylib` with a few `extern "C"` functions: `eval(recipe, settings_json)
  -> buffer`, `free(buffer)`. Everything already never panics (CLAUDE.md), but wrap with
  `catch_unwind` anyway [inference].
- McNeel's own guidance is P/Invoke, with one native binary per platform (dll, dylib)
  ([Wrapping Native Libraries](https://developer.rhino3d.com/guides/rhinocommon/wrapping-native-libraries/);
  written 2021 for Rhino 5/6, it says nothing about Rhino 8).
- **Runtime:** Rhino 8 runs .NET Core on Windows and Mac (Windows can switch to .NET Framework;
  Mac is .NET only) ([forum](https://discourse.mcneel.com/t/rhinoceros-8-netfx-vs-fallback-to-netfx-from-netcore-difference/174020),
  [forum](https://discourse.mcneel.com/t/how-to-develop-grasshopper-components-for-rhino-8-using-net-7/165957)).
  A newer Cordyceps listing says it requires .NET 8 and Rhino 8.21+, so the runtime has moved on in
  later 8.x releases ([listing, third party](https://lobehub.com/mcp/brookstalley-cordyceps)).
- **Mac caveats** (forum reports, not McNeel statements): dylibs must be universal or arm64 and
  there were `dlopen` failures with relative paths under the hardened runtime
  ([forum](https://discourse.mcneel.com/t/rhino-8-c-dll-procedures-not-found-works-fine-in-rhino-7/177167),
  [forum](https://discourse.mcneel.com/t/problem-loading-librhino3dm-native-on-mac-rhino-8/173008));
  a downloaded unsigned dylib is also likely to be quarantined by Gatekeeper [unverified].
- **Without a plugin?** A C# script component can call `[DllImport]`/`NativeLibrary` with an absolute path
  [inference, not tested]. The user must put the binary somewhere first. As a plugin (.gha) it
  ships through Yak, which tags packages per platform (`win`, `mac`, `any`) and Rhino version
  ([Yak guide](https://developer.rhino3d.com/guides/yak/creating-a-grasshopper-plugin-package),
  [forum](https://discourse.mcneel.com/t/yak-backwards-compatible-packages-no-compatible-distribution-found-for-scriptparasite-1-0-4/111580/2)).
  Our repo has no C# toolchain; a .gha is a new language to maintain.
- Effort: M (script + loader), L (a signed, notarised, Yak-distributed plugin for both OSes).

### (b) The existing WebAssembly build under Wasmtime in .NET

- `wasmtime-dotnet` is the Bytecode Alliance's .NET embedding, Apache-2.0, on NuGet as `Wasmtime`
  ([repo](https://github.com/bytecodealliance/wasmtime-dotnet)). I could not confirm its supported
  platforms (osx-arm64, win-x64) from the repo page [unverified]; check the package file list.
- It could be referenced from a script with `#r "nuget: Wasmtime, <ver>"` (same mechanism as
  above) [inference, untested]. One portable `engine.wasm`, no per-platform binary of ours; but
  Wasmtime's own native library is per platform, so the signing question moves, it does not vanish.
- Today's wasm is built with wasm-bindgen (JS glue). It cannot be called plainly from .NET; a
  second build with a plain C-style export set (the same `eval`/`free` as in (a)) is needed. Size is
  1.2 MB raw (`web/pkg`), inside the 3 MB gzipped budget.
- Effort: M.

### (c) Rhino 8's CPython with a Rust wheel (PyO3) or the `wasmtime` Python package

- Rhino 8's Python 3 script component installs pip packages by itself
  ([doc](https://developer.rhino3d.com/guides/scripting/scripting-component/)), so `# r: easycad`
  at the top of a script is the user's whole install step. No plugin, same on Windows and Mac.
- A PyO3 wheel built with `maturin` and tagged abi3 would not depend on the Python version Rhino
  embeds [inference; I did not find which CPython Rhino 8 embeds]. It needs wheels for win-x64,
  macOS arm64 (and Intel if wanted), hosted on PyPI or a private index (alternate indexes are
  supported, see the package-references page above). Our licence is "all rights reserved": a wheel
  on PyPI is fine for distribution; the licence is ours to set [inference].
- The `wasmtime` Python package would run (b) without a native build of ours [unverified: package
  and wheel coverage not checked].
- Experience: user drops a Python 3 Script component, pastes 15 lines, gets inputs for the settings
  and outputs for polylines and data trees. Wheel installation runs on first Run; the editor is
  disabled during installation (doc above).
- **This is the best no-plugin route in my reading.** Effort: M. Main risk: pip behaviour inside
  Rhino on Mac and a wheel that Gatekeeper accepts [both unverified].

### (d) Hops / Rhino.Compute-style service

- Hops puts a Grasshopper definition (or a REST endpoint) behind a component. Inputs come from
  `Get` components (Params > Util), outputs from Context Bake/Print; `At Most` 1 is item access,
  more is list, Tree Access a data tree; Min/Max/presets apply only in the Grasshopper Player;
  the path can be a file, URL or REST endpoint
  ([Hops component](https://developer.rhino3d.com/guides/compute/hops-component/)).
- Hops works in Rhino 7 and 8 for Windows. Rhino.Compute does not run on macOS; Mac users can call a
  remote Windows server or a local Python `ghhops_server` (same page). So a **local EasyCAD server
  that speaks the Hops protocol** would work on both OSes without Rhino.Compute. [unverified] The
  protocol details (how an endpoint describes its inputs, whether sliders can be offered) I did not
  read; the Python `ghhops_server` source is the reference.
- Hosted version: the app is on GitHub Pages (static), so a hosted endpoint is a new server we
  would run. The local `easycad-cli` (native, in the architecture doc) could serve it instead.
- Rust precedent: RustHopper talks to Rhino.Compute from Rust over HTTP
  ([RustHopper](https://github.com/hrntsm/RustHopper), MIT). It is the opposite direction from what
  we want, but it shows nobody has Rust running *inside* Grasshopper.
- Experience: the component shows input sockets named after the recipe's settings; the user wires
  sliders; polylines come back. Sliders are not created for you [inference].
- Effort: M for a local server; the Windows-only Rhino.Compute route is not needed.

### Comparison of the four (detail in §9)

| | no plugin install | Mac | one engine | effort |
|---|---|---|---|---|
| (a) native + P/Invoke | script: user places a binary; plugin: Yak | yes, signing issues | yes | M / L |
| (b) Wasmtime | script, NuGet | likely, unverified | yes | M |
| (c) Python wheel | yes, pip on first Run | yes, unverified | yes | M |
| (d) Hops server | needs Hops (ships with Rhino 8) | yes via REST | yes | M |

---

## 5. Question 4: writing real Grasshopper definitions (`.gh`/`.ghx`)

### The format

- `.gh` is binary, `.ghx` XML, both a "serialised dictionary": the file format knows nothing of
  Grasshopper, so nothing validates; each component decides how it stores itself, so the output
  of a component type cannot be predicted from the type alone (David Rutten, 2019:
  [forum](https://discourse.mcneel.com/t/create-ghx-files-from-python-environment-without-grasshopper/85881)).
  The wiki says there is "no schema" and that `.ghx` is editable
  ([McNeel wiki, last updated 2008](https://wiki.mcneel.com/labs/grasshopper_fileformat)).
- `GH_IO.dll` reads and writes both, as a .NET library depending only on framework assemblies; it
  ships with Grasshopper (same wiki). McNeel's API docs call it independent of Grasshopper.dll
  ([API: GH_IO](https://developer.rhino3d.com/api/grasshopper/html/N_GH_IO.htm), seen only as a search snippet).
- **Licence conflict:** the 2008 wiki says GH_IO is GPL. In July 2023 Dale Fugier (McNeel) wrote that
  GH_IO "is not licensed under GNU General Public, and you may not distribute it with some application
  you write", quoting the Rhino EULA; others pointed at the wiki; it was left to be looked into, and
  no resolution is on the page
  ([forum](https://discourse.mcneel.com/t/gh-io-dll-licensing/163385)). **Treat GH_IO.dll as not
  redistributable.** Using it inside a user's own Rhino (a script running in GH) is fine, shipping
  it with EasyCAD is not.
- **No non-.NET writer or parser** found on GitHub. Found: a 2019 JavaScript proof of concept that
  composes `.ghx` in the browser, with component GUIDs gathered by hand and later generated from
  `Instances.ComponentServer.ObjectProxies`, supporting a handful of components
  ([Resthopper](https://discourse.mcneel.com/t/resthopper-client-side-ghx-composition-poc/91412), repo
  `cdriesler/resthopper`); a `.gh` to `.ghx` converter app
  ([gh2ghx](https://gh.hhj520.top/tsevis/gh2ghx), seen via search); and `moult`, which pulls script
  text out of definitions. No complete reader or writer.
- **Fragility, in practice:** a 2026 forum thread where someone had Claude generate Grasshopper
  files: first a file with the right components but no wires; after ~25 attempts a Python script
  that wrote a loadable `.gh`, still needing manual fixes
  ([forum](https://discourse.mcneel.com/t/can-claude-do-gh-part-1-1-1-2-and-1-3/218637)). A related
  thread says plain generated XML from ChatGPT is not readable by Grasshopper
  ([forum](https://discourse.mcneel.com/t/can-chatgpt-generate-valid-gh-files-for-grasshopper/204387)).
  So generating files outside Grasshopper means mirroring every component's private serialisation
  and every plugin version's GUIDs. I would not.

### The robust way: build the definition from inside Grasshopper

A script (C# or Python 3) running in Grasshopper can create components and wires through the
Grasshopper API: `GH_Document.AddObject`, `Instances.ComponentServer.EmitObject` to make a
component from its GUID, position via `Attributes.Pivot`, and wires with
`Params.Input[i].AddSource(...)`
([AddObject](https://developer.rhino3d.com/api/grasshopper/html/M_Grasshopper_Kernel_GH_Document_AddObject.htm),
[forum](https://discourse.mcneel.com/t/adding-component-to-canvas-by-name/152533),
[forum](https://discourse.mcneel.com/t/can-i-instantiate-specific-component-on-the-canvas-with-a-script-python/74204)).
Existing tools do this: Cordyceps (add components, wire, set values, groups; Rhino 8.21+, Win/Mac,
[forum, Jan 2026](https://discourse.mcneel.com/t/cordyceps-mcp-server-for-grasshopper-win-mac/215122)),
and other Grasshopper MCP servers listed in `docs/research.md` §3.2. The SDK is "pretty lackadaisical
about enforcing the rules" per one forum user (same forum search), so expect version quirks. The
file Grasshopper then saves is correct by construction. [inference] This also sidesteps the licence
question.

**What a recipe to definition mapping would look like:**
- `param` → Number Slider (range, step, value), Boolean Toggle, Value List (dropdown). `group`s → GH
  Groups. `note` → Panel.
- A step with a native component → that component, wired. List matching comes free.
- A step of ours (toolpath, rings, fields, bead model) → **one script component** per step, with the
  step's Rust-engine call or a C# body inside; or one "EasyCAD" component wrapping the engine (§4 c).
  A *cluster* of components is the editable alternative for composite steps; clusters are also
  serialised dictionaries, so only worth it if built through the API in-app.
- `show`/`print` → Preview/Bake; `export` → script that writes the G-code from the engine.
- Layout is not in recipes; auto-layout by dependency depth is easy.

**Honest read:** this is stage 4 (L). The editable-in-Grasshopper promise holds only for steps
that map to native components; for the rest the user edits a script, not a graph. And once the user
changes the definition, nothing keeps it in step with the recipe: two sources of truth. A one-way
export ("snapshot for exploration") is acceptable; round trip is where it breaks.

### Grasshopper 2

GH2 uses a new format, extension `.ghz` in the beta's example files; GH1 files open in GH2 through a
GH1 layer running in the background, "not a recommended workflow", and GH1 plugins do not load in GH2
([beta thread](https://discourse.mcneel.com/t/rhino-beta-feature-grasshopper-2-beta/221336)). The GH2
SDK is moving: one developer calls it "a versioning nightmare"
([forum, 2025](https://discourse.mcneel.com/t/new-grasshopper-2-0-alpha-2025-feb/199833), seen via
search). GH2 has C# and Python 3 script components on the same editor stack. **Do not target the
GH2 format.** Anything that is a script is portable; anything that is a file is not.

---

## 6. Question 5: reading Grasshopper files into EasyCAD

**Parsing `.ghx`.** It is XML, so a browser or Rust parser can read it. The structure (from my
memory of ghx files, not verified here; look at one before designing): a root archive, a
`Definition` chunk, a `DefinitionObjects` list with one chunk per object holding the component's
GUID, name and instance GUID, its input/output parameters, and for each input the *source* instance
GUIDs, which are the wires. Number sliders store their value, range and step; panels their text;
internalised data is stored inside parameter chunks. Because each component serialises itself
(Rutten, above), a parser has to know each component it wants to understand.
`.gh` (binary) needs GH_IO or a reverse-engineered reader; a forum answer calls the latter
feasible but "likely not worth the effort"
([forum](https://discourse.mcneel.com/t/can-chatgpt-generate-valid-gh-files-for-grasshopper/204387)).
Users can save as `.ghx` in Grasshopper [unverified: where the option is in Rhino 8].

**How much of a typical definition could be understood?** [inference]
- Number sliders, toggles, panels: yes (parameters).
- Internalised points and curves: yes, if the curve kind is plain (polyline, NURBS control points).
  Our curves are NURBS via `curvo`, so we can hold them.
- Native components that match a step (`Circle`, `Move`, `Loft`, `Divide Curve`, `Series`, `Range`):
  a subset, one by one, each with its input names and its GH defaults mapped.
- Data trees with paths: the language has nested lists, no paths. Most real definitions use
  Graft/Flatten/Path Mapper; mapping them to nested lists works for depth 1–2, then it breaks.
- Plugins (Weaverbird, Kangaroo, LunchBox, TOOLPATHS…), scripts, clusters, Hops calls: opaque.
  C#/Python script text can be read but not run (EasyCAD has no script step, by design: the language
  has no loops or file access).
- Data-dependent components (Random, Graph Mapper, time) differ.

**Realistic subset, in order of value:**
1. **Our own export, round trip.** A definition that EasyCAD's script (stage 4) built carries a
   recipe hash and a "source" panel; read back by GUID rather than by guessing. Cheap if stage 4 exists.
2. **Parameters plus geometry from any definition**, extracted *by a script running inside Grasshopper*
   (stage 3): walks `GH_Document.Objects`, writes JSON of slider values and ranges, toggles,
   panels and the geometry on selected parameters (curves as control points). No `.gh` parsing and no
   GH_IO redistribution. EasyCAD turns sliders into `param`s and curves into curve literals. **This
   is the useful 80% for the effort.**
3. **Parse `.ghx` for a hand-picked list of components.** L. Only when a user has real
   definitions to bring over; the list should come from those definitions, not from a guess.

---

## 7. Question 6: building in the app without the AI

### How others do it

| Tool | What it does for no-code building | Lesson for EasyCAD |
|---|---|---|
| **Grasshopper** | A node canvas; wires, lists, trees; sliders on the canvas | The reference. Desktop-sized; hard on a phone [inference] |
| **Nodi 3D** | A Grasshopper-like node editor in the browser, open source (Apache-2.0 per a listing), beta; exports obj/stl/dxf ([listing](https://alternativeto.net/software/nodi/about), repo `Nodi3d/nodi`) | A node editor in the browser is proven, but it is its own geometry kernel |
| **NodePen** | A web client for Grasshopper that runs the real Grasshopper engine headless ([listing](https://www.aecplustech.com/tools/nodepen)) | Shows the "hosted GH" route; needs a Rhino server |
| **ShapeDiver** | Uploads a GH definition; a viewer API and App Builder expose its parameters as UI; computation on their servers ([ShapeDiver docs](https://help.shapediver.com/doc/get-started-developers)) | Settings window plus viewer, like ours; the logic stays hidden |
| **Hops / Grasshopper Player** | Definition as a function; sliders only | Same idea |
| **Blender Geometry Nodes** | Node graph as a modifier; direct manipulation of node values in the 3D view only through add-ons ([docs](https://docs.blender.org/manual/en/3.0/modeling/geometry_nodes/introduction.html), [add-on](https://blendermarket.com/products/node-parameter-display)) | People ask for what we have in `mark`: touch the thing, reach the parameter |
| **FullControl GCode Designer** | Print paths designed as code/spreadsheet, now also a browser app and Python package ([all3dp](https://all3dp.com/2/fullcontrol-gcode-designer-simply-explained/), [printer.tools](https://printer.tools/tools/fullcontrol/)) | The same audience wants path-level control; its entry is a gallery of designs plus parameters |
| **jüngerkühn TOOLPATHS** | Grasshopper plugin; Toolpath object with settings global, inherited or local, and a scrubbable preview ([beta thread](https://discourse.mcneel.com/t/toolpaths-3d-printing-plugin-beta-release/223142)) | "Settings inherited down, overridden locally" is the model we already have per part |
| **Cuttle** | Browser design app; parameters first, scripting second ([Fab Academy notes](https://fabacademy.org/2025/labs/aalto/students/forrest-oliphant/fab-academy/02-cuttle-xyz.html)) | Direct manipulation and programmability in one place, no graph |
| **Onshape FeatureScript UI, nTop blocks** | Not researched: no usable sources found in this pass | |

### What would fit EasyCAD

Constraints from the repo: phone-first, "no explanations inside the app", the recipe is the single
source of truth, everything is a command, the Graph tab was cut on 1 Nov 2026 for a presentation
(its edit features, if any, I did not check in the devlog).

1. **A graph editor again.** Familiar to a Grasshopper user; poor on a phone; two views to
   keep in step. A read-only graph (what feeds what) is cheap, an editing one is L. I would not
   bring it back first. [inference]
2. **A step list with inline pickers.** The recipe is already a list of named steps. Show it as
   rows; each row is a step with its inputs as chips: a name from the earlier steps (picker lists
   only values of the right kind), a number, a field. "Add step" opens the catalogue filtered by
   what is selected. Each action is an existing recipe rewrite, so the text stays the truth.
   The data is there: `nodes.rs` carries each input's `kind` (`Curve`, `Number | Field`,
   `List<Number>`…), `access` and `output`, so "which steps accept a curve" is a query, not new
   knowledge. Effort M.
3. **Direct manipulation in the Plan and 3D view.** Tap a curve → what it *is* (which line made it)
   and what can be done with it: the steps whose inputs take that kind (offset, divide, loft,
   spiralize…), each a new recipe line with its settings. Tap a point/handle → drag, written back
   into the `param`. Effort M for curves in the Plan, L for 3D picking.

**How this builds on `mark`.** Today (devlog 9 Nov 2026) `mark curve for setting, … "Label"` names
the settings that drive a feature, and a tap in the Plan opens the first setting that shows and lights
them all. That is *curve → settings*. Options 2 and 3 are *curve → steps*:
- `mark` stays as the authored "drives" link; add a derived "made by" link: every value already
  knows which step line created it, so tapping any curve can show its line and settings without
  anyone writing a `mark`. [inference; check the evaluator keeps the step name per value]
- The picker's choices come from the node table's types; the settings it writes use the existing
  `addPart`-style commands (`docs/lamp-builder.md`).
- The agent stays optional: it uses the same commands the picker does.

**My pick:** 2 first (the step list), then 3 for the Plan view since the lamp builder lives there.
Both are inside the app, need no Rhino, and make recipes by hand a normal act. Grasshopper
interop (stages 1–3) does not replace this and does not depend on it.

---

## 8. Experiments that need a real Rhino (10–30 minutes each)

Do these before committing to stage 2.

1. **Wheel in a Python 3 Script component (Win + Mac).** In Rhino 8, paste
   `# r: <small pure package>` (e.g. `numpy`), run; then a local wheel path or private index with a
   tiny PyO3/abi3 "hello" wheel. Record: Python version Rhino embeds (`sys.version`), install time,
   and whether macOS arm64 accepts a non-notarised wheel. Decides (c).
2. **Wasmtime from a C# Script component.** `#r "nuget: Wasmtime, <latest>"`, instantiate a 3-line
   wasm module, call an `add`. Win and Mac. Decides (b).
3. **P/Invoke a 10-line Rust cdylib from a C# Script component (Win + Mac arm64).** Absolute path;
   unsigned vs ad-hoc-signed; quarantine flag on a downloaded copy. Decides (a) and whether plugin
   packaging is needed.
4. **Open a hand-built `.3dm`.** From rhino3dm.js (node is fine): 3 polylines on 2 layers,
   user strings, one point cloud with a value per point. Open in Rhino 8; read user strings in a GH
   script (RhinoCommon) and Rhino Data Types; does the point-cloud `value` survive? Check the units.
5. **Load test.** A 1M-point polyline and the same split into 1,000 layer polylines: file size, open
   time, GH preview and bake speed. Compare JSON versus `.3dm` loading.
6. **Hops against a toy endpoint.** A 20-line local Python `ghhops_server` function with two numeric
   inputs; see what Hops shows for inputs and ranges, and whether a Rust server can answer the same
   protocol. Also on Mac.
7. **Build a mini definition by script.** From a Python 3 and from a C# Script component: add a Number
   Slider, Circle, Move, Preview; wire them; place them. Then save the file as `.ghx` and open the XML:
   check the slider GUID (I recall `57da07bd-ecab-415d-9d86-af36d7073abc` [unverified]), how a wire
   appears, and whether a Rhino 8 script component's source is readable in the file.
8. **Dynamic inputs.** Can a script component add its own input parameters from a list (the recipe's
   settings) and create and attach sliders on first run? [forum pattern](https://discourse.mcneel.com/t/create-input-components-from-within-ghpython-component/154667)
   (title seen via search only).
9. **A TOOLPATHS trial.** Install the beta (Windows) and see whether its Toolpath object has an
   import or scripting API we could feed.
10. **Ask McNeel** (Dale Fugier / forum) for a clear answer: may GH_IO.dll be redistributed with a
    standalone application that only reads and writes definitions?
11. **Rhino 8 runtime.** Which .NET version does Rhino 8 (current service release) run on Windows
    and on Mac? (Sources above disagree by date: .NET 7, later .NET 8.)

---

## 9. Comparison of the routes

| Route | Rhino 7 | Rhino 8 Win | Rhino 8 Mac | Plugin to install? | Editable in GH? | Same results as engine? | Effort |
|---|---|---|---|---|---|---|---|
| 1A JSON/CSV + script | yes (IronPython/C#) | yes | yes | no | the script, and the data | yes (data from the engine) | S |
| 1B `.3dm` via rhino3dm.js | yes (opens in Rhino) | yes | yes | no | geometry only | yes (data) | S–M |
| (a) cdylib + P/Invoke, script | no (different runtime) [inference] | yes | yes, signing risk | no, but a binary to place | no | yes | M |
| (a) as `.gha` via Yak | probably (net48 target) [inference] | yes | yes | yes (Yak) | no | yes | L |
| (b) Wasmtime in C# | no | yes | likely, unverified | no (NuGet) | no | yes | M |
| (c) Python wheel | no (needs Python 3) | yes | yes, unverified | no (pip) | recipe text and inputs | yes | M |
| (d) Hops + local server | yes (Hops) | yes | yes via REST | Hops is included in Rhino 8 [unverified for Mac] | inputs and outputs only | yes | M |
| Transpile to C#/Python | partial | yes | yes | no | yes, the whole script | **no**, close at best | L |
| Generated definition, built inside GH | partial | yes | yes | no | yes, native parts | **no**, close at best | L |
| Write `.ghx` from outside | fragile | fragile | fragile | no | yes | no | L, risky |
| GH2 | n/a | beta | beta | n/a | n/a | n/a | do not target |

Rhino 7 column: [inference] from the facts that GhPython is IronPython 2 and that the new Script
component and `# r:` are Rhino 8 features; not tested.

---

## 10. What I am least sure of

1. **Whether a Python 3 script with a pip-installed Rust wheel works cleanly on Rhino 8 for Mac**
   (and which CPython version Rhino embeds). It carries the recommendation for stage 2.
2. **GH_IO.dll licensing.** McNeel staff say it is not redistributable (2023); the 2008 wiki says
   GPL; nothing is settled. Matters only if EasyCAD ever reads `.gh` binary itself.
3. **Per-vertex bead data in a `.3dm`.** The API offers a point-cloud value per point and user strings
   per object, but I did not find how Rhino/Grasshopper expose point-cloud values or how large a
   user string can be, so the sidecar file is my fallback, not a tested design.
