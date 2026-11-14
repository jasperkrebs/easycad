# Working together: the owner and two agents

Since 14 Nov 2026 EasyCAD is built by the owner with two Claude Code agents, each on its own
long-lived branch, each with its own kind of work. Both merge into `main` through pull requests
that the owner merges; `main` is the live app (every push deploys to GitHub Pages).

| Branch | Agent | Works on |
|---|---|---|
| `general-manager` | the general manager | the app and its architecture: everything general |
| `recipe-developer` | the recipe developer | recipes and projects: templates, the owner's project files, print tests |

The point of the split: each agent's context stays on its own kind of work. Ask the recipe
developer about a lampshade's settings, a shade, a test print; ask the general manager about the
app, the engine, the language, the build, how the pieces fit.

## Who owns what

| Area | Files | Owner |
|---|---|---|
| Templates and project recipes | `web/templates/*.easycad`, `fixtures/recipes/`, the owner's `.easycad` files | recipe developer |
| Recipe tests | `crates/core/src/lamp_tests.rs`, the template tests in `blocks_tests.rs` and `toolbox_tests.rs` | recipe developer |
| Recipe docs | `docs/lamp-builder.md` (the shade and its settings), `docs/easygcode-coverage.md`, `docs/creating-shapes.md` | recipe developer |
| Tuning a print (overhangs, flows, layer heights) and test prints | settings, notes in the devlog | recipe developer |
| The engine | `crates/` (steps, `vase_revolve`, toolpaths, G-code, STEP/STL, the checks) | general manager |
| The language | `crates/lang/`, `docs/definition-language.md`, `web/js/language-guide.js` | general manager |
| The app | `web/js/`, `web/index.html`, `web/styles.css`, `web/vendor/` | general manager |
| G-code safety | `crates/core/src/gcode.rs`, golden tests, `fixtures/easygcode/`, `fixtures/hand-checked/` | general manager |
| Build, CI, proxy, feature requests | `scripts/`, `.github/`, `agent-proxy/`, `docs/feature-requests.md` | general manager |
| Architecture and plans | `CLAUDE.md`, `docs/architecture.md`, `docs/roadmap.md`, this file | general manager |
| Shared | `README.md`, `docs/devlog.md` | both (see below) |

Owning an area means: that agent changes it, keeps its docs right, and answers for it. The other
agent can read everything; if it needs a change in the other's area it asks (below) instead of
making it, unless the owner says otherwise. Golden tests are never changed to make a recipe fit.

## When a recipe needs something the app can't do

General steps, not per-project code (`CLAUDE.md`): first try every combination of existing steps.
If none does it, the recipe developer writes down what's needed (what the step takes, what it
gives, the project that needs it, a small example) in its pull request and in the dev log under
**Needs from the app**; the owner hands it to the general manager, who builds it as a general,
documented, tested step. The recipe developer then uses it in the recipe.

The other way round: when the general manager changes the engine or the language in a way that
touches recipes (a renamed setting, a step that behaves differently, a new default), it updates
the templates too, so `main` never has a template that's broken, and says so in its pull request
so the recipe developer knows.

## Branches and `main`: always start from the newest

`main` moves while you work: the other agent's pull requests land there. So:

1. **Before every task:** `git fetch origin` and merge `origin/main` into your branch
   (`git merge origin/main`). Never rebase a pushed branch and never force-push.
2. **Before opening a pull request:** merge `origin/main` again, run the checks (`CLAUDE.md`,
   "Build and run"), push.
3. **After your pull request is merged:** keep the same branch; merge `origin/main` into it at
   the start of the next task (step 1), which brings the merge commit in.
4. **Conflicts:** keep both sides' changes. In a file the other agent owns, take theirs and
   re-apply only what your change needs. Run the tests after resolving.
5. **Small pull requests**, one topic each, so the other agent's merges rarely conflict.

The owner merges pull requests, one at a time; CI runs on each.

## The dev log

Both write `docs/devlog.md`, newest first. Start each entry's heading with the date and end it
with the branch: `## 2026-11-14: the brim's own extrusion (general-manager)`. On a conflict there,
keep both entries, the newer first.

## Feature requests

The app's own feature requests (`docs/feature-requests.md`) are built by an unattended Claude
Code run on its own branch, not by either agent; its pull requests follow the same rules.
