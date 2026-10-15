// The in-app agent: Claude builds and edits the recipe through a few tools, sees the result of
// every run (and pictures of the model when it asks), and answers in the chat.
//
// The model only ever changes the project through the recipe text, like everything else in the
// app (the text stays the single source of truth). Tools run here in the browser; the only
// thing sent out is the conversation, to the Claude API (directly with a key on this device, or
// through the password-protected proxy in agent-proxy/).
//
// Cost: the system prompt (language guide + step catalog) is identical on every request and
// cached, so after the first message it costs a tenth of the input price; the conversation is
// cached as it grows. Each run's numbers come back as compact text, and pictures only when the
// model asks for them.

import { LANGUAGE_GUIDE } from './language-guide.js';

/** Prices in dollars per million tokens (Claude API, Oct 2026). Cache writes are 5-minute ones. */
export const MODELS = {
  'claude-sonnet-5-5': { name: 'Sonnet 5.5', input: 2, output: 10, cacheRead: 0.2, cacheWrite: 2.5, fallback: true },
  'claude-opus-5-5': { name: 'Opus 5.5', input: 4, output: 20, cacheRead: 0.2, cacheWrite: 5, fallback: true },
  'claude-haiku-5-5': { name: 'Haiku 5.5', input: 0.1, output: 0.5, cacheRead: 0.01, cacheWrite: 0.125, fallback: false },
};
export const DEFAULT_MODEL = 'claude-sonnet-5-5';
export const EFFORTS = ['low', 'medium', 'high'];

const MAX_TOKENS = 32000;
/** Model requests per message before the agent stops and reports back. */
const MAX_ROUNDS = 16;
/** Marks the project snapshot the app attaches to each message (hidden in the chat). */
export const CONTEXT_TAG = '<project>';

/** What a set of usage numbers cost, in dollars. */
export function costOf(model, usage) {
  const m = MODELS[model] || MODELS[DEFAULT_MODEL];
  const u = usage || {};
  return (
    ((u.input_tokens || 0) * m.input +
      (u.cache_creation_input_tokens || 0) * m.cacheWrite +
      (u.cache_read_input_tokens || 0) * m.cacheRead +
      (u.output_tokens || 0) * m.output) /
    1e6
  );
}

/**
 * The system prompt. It must not change between requests (prompt caching is a prefix match),
 * so it holds only stable things: how to work, the language, the step catalog and the user's
 * own standing rules. Everything about the current project goes into the messages.
 * @param {string} catalog the engine's step catalog
 * @param {string} rules the user's standing rules ("always use volumetric flow", …)
 */
export function systemPrompt(catalog, rules = '') {
  const own = rules.trim() ? `\n\nThe user's standing rules (follow them unless they ask otherwise):\n${rules.trim()}` : '';
  return `You are the design assistant inside EasyCAD, a parametric design app for 3D printing with custom toolpaths. You work like an expert Grasshopper user who writes definitions as text: you build and change the project's recipe, a small dataflow program in the EasyCAD language. The app runs every change at once and shows the shape, the toolpath, a settings window generated from the recipe, and print estimates. The user is a designer who knows Grasshopper well and wants to keep design authority: they tune the settings themselves, so your job is a clear, well-structured recipe with the right settings, not a finished object frozen in place.

How you work
- The recipe is the project. Change it only with your tools: edit_recipe for focused changes (copy the old text exactly), write_recipe for a new project or a rewrite. Each change runs right away and you get back its problems, its steps and what they made, its settings, its size and its print numbers.
- Check your work. Read the run result after every change and fix problems before you answer. When the shape or its position in space matters (new projects, transforms, anything 3D), use look to see it from four sides; numbers alone can hide a wrong axis or sign.
- Make every number the user may want to tune a param with a sensible range, step and short label, grouped (for example "Shape" and "Print"). Mark guesses @assumed. Derive dependent numbers in steps instead of asking for them twice.
- Keep the project's memory in note lines at the top: what it is, the key decisions and constraints you learned from the user. Keep them short and current (edit them rather than piling up).
- Write for a Grasshopper user: one clear step per line, names that say what they hold, pipes for chains, a short comment above anything non-obvious. Prefer lists and replication over repeating lines.
- Use only the steps in the catalog below. If something can't be expressed with them, say so plainly, build the closest correct version, and name the step that is missing. Never add a setting that does nothing or fake a feature.
- Before a bigger change, say in a line what you're about to do. Answer briefly and plainly: what you changed, what to try in the settings, and anything you couldn't do. The user may write in any language; answer in theirs.
- Settings the user moved are deliberate: keep their values unless asked to change them.

${LANGUAGE_GUIDE}

The steps you can use (generated from the engine; inputs with = have defaults):

${catalog.trim()}${own}`;
}

/** The tools the model can call. Sorted by name, and identical on every request (caching). */
export const TOOLS = [
  {
    name: 'edit_recipe',
    description:
      'Changes the recipe by replacing exact text. Each edit replaces `old` with `new`; `old` must be copied exactly from the current recipe (spaces included) and appear in it exactly once. Edits apply in order, all or none. The recipe runs right after and you get the result back. Use it for focused changes.',
    input_schema: {
      type: 'object',
      properties: {
        edits: {
          type: 'array',
          minItems: 1,
          items: {
            type: 'object',
            properties: { old: { type: 'string' }, new: { type: 'string' } },
            required: ['old', 'new'],
            additionalProperties: false,
          },
        },
      },
      required: ['edits'],
      additionalProperties: false,
    },
    eager_input_streaming: true,
  },
  {
    name: 'look',
    description:
      'Renders the 3D view into one picture with four views: perspective, front (looking along +Y), right side (looking along -X) and top. Use it to check shapes and toolpaths in space. The grid is the printer bed.',
    input_schema: {
      type: 'object',
      properties: { show: { type: 'string', enum: ['shape', 'toolpath', 'both'], description: 'What to draw (default both).' } },
      additionalProperties: false,
    },
  },
  {
    name: 'read_recipe',
    description: 'Returns the current recipe with line numbers. The user can change it between messages, so read it when unsure.',
    input_schema: { type: 'object', properties: {}, additionalProperties: false },
  },
  {
    name: 'set_settings',
    description:
      "Moves settings (params) to new values, each in the setting's own unit as shown in the settings window (18 for a setting of 18 %, 90 for 90°). Values outside a setting's range are clamped. The recipe runs right after and you get the result back.",
    input_schema: {
      type: 'object',
      properties: {
        values: { type: 'object', description: 'Setting name → new value.', additionalProperties: { type: ['number', 'boolean', 'string'] } },
      },
      required: ['values'],
      additionalProperties: false,
    },
  },
  {
    name: 'write_recipe',
    description:
      'Replaces the whole recipe: for a new project or a rewrite. It must start with `easycad 0.1`. The recipe runs right after and you get the result back.',
    input_schema: {
      type: 'object',
      properties: { recipe: { type: 'string', description: 'The complete recipe text.' } },
      required: ['recipe'],
      additionalProperties: false,
    },
    eager_input_streaming: true,
  },
];

/**
 * The project as the model should see it at the start of a message: the recipe with line
 * numbers, the last run, and the printer.
 * @param {{ source: string, last: any, printerName: string, profile: any }} project
 */
export function projectContext(project) {
  const p = project.profile;
  const printer = p
    ? `${project.printerName}: ${p.mode} printer, bed ${p.bed[0]} × ${p.bed[1]} mm, build height ${p.bed[2]} mm, the recipe's origin sits at X${p.center[0]} Y${p.center[1]} on the bed`
    : project.printerName;
  return `${CONTEXT_TAG}
Printer: ${printer}.
The recipe now:
${numbered(project.source)}

Its last run:
${project.last ? runStatus(project.last) : 'not run yet'}
</project>`;
}

/** The recipe with line numbers (as the problems refer to them). */
export function numbered(source) {
  const lines = source.split('\n');
  const width = String(lines.length).length;
  return lines.map((l, i) => `${String(i + 1).padStart(width)}| ${l}`).join('\n');
}

const fmtNum = (n, digits = 1) => (Number.isFinite(n) ? Number(n.toFixed(digits)).toString() : '?');

/**
 * A run's result as compact text for the model: problems, steps, settings, size, print.
 * @param {any} r the engine's RunResult
 */
export function runStatus(r) {
  const out = [];
  const problems = r.diagnostics || [];
  out.push(problems.length ? `${problems.length} problem${problems.length === 1 ? '' : 's'}:` : 'No problems.');
  for (const d of problems) out.push(`  line ${d.line}:${d.col} ${d.severity}: ${d.message}`);
  if (r.steps?.length) {
    out.push('Steps:');
    for (const s of r.steps) out.push(`  ${s.name} (line ${s.line}, ${s.status}): ${s.summary}`);
  }
  if (r.params?.length) {
    const units = { deg: '°', '%': ' %' };
    const show = (p) => {
      const unit = p.unit ? units[p.unit] ?? ` ${p.unit}` : '';
      const range = p.min != null && p.max != null ? ` (${p.min}..${p.max}${p.step ? ` step ${p.step}` : ''})` : '';
      return `${p.name} = ${typeof p.value === 'string' ? JSON.stringify(p.value) : p.value}${unit}${range}`;
    };
    out.push(`Settings: ${r.params.map(show).join('; ')}`);
  }
  if (r.bounds) {
    const [a, b] = [r.bounds.min, r.bounds.max];
    out.push(`Drawn size: x ${fmtNum(a[0])}..${fmtNum(b[0])}, y ${fmtNum(a[1])}..${fmtNum(b[1])}, z ${fmtNum(a[2])}..${fmtNum(b[2])} mm`);
  }
  const st = r.print?.stats;
  if (st) {
    const minutes = Math.round(st.timeS / 60);
    out.push(
      `Prints: ${st.layers} layer${st.layers === 1 ? '' : 's'}, ${fmtNum(st.printedHeightMm)} mm tall, ` +
        `${fmtNum(st.pathLengthMm / 1000, 2)} m of extrusion, ${fmtNum(st.volumeMm3 / 1000, 2)} cm³ (${fmtNum(st.massGPla, 0)} g PLA), ` +
        `about ${minutes} min, up to ${fmtNum(st.flowMm3S)} mm³/s, line ${fmtNum(st.widthMm, 2)} mm, layer ${fmtNum(st.layerMm, 2)} mm` +
        (st.samplesPerTurn > 0 && st.layers > 1 ? `, steepest overhang ${fmtNum(st.maxOverhangDeg)}°` : ''),
    );
  } else {
    out.push('Prints: nothing (no `print` line).');
  }
  if (r.exports?.length) out.push(`Files: ${r.exports.map((x) => x.file).join(', ')}`);
  return out.join('\n');
}

/** Applies exact-text edits (all or none), or explains why not. */
export function applyEdits(source, edits) {
  let text = source;
  for (const [i, e] of edits.entries()) {
    const count = text.split(e.old).length - 1;
    if (!e.old || count !== 1) {
      const why = !e.old ? 'is empty' : count === 0 ? 'was not found' : `appears ${count} times`;
      return { error: `Edit ${i + 1}: the old text ${why}. Copy it exactly from the recipe (read_recipe shows the current text) and include enough of it to be unique. Nothing was changed.` };
    }
    text = text.replace(e.old, () => e.new);
  }
  return { text };
}

/** Checks a tool's input (inputs stream in unvalidated, so the app checks them itself). */
export function validate(name, input) {
  const isObj = (v) => v && typeof v === 'object' && !Array.isArray(v);
  if (!isObj(input)) return 'the input must be an object';
  if (name === 'edit_recipe') {
    if (!Array.isArray(input.edits) || !input.edits.length) return '`edits` must be a non-empty list';
    if (!input.edits.every((e) => isObj(e) && typeof e.old === 'string' && typeof e.new === 'string')) return 'each edit needs `old` and `new` text';
  } else if (name === 'write_recipe') {
    if (typeof input.recipe !== 'string' || !input.recipe.trim()) return '`recipe` must be the recipe text';
  } else if (name === 'set_settings') {
    if (!isObj(input.values) || !Object.keys(input.values).length) return '`values` must map setting names to values';
  } else if (name === 'look') {
    if (input.show != null && !['shape', 'toolpath', 'both'].includes(input.show)) return '`show` must be shape, toolpath or both';
  } else if (name !== 'read_recipe') {
    return `there is no tool called ${name}`;
  }
  return null;
}

/**
 * One user message to the agent, with as many tool rounds as it needs. Appends to `history`
 * (append-only: earlier turns are never edited, which keeps the cache and Claude's own
 * reasoning valid).
 * @param {object} o
 * @param {any} o.client an Anthropic SDK client
 * @param {string} o.model
 * @param {string} o.effort
 * @param {string} o.system
 * @param {any[]} o.history
 * @param {any[] | null} o.userContent the new user message (null to retry the last one)
 * @param {(name: string, input: any) => Promise<{ content: any, isError?: boolean }>} o.execute
 * @param {(event: any) => void} o.onEvent
 * @param {AbortSignal} [o.signal]
 */
export async function runTurn({ client, model, effort, system, history, userContent, execute, onEvent, signal }) {
  if (userContent) history.push({ role: 'user', content: userContent });
  for (let round = 0; round < MAX_ROUNDS; round++) {
    const params = {
      model,
      max_tokens: MAX_TOKENS,
      thinking: { type: 'adaptive', display: 'summarized' },
      output_config: { effort },
      // One breakpoint after the stable system prompt, plus automatic caching of the growing chat.
      system: [{ type: 'text', text: system, cache_control: { type: 'ephemeral' } }],
      cache_control: { type: 'ephemeral' },
      tools: TOOLS,
      messages: history,
    };
    if (MODELS[model]?.fallback) {
      // If a safety check declines a request, the API retries it on a fallback model.
      params.betas = ['server-side-fallback-2026-07-01'];
      params.fallbacks = 'default';
    }
    const stream = client.beta.messages.stream(params, { signal });
    stream.on('text', (delta) => onEvent({ type: 'text', delta }));
    stream.on('thinking', (delta) => onEvent({ type: 'thinking', delta }));
    stream.on('contentBlock', (block) => onEvent({ type: 'block', block }));
    stream.on('inputJson', (_delta, snapshot) => onEvent({ type: 'toolInput', snapshot }));
    const message = await stream.finalMessage();
    history.push({ role: 'assistant', content: message.content });
    onEvent({ type: 'message', message, cost: costOf(model, message.usage) });

    const uses = message.content.filter((b) => b.type === 'tool_use');
    if (message.stop_reason === 'refusal') {
      closeToolUses(history, 'Not run: the answer was declined.');
      onEvent({ type: 'refusal', details: message.stop_details });
      return;
    }
    if (message.stop_reason === 'pause_turn') continue;
    if (!uses.length) return;
    if (message.stop_reason === 'max_tokens') {
      closeToolUses(history, 'Not run: the tool input was cut off (too long). Make smaller edits.');
      continue;
    }
    const results = [];
    for (const use of uses) {
      const problem = validate(use.name, use.input);
      const result = problem ? { content: `Not run: ${problem}.`, isError: true } : await execute(use.name, use.input);
      onEvent({ type: 'toolResult', use, result });
      results.push({ type: 'tool_result', tool_use_id: use.id, content: result.content, ...(result.isError ? { is_error: true } : {}) });
    }
    history.push({ role: 'user', content: results });
    if (signal?.aborted) return;
  }
  onEvent({ type: 'limit', rounds: MAX_ROUNDS });
}

/**
 * Answers any tool calls left without results (after a stop or a cut-off answer), so the
 * conversation stays valid for the next request.
 */
export function closeToolUses(history, reason) {
  const last = history[history.length - 1];
  if (!last || last.role !== 'assistant' || !Array.isArray(last.content)) return;
  const uses = last.content.filter((b) => b.type === 'tool_use');
  if (!uses.length) return;
  history.push({
    role: 'user',
    content: uses.map((u) => ({ type: 'tool_result', tool_use_id: u.id, content: reason, is_error: true })),
  });
}
