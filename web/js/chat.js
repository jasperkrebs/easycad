// The Chat tab: talk to the agent (web/js/agent.js) about the open project.
//
// Two ways to reach Claude, chosen in "AI settings":
// - Password: the app calls EasyCAD's own proxy (agent-proxy/), which holds the API key as a
//   secret; the app only knows a password. Works on every device with just the password.
// - API key on this device: the app calls the Claude API directly with a key stored only in this
//   browser.
// Either way, nothing secret is in the website or the repo.
//
// The chat is kept on this device (IndexedDB) with the project it's about; "New chat" starts
// over. The project's own memory is its recipe and note lines, which travel with the file.

import { CONTEXT_TAG, DEFAULT_MODEL, EFFORTS, MODELS, applyEdits, closeToolUses, numbered, projectContext, runStatus, runTurn, systemPrompt } from './agent.js';
import { $, el } from './dom.js';

const SETTINGS_KEY = 'easycad.ai';
const SECRET_KEY = 'easycad.ai.secret';
const SPEND_KEY = 'easycad.ai.spend';
const DB_NAME = 'easycad';
const DB_STORE = 'chat';

/**
 * @typedef {{ mode: 'proxy' | 'key', proxyUrl: string, model: string, effort: string, rules: string }} AiSettings
 * @typedef {{ source: string, last: any, printerName: string, profile: any, values: Record<string, any>, editorDirty: boolean }} Project
 */

/**
 * @param {object} app
 * @param {() => Project} app.project the open project
 * @param {(source: string) => Promise<any>} app.applyRecipe replaces the recipe, runs it, returns the result
 * @param {(values: Record<string, any>) => Promise<any>} app.applyValues moves settings, runs, returns the result
 * @param {(show: string) => string} app.look renders the four views, returns a JPEG data URL
 * @param {() => Promise<string>} app.catalog the engine's step catalog
 */
export function createChat(app) {
  /** @type {any[]} */
  let history = [];
  /** @type {Record<number, number>} cost in dollars of the assistant message at each index */
  let costs = {};
  let busy = false;
  /** @type {AbortController | null} */
  let abort = null;
  /** Recipe before the last turn changed it, for Undo. */
  let undoSource = null;
  /** A note for the model at the start of the next message (e.g. "I undid your change"). */
  let pendingNote = '';
  let defaultProxyUrl = '';
  /** @type {Promise<any> | null} */
  let sdk = null;

  const log = $('chatLog');
  const input = /** @type {HTMLTextAreaElement} */ ($('chatText'));

  // ---------------------------------------------------------------------------------------------
  // Settings and secrets (this device only)
  // ---------------------------------------------------------------------------------------------

  /** @returns {AiSettings} */
  function settings() {
    let s = {};
    try {
      s = JSON.parse(localStorage.getItem(SETTINGS_KEY) || '{}') || {};
    } catch {
      s = {};
    }
    return {
      mode: s.mode === 'key' ? 'key' : 'proxy',
      proxyUrl: typeof s.proxyUrl === 'string' && s.proxyUrl ? s.proxyUrl : defaultProxyUrl,
      model: s.model in MODELS ? s.model : DEFAULT_MODEL,
      effort: EFFORTS.includes(s.effort) ? s.effort : 'medium',
      rules: typeof s.rules === 'string' ? s.rules : '',
    };
  }

  function saveSettings(patch) {
    try {
      localStorage.setItem(SETTINGS_KEY, JSON.stringify({ ...settings(), ...patch }));
    } catch {
      // Private browsing: the settings last until the page closes.
    }
    renderStatus();
  }

  const secret = () => {
    try {
      return localStorage.getItem(SECRET_KEY) || '';
    } catch {
      return '';
    }
  };

  function setSecret(value) {
    try {
      if (value) localStorage.setItem(SECRET_KEY, value);
      else localStorage.removeItem(SECRET_KEY);
    } catch {
      // ignore
    }
    showSecretState();
    renderStatus();
  }

  function showSecretState() {
    const s = settings();
    $('aiSecretState').textContent = secret() ? (s.mode === 'proxy' ? 'Password saved on this device.' : 'Key saved on this device.') : 'Nothing saved yet.';
  }

  function ready() {
    const s = settings();
    return Boolean(secret()) && (s.mode === 'key' || Boolean(s.proxyUrl));
  }

  async function client() {
    sdk ||= import('../vendor/anthropic/anthropic-sdk.min.mjs');
    const { default: Anthropic } = await sdk;
    const s = settings();
    if (s.mode === 'proxy') {
      // The proxy reads the password as a bearer token and adds the real key itself.
      return new Anthropic({ baseURL: s.proxyUrl.replace(/\/+$/, ''), apiKey: null, authToken: secret(), dangerouslyAllowBrowser: true, maxRetries: 2 });
    }
    return new Anthropic({ apiKey: secret(), dangerouslyAllowBrowser: true, maxRetries: 2 });
  }

  // ---------------------------------------------------------------------------------------------
  // Spending (shown in the chat; the Claude Console has the exact numbers)
  // ---------------------------------------------------------------------------------------------

  const month = () => new Date().toISOString().slice(0, 7);

  function monthSpend() {
    try {
      const s = JSON.parse(localStorage.getItem(SPEND_KEY) || 'null');
      return s && s.month === month() ? Number(s.usd) || 0 : 0;
    } catch {
      return 0;
    }
  }

  function addSpend(usd) {
    try {
      localStorage.setItem(SPEND_KEY, JSON.stringify({ month: month(), usd: monthSpend() + usd }));
    } catch {
      // ignore
    }
  }

  const money = (usd) => (usd < 0.01 ? `$${usd.toFixed(4)}` : `$${usd.toFixed(usd < 1 ? 3 : 2)}`);

  function renderStatus() {
    const s = settings();
    const chatCost = Object.values(costs).reduce((a, b) => a + b, 0);
    $('chatCost').textContent = `${MODELS[s.model].name} · ${s.effort} · this chat ${money(chatCost)} · this month ${money(monthSpend())}`;
    $('chatSetup').hidden = ready();
    $('aiProxyFields').hidden = s.mode !== 'proxy';
    $('aiKeyFields').hidden = s.mode !== 'key';
  }

  // ---------------------------------------------------------------------------------------------
  // The chat log
  // ---------------------------------------------------------------------------------------------

  function scrollDown() {
    log.scrollTop = log.scrollHeight;
  }

  /** Plain text with a little Markdown (paragraphs, lists, `code`, **bold**, code blocks), built as DOM: no innerHTML. */
  function richText(text) {
    const box = el('div', { className: 'md' });
    const parts = text.split(/```[^\n]*\n?/);
    parts.forEach((part, i) => {
      if (i % 2 === 1) {
        box.append(el('pre', {}, [el('code', { text: part.replace(/\n$/, '') })]));
        return;
      }
      for (const para of part.split(/\n{2,}/)) {
        const lines = para.split('\n').filter((l) => l.trim());
        if (!lines.length) continue;
        if (lines.every((l) => /^\s*([-*]|\d+\.)\s/.test(l))) {
          box.append(el('ul', {}, lines.map((l) => el('li', {}, inline(l.replace(/^\s*([-*]|\d+\.)\s/, ''))))));
        } else {
          const p = el('p');
          lines.forEach((l, k) => {
            if (k) p.append(el('br'));
            p.append(...inline(l));
          });
          box.append(p);
        }
      }
    });
    return box;
  }

  function inline(text) {
    return text.split(/(`[^`]+`|\*\*[^*]+\*\*)/).filter(Boolean).map((t) => {
      if (t.startsWith('`') && t.endsWith('`')) return el('code', { text: t.slice(1, -1) });
      if (t.startsWith('**') && t.endsWith('**')) return el('b', { text: t.slice(2, -2) });
      return document.createTextNode(t);
    });
  }

  /** A one-line description of a tool call, for the chat. */
  function describeTool(use) {
    const i = use.input || {};
    switch (use.name) {
      case 'edit_recipe':
        return `Edited the recipe (${(i.edits || []).length} change${(i.edits || []).length === 1 ? '' : 's'})`;
      case 'write_recipe':
        return `Wrote the recipe (${String(i.recipe || '').split('\n').length} lines)`;
      case 'set_settings':
        return `Set ${Object.entries(i.values || {}).map(([k, v]) => `${k} = ${v}`).join(', ')}`;
      case 'look':
        return `Looked at the ${i.show === 'toolpath' ? 'toolpath' : i.show === 'shape' ? 'shape' : 'model'}`;
      case 'read_recipe':
        return 'Read the recipe';
      default:
        return use.name;
    }
  }

  function bubble(role, children) {
    const b = el('div', { className: `msg ${role}` }, children);
    log.append(b);
    scrollDown();
    return b;
  }

  /** Rebuilds the whole log from the saved conversation. */
  function renderHistory() {
    log.replaceChildren();
    /** @type {Map<string, HTMLElement>} */
    const tools = new Map();
    // One bubble per answer: the model's rounds between two user messages, with their total cost.
    /** @type {HTMLElement | null} */
    let answer = null;
    let answerCost = 0;
    const finish = () => {
      if (answer && answerCost > 0) answer.append(el('div', { className: 'cost', text: money(answerCost) }));
      answer = null;
      answerCost = 0;
    };
    history.forEach((m, index) => {
      if (m.role === 'user') {
        const blocks = Array.isArray(m.content) ? m.content : [{ type: 'text', text: String(m.content) }];
        const words = blocks.filter((b) => b.type === 'text' && !b.text.startsWith(CONTEXT_TAG) && !b.text.startsWith('(Note from the app')).map((b) => b.text);
        if (words.length) {
          finish();
          bubble('user', [richText(words.join('\n\n'))]);
        }
        for (const b of blocks) {
          if (b.type === 'tool_result') {
            const chip = tools.get(b.tool_use_id);
            if (chip) chip.classList.add(b.is_error ? 'bad' : 'ok');
          }
        }
        return;
      }
      answer ||= bubble('assistant', []);
      for (const b of m.content || []) {
        if (b.type === 'text' && b.text.trim()) answer.append(richText(b.text));
        else if (b.type === 'thinking' && b.thinking?.trim()) answer.append(thinkingBox(b.thinking));
        else if (b.type === 'tool_use') {
          const chip = el('div', { className: 'tool', text: describeTool(b) });
          tools.set(b.id, chip);
          answer.append(chip);
        } else if (b.type === 'fallback') {
          answer.append(el('div', { className: 'tool', text: `Continued on ${b.to?.model || 'another model'}` }));
        }
      }
      answerCost += costs[index] || 0;
    });
    finish();
    scrollDown();
    if (!history.length) {
      log.append(
        el('p', {
          className: 'hint empty',
          text: 'Describe a new project or a change to this one, e.g. “a cylinder whose top is cut at an angle, with the layers tilting gradually, printed as one spiral”.',
        }),
      );
    }
    renderStatus();
  }

  function thinkingBox(text) {
    const d = el('details', { className: 'thinking' }, [el('summary', { text: 'Thinking' }), el('p', { text })]);
    return d;
  }

  // ---------------------------------------------------------------------------------------------
  // Persistence (IndexedDB: the conversation can hold pictures, too big for localStorage)
  // ---------------------------------------------------------------------------------------------

  function db() {
    return new Promise((resolve, reject) => {
      const req = indexedDB.open(DB_NAME, 1);
      req.onupgradeneeded = () => req.result.createObjectStore(DB_STORE);
      req.onsuccess = () => resolve(req.result);
      req.onerror = () => reject(req.error);
    });
  }

  async function save() {
    try {
      const d = await db();
      await new Promise((resolve, reject) => {
        const tx = d.transaction(DB_STORE, 'readwrite');
        tx.objectStore(DB_STORE).put({ history, costs }, 'current');
        tx.oncomplete = resolve;
        tx.onerror = () => reject(tx.error);
      });
      d.close();
    } catch {
      // No storage (private browsing): the chat lasts until the page closes.
    }
  }

  async function load() {
    try {
      const d = await db();
      const saved = await new Promise((resolve, reject) => {
        const req = d.transaction(DB_STORE).objectStore(DB_STORE).get('current');
        req.onsuccess = () => resolve(req.result);
        req.onerror = () => reject(req.error);
      });
      d.close();
      if (saved && Array.isArray(saved.history)) {
        history = saved.history;
        costs = saved.costs || {};
      }
    } catch {
      // Start with an empty chat.
    }
  }

  // ---------------------------------------------------------------------------------------------
  // Tools: how the agent's calls change the project
  // ---------------------------------------------------------------------------------------------

  /** @param {any} result a RunResult */
  const ranText = (result) => (result ? runStatus(result) : 'The recipe could not be run (the engine stopped).');

  /**
   * @param {string} name
   * @param {any} input
   * @returns {Promise<{ content: any, isError?: boolean }>}
   */
  async function execute(name, input) {
    const project = app.project();
    if (name === 'read_recipe') return { content: numbered(project.source) };
    if (name === 'write_recipe') {
      if (input.recipe.length > 256 * 1024) return { content: 'Not run: the recipe is too long (256 kB at most).', isError: true };
      const result = await app.applyRecipe(input.recipe);
      return { content: ranText(result), isError: hasErrors(result) };
    }
    if (name === 'edit_recipe') {
      const edited = applyEdits(project.source, input.edits);
      if (edited.error) return { content: edited.error, isError: true };
      const result = await app.applyRecipe(edited.text);
      return { content: ranText(result), isError: hasErrors(result) };
    }
    if (name === 'set_settings') {
      const params = new Map((project.last?.params || []).map((p) => [p.name, p]));
      const values = {};
      const notes = [];
      for (const [key, raw] of Object.entries(input.values)) {
        const p = params.get(key);
        if (!p) {
          notes.push(`\`${key}\` is not a setting (settings: ${[...params.keys()].join(', ') || 'none'})`);
          continue;
        }
        let v = raw;
        if (p.kind === 'number') {
          v = Number(raw);
          if (!Number.isFinite(v)) {
            notes.push(`\`${key}\` needs a number`);
            continue;
          }
          const clamped = Math.min(p.max ?? v, Math.max(p.min ?? v, v));
          if (clamped !== v) notes.push(`\`${key}\` clamped to ${clamped} (range ${p.min}..${p.max})`);
          v = clamped;
        } else if (p.kind === 'toggle') v = raw === true || raw === 'true';
        else if (p.kind === 'choice') {
          const choice = p.choices.find((c) => String(c) === String(raw));
          if (choice === undefined) {
            notes.push(`\`${key}\` must be one of ${p.choices.join(', ')}`);
            continue;
          }
          v = choice;
        } else v = String(raw);
        values[key] = v;
      }
      if (!Object.keys(values).length) return { content: `Nothing changed: ${notes.join('; ')}.`, isError: true };
      const result = await app.applyValues(values);
      return { content: (notes.length ? `Note: ${notes.join('; ')}.\n` : '') + ranText(result), isError: hasErrors(result) };
    }
    if (name === 'look') {
      const url = app.look(input.show || 'both');
      const data = url.slice(url.indexOf(',') + 1);
      return {
        content: [
          { type: 'image', source: { type: 'base64', media_type: 'image/jpeg', data } },
          { type: 'text', text: 'Top left: perspective. Top right: front (looking along +Y). Bottom left: right side (looking along -X). Bottom right: top (looking down, +Y up). The grid is the printer bed.' },
        ],
      };
    }
    return { content: `There is no tool called ${name}.`, isError: true };
  }

  const hasErrors = (result) => !result || result.diagnostics.some((d) => d.severity === 'error');

  // ---------------------------------------------------------------------------------------------
  // Sending
  // ---------------------------------------------------------------------------------------------

  function setBusy(on) {
    busy = on;
    $('chatSend').hidden = on;
    $('chatStop').hidden = !on;
    input.disabled = false;
  }

  function showError(text) {
    bubble('error', [el('p', { text })]);
  }

  /** What went wrong, for people. */
  function explain(err, Anthropic) {
    const s = settings();
    if (Anthropic && err instanceof Anthropic.APIUserAbortError) return '';
    if (Anthropic && err instanceof Anthropic.AuthenticationError) {
      return s.mode === 'proxy' ? 'The proxy refused the password. Check it in AI settings.' : 'The API key was refused. Check it in AI settings.';
    }
    if (Anthropic && err instanceof Anthropic.PermissionDeniedError) return `Not allowed: ${err.message}`;
    if (Anthropic && err instanceof Anthropic.RateLimitError) return 'Too many requests right now (rate limit). Wait a moment and send again.';
    if (Anthropic && err instanceof Anthropic.InternalServerError) return 'Claude is busy or had a problem. Try again in a moment.';
    if (Anthropic && err instanceof Anthropic.APIConnectionError) {
      return s.mode === 'proxy' ? `Can't reach the proxy at ${s.proxyUrl} (offline, or a wrong address?).` : "Can't reach the Claude API (offline?).";
    }
    if (Anthropic && err instanceof Anthropic.APIError) return `The API said: ${err.message}`;
    return String((err && err.message) || err);
  }

  async function send(text, { retry = false } = {}) {
    if (busy) return;
    if (!ready()) {
      $('aiSettings').open = true;
      showError(settings().mode === 'proxy' ? 'Enter the proxy password in AI settings first.' : 'Enter your API key in AI settings first.');
      return;
    }
    const project = app.project();
    if (project.editorDirty) {
      showError('The Recipe tab has edits that were not run yet. Run or revert them first, so the AI works on the current recipe.');
      return;
    }
    setBusy(true);
    abort = new AbortController();
    const before = project.source;
    let userContent = null;
    if (!retry) {
      const note = pendingNote ? [{ type: 'text', text: `(Note from the app: ${pendingNote})` }] : [];
      pendingNote = '';
      userContent = [{ type: 'text', text: projectContext(project) }, ...note, { type: 'text', text }];
      bubble('user', [richText(text)]);
    }
    const s = settings();
    /** @type {HTMLElement | null} the answer being streamed in */
    let live = null;
    let liveText = '';
    const ensureLive = () => (live ||= bubble('assistant streaming', [el('p', { className: 'status-line', text: 'Thinking…' })]));
    let Anthropic = null;
    try {
      const c = await client();
      Anthropic = (await sdk).default;
      const system = systemPrompt(await app.catalog(), s.rules);
      await runTurn({
        client: c,
        model: s.model,
        effort: s.effort,
        system,
        history,
        userContent,
        signal: abort.signal,
        execute,
        onEvent(e) {
          if (e.type === 'text') {
            ensureLive();
            liveText += e.delta;
            live.replaceChildren(richText(liveText));
            scrollDown();
          } else if (e.type === 'thinking') {
            ensureLive();
          } else if (e.type === 'toolInput') {
            const box = ensureLive();
            const recipe = e.snapshot && typeof e.snapshot.recipe === 'string' ? e.snapshot.recipe : null;
            const status = recipe !== null ? `Writing the recipe… ${recipe.split('\n').length} lines` : 'Working on the recipe…';
            let line = box.querySelector('.status-line');
            if (!line) box.append((line = el('p', { className: 'status-line' })));
            line.textContent = status;
            scrollDown();
          } else if (e.type === 'message') {
            costs[history.length - 1] = e.cost;
            addSpend(e.cost);
            live = null;
            liveText = '';
            renderHistory();
            save();
          } else if (e.type === 'toolResult') {
            renderHistory();
          } else if (e.type === 'refusal') {
            showError('Claude declined this request. Try rephrasing it.');
          } else if (e.type === 'limit') {
            showError(`Stopped after ${e.rounds} steps. Send "continue" to let it carry on.`);
          }
        },
      });
    } catch (err) {
      closeToolUses(history, 'Stopped before this ran.');
      const msg = explain(err, Anthropic);
      renderHistory();
      if (msg) showError(msg);
    } finally {
      abort = null;
      setBusy(false);
      save();
      if (app.project().source !== before) {
        undoSource = before;
        $('chatUndo').hidden = false;
      }
      renderStatus();
    }
  }

  // ---------------------------------------------------------------------------------------------
  // Wiring
  // ---------------------------------------------------------------------------------------------

  function wire() {
    $('chatForm').addEventListener('submit', (e) => {
      e.preventDefault();
      const text = input.value.trim();
      if (!text) return;
      input.value = '';
      send(text);
    });
    input.addEventListener('keydown', (e) => {
      if (e.key === 'Enter' && (e.ctrlKey || e.metaKey)) {
        e.preventDefault();
        $('chatForm').requestSubmit();
      }
    });
    $('chatStop').addEventListener('click', () => abort?.abort());
    $('chatNew').addEventListener('click', async () => {
      if (busy) return;
      history = [];
      costs = {};
      undoSource = null;
      $('chatUndo').hidden = true;
      await save();
      renderHistory();
    });
    $('chatUndoButton').addEventListener('click', async () => {
      if (undoSource === null) return;
      await app.applyRecipe(undoSource);
      undoSource = null;
      $('chatUndo').hidden = true;
      pendingNote = 'the user undid the recipe changes from your last turn; the recipe is back to how it was before them.';
    });
    $('chatKeep').addEventListener('click', () => {
      undoSource = null;
      $('chatUndo').hidden = true;
    });

    const s = settings();
    const modeInputs = /** @type {NodeListOf<HTMLInputElement>} */ (document.querySelectorAll('input[name="aiMode"]'));
    modeInputs.forEach((r) => {
      r.checked = r.value === s.mode;
      r.addEventListener('change', () => {
        if (r.checked) {
          setSecret('');
          saveSettings({ mode: r.value });
        }
      });
    });
    const proxyUrl = /** @type {HTMLInputElement} */ ($('aiProxyUrl'));
    proxyUrl.value = s.proxyUrl;
    proxyUrl.addEventListener('change', () => saveSettings({ proxyUrl: proxyUrl.value.trim() }));

    const model = /** @type {HTMLSelectElement} */ ($('aiModel'));
    for (const [id, m] of Object.entries(MODELS)) {
      model.append(el('option', { value: id, text: `${m.name} · $${m.input} / $${m.output} per million tokens in / out` }));
    }
    model.value = s.model;
    model.addEventListener('change', () => saveSettings({ model: model.value }));
    const effort = /** @type {HTMLSelectElement} */ ($('aiEffort'));
    effort.value = s.effort;
    effort.addEventListener('change', () => saveSettings({ effort: effort.value }));
    const rules = /** @type {HTMLTextAreaElement} */ ($('aiRules'));
    rules.value = s.rules;
    rules.addEventListener('change', () => saveSettings({ rules: rules.value }));

    const secretInput = /** @type {HTMLInputElement} */ ($('aiSecret'));
    $('aiSave').addEventListener('click', async () => {
      const value = secretInput.value.trim();
      if (!value) return;
      setSecret(value);
      secretInput.value = '';
      await check();
    });
    $('aiCheck').addEventListener('click', check);
    $('aiForget').addEventListener('click', () => setSecret(''));
  }

  /** Checks the password or key without spending anything. */
  async function check() {
    const out = $('aiSecretState');
    const s = settings();
    if (!secret()) {
      out.textContent = 'Nothing saved yet.';
      return;
    }
    out.textContent = 'Checking…';
    try {
      if (s.mode === 'proxy') {
        const r = await fetch(`${s.proxyUrl.replace(/\/+$/, '')}/check`, { method: 'POST', headers: { Authorization: `Bearer ${secret()}` } });
        out.textContent = r.ok ? 'Password saved and accepted by the proxy.' : r.status === 401 ? 'The proxy refused this password.' : `The proxy answered ${r.status}.`;
      } else {
        const c = await client();
        await c.models.list({ limit: 1 });
        out.textContent = 'Key saved and accepted by the Claude API.';
      }
    } catch (err) {
      const Anthropic = sdk ? (await sdk).default : null;
      out.textContent = explain(err, Anthropic) || 'Could not check.';
    }
  }

  return {
    /** Loads the saved chat and the proxy address the site was deployed with. */
    async start() {
      try {
        const r = await fetch('config.json', { cache: 'no-store' });
        if (r.ok) {
          const config = await r.json();
          if (typeof config.agentUrl === 'string') defaultProxyUrl = config.agentUrl;
        }
      } catch {
        // No config (local server): the address can be typed in AI settings.
      }
      wire();
      showSecretState();
      await load();
      renderHistory();
    },

    /** The project changed outside the chat (a template or file was opened). */
    projectReplaced() {
      undoSource = null;
      $('chatUndo').hidden = true;
      if (history.length) pendingNote = 'the user opened a different project; earlier messages were about the previous one.';
    },

    get busy() {
      return busy;
    },
  };
}
