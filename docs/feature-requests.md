# Feature requests: the app asks, Claude Code builds, the app carries on

*Since 17 Oct 2026.*

When the in-app agent can't build what you ask for (no combination of steps can do it), it
doesn't fake it. It asks for the missing step:

1. **The agent files a request.** Its `request_feature` tool writes a precise, general
   proposal: what you wanted, what's missing and why, the step it needs (name, inputs, output),
   and an example recipe. The app files it as an issue labelled `feature-request` in this repo,
   through your proxy. Without a proxy, it shows a button that files it on GitHub with one tap.
   The agent tells you what it asked for, and stops.
2. **Claude Code builds it.** The issue starts the **Feature builder** workflow
   (`.github/workflows/feature-builder.yml`). Claude Code reads the request and first decides:
   - **already possible**: it answers with a recipe made of existing steps, and builds nothing;
   - **not for an unattended build**: it explains why, and the request waits for you;
   - **otherwise** it builds the step the EasyCAD way: general, tested, documented, all checks
     passing.
3. **The workflow checks and ships it.** The workflow does this itself, not the AI:
   - it runs CI on the branch: every test, the golden G-code, the browser tests;
   - it checks the merge rules: no protected files, no fewer tests, not too big;
   - then it merges, deploys, and runs CI again on the live branch.

   If anything fails, it undoes the change and the builder tries again with the failure in
   hand: three attempts, then it asks for you.
4. **You get an email.** The workflow comments on the issue when it's done (or stuck), and
   GitHub emails you about it.
5. **The chat carries on by itself.** The app checks open requests every few minutes, and
   whenever you come back to it. When one is done, it reloads to get the new step (only when
   you're not using it; otherwise it shows a button). Then it sends the agent a message, so it
   builds what you asked for in the first place. A request answered with existing steps
   carries on without a reload.

The requests the app is waiting for are listed above the message box in the Chat tab.
**"Carry on by itself"** in AI settings switches the automatic continuation off; you then send
the next message yourself.

## Safety: why it can merge without you

You asked for no human in the loop, and mostly there isn't one. These rules make that
reasonable:

- **Only you can start it.** The builder runs only for issues opened and labelled by the
  repository owner: you, through your proxy's token or by hand. Anyone else's issue does
  nothing, even with the right label.
- **The AI doesn't merge its own work.** Claude Code only commits. The workflow pushes, runs
  CI, checks the rules (`.github/feature-builder/guard.sh`), merges and deploys.
- **CI is the judge**, as for every change:
  - all Rust tests;
  - the golden tests, which must match EasyGCode's G-code byte for byte;
  - the hand-checked fixtures;
  - the wasm size budget;
  - the browser tests on desktop and phone.
- **Some files only a person changes.** The merge is refused, and the request waits for you,
  if the change touches:
  - the workflows and these rules (`.github/`);
  - the AI proxy;
  - EasyGCode's fixtures and the script that makes them;
  - `CLAUDE.md`;
  - dependencies (`Cargo.toml`/`Cargo.lock`, `THIRD-PARTY-NOTICES.md`, `web/vendor/`), and
    `web/config.json`.

  So it can't add a library, weaken a test or change its own rules.
- **G-code drives real machines.** A change to the G-code writer (`crates/core/src/gcode.rs`)
  isn't merged by itself: it waits for your review (label `needs-review`). New G-code from a
  step must be checked against a hand-checked fixture (CLAUDE.md).
- **Tests only grow**, and a change over 3000 lines needs a person.
- **Broken means undone.** If CI fails after the merge, the change is reverted (the site goes
  back to the version before) and the builder tries again.
- **The request is data, not orders.** The builder treats the issue text as a description to
  judge, never as instructions that override CLAUDE.md.

What's left as a risk, plainly:
- Claude Code runs commands while it builds (it has to, to test).
- A request planted in a file you open could steer the in-app agent into asking for something
  odd. The builder would still only commit inside the rules above, and CI would still have to
  pass.
- Turn the builder off any time: disable the workflow in the Actions tab.

## Setup (once)

Steps 1 and 3 are required: without them nothing gets built. Steps 2 and 4 make it smoother.

1. **Let Claude Code run in Actions.** Pick one, as a repository secret (Settings → Secrets
   and variables → Actions):
   - `CLAUDE_CODE_OAUTH_TOKEN`: uses your Claude subscription. Make it on your computer with
     `claude setup-token`, then paste it into the secret (not into a chat);
   - `ANTHROPIC_API_KEY`: pay per use. The same key as the proxy's works.
2. **Install the Claude GitHub app** on the repo (https://github.com/apps/claude). The action
   uses it to act on GitHub. Without it, the workflow's own token still works.
3. **Allow workflows to write.** Settings → Actions → General → Workflow permissions:
   - "Read and write permissions";
   - and tick "Allow GitHub Actions to create and approve pull requests".

   The builder pushes its branch, opens the pull request, merges it, and labels and closes the
   issue.
4. **Let the proxy file requests** (optional; without it you file each request with one tap):
   - make a fine-grained personal access token at
     https://github.com/settings/personal-access-tokens:
     - repository access: only `easycad`;
     - permissions: Issues "Read and write", nothing else;
   - add it as the repository secret `FEATURE_REQUEST_TOKEN`;
   - the next deploy gives it to the proxy (as its `GITHUB_TOKEN` secret).

   The proxy itself must be set up already (docs/api-key.md).
5. **Email:** GitHub's notifications for issues you're part of, on by default (Settings →
   Notifications → "Participating": email).

Optional:
- the repository variable `BUILDER_MODEL` picks the model the builder uses (it defaults to the
  action's);
- you can run the builder by hand for any request: Actions → Feature builder → Run workflow,
  with the issue number. It also retries a stuck one.

## Costs

- **Building a step** is a long Claude Code session: reading the code, writing the step and
  tests, running the checks, fixing what fails.
  - Expect from about one to a few dollars of API use per request; more for hard ones and for
    retries.
  - With `CLAUDE_CODE_OAUTH_TOKEN` it counts against your subscription's usage instead.
  - The workflow caps a run at 300 turns and two and a half hours, and a request at three
    attempts.
- **Answering** ("existing steps can do it") is much cheaper.
- **GitHub Actions** minutes are free for public repositories.
- **Filing and checking** costs nothing: the app checks the public issue, no token needed.

## Which model for the in-app agent?

The plan was that good steps would let a small, cheap model (Haiku) do the in-app work. That's
partly right:

- **Haiku is fine for tuning and small edits:** moving settings, renaming, changing a number,
  explaining a recipe. Steps with clear names and catalog entries make that reliable.
- **Sonnet (the default) is better for new projects and for deciding what's missing.** Telling
  "this truly can't be done" from "this takes three steps combined with a field" is the
  hardest judgement in the loop. A wrong "can't" costs a builder run and an hour's wait; a
  wrong "can" fakes a feature.
- **The builder is the safety net:** a request that existing steps can satisfy comes back as an
  answer with a recipe, not as new code, and the chat carries on with it.

So: Sonnet for building, Haiku for tuning. Switch in AI settings whenever you like; the builder
uses its own model.

## For the builder (Claude Code in Actions)

The builder's instructions are `.github/feature-builder/PROMPT.md`, and the merge rules are
`.github/feature-builder/guard.sh`. Both are under `.github/`, so the builder can't change
them. It writes its decision to `.builder/` (never committed):
- `answer.md`: existing steps can do it;
- `stuck.md`: not for an unattended build, and why;
- `summary.md`: what it built, for the owner. Its last line is `steps: a, b`.

Labels the workflow uses on requests: `feature-request` (starts it), `building`, `shipped`,
`answered`, `needs-human`, `needs-review`.

Each comment the workflow writes ends in a marker, `<!-- easycad-builder … -->`. The app reads
the last one:
- `attempt N`;
- `answered`;
- `stuck`;
- `shipped steps: a, b`: the app reloads until its engine has those steps, then carries on.
