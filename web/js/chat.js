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
//
// When the agent can't build something, it proposes a feature request (web/js/requests.js); the
// user sends it to be built or revises what they asked for. The requests show above the message
// box; when one is done, the chat carries on by itself (a message from the app), once this page
// runs the version the step went live in (it reloads for it) and, when the project's template was
// improved too, once the user has chosen to update to it or keep their own.

import { CONTEXT_TAG, DEFAULT_MODEL, EFFORTS, MODELS, applyEdits, closeToolUses, numbered, projectContext, runStatus, runTurn, systemPrompt } from './agent.js';
import { $, el } from './dom.js';
import { PRINTERS } from './print.js';
import { listValue } from './settings.js';
import { createRequests, formatIssue, hasVersion, newId, newIssueUrl } from './requests.js';

const SETTINGS_KEY = 'easycad.ai';
const SECRET_KEY = 'easycad.ai.secret';
const SPEND_KEY = 'easycad.ai.spend';
const DB_NAME = 'easycad';
const DB_STORE = 'chat';

/** How often open feature requests are checked while the app is open. */
const REQUEST_POLL_MS = 5 * 60 * 1000;
/** Reload by itself for a shipped step only after this long without a touch or key. */
const IDLE_RELOAD_MS = 60 * 1000;
const RELOAD_FLAG = 'easycad.requests.reloaded';

/**
 * @typedef {{ mode: 'proxy' | 'key', proxyUrl: string, model: string, effort: string, rules: string, autoContinue: boolean }} AiSettings
 * @typedef {{ source: string, last: any, printer: string, values: Record<string, any>, editorDirty: boolean }} Project
 */

/**
 * @param {object} app
 * @param {() => Project} app.project the open project
 * @param {(source: string) => Promise<any>} app.applyRecipe replaces the recipe, runs it, returns the result
 * @param {(values: Record<string, any>) => Promise<any>} app.applyValues moves settings, runs, returns the result
 * @param {(id: string | undefined, settings: Record<string, unknown> | undefined, material: string | undefined) => Promise<{ printer: string, notes: string[] }>} app.setPrinter
 *   switches the printer and/or changes its settings; returns the printer as text for the agent
 * @param {(show: string) => string} app.look renders the four views, returns a JPEG data URL
 * @param {() => Promise<string>} app.catalog the engine's step catalog
 * @param {() => Promise<{ update: () => Promise<void>, keep: () => void } | null>} app.templateOffer
 *   the project's template has been improved and the user hasn't said yet whether to update to it
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
  /** EasyCAD's GitHub repository ("owner/name"), where feature requests go. */
  let repo = '';
  /** The commit this page was deployed from (config.json), to tell when a shipped step is here. */
  let runningVersion = '';
  /** Some request is done but waits for the user's answer to a template update. */
  let waitingForTemplate = false;
  const requests = createRequests({ repo: () => repo });
  /** @type {ReturnType<typeof setTimeout> | null} */
  let pollTimer = null;
  let lastActivity = Date.now();
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
      autoContinue: s.autoContinue !== false,
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
      case 'set_printer': {
        const parts = [];
        if (i.printer) parts.push(`Switched to the ${PRINTERS[i.printer]?.name || i.printer}`);
        if (i.material) parts.push(`${parts.length ? 'used' : 'Used'} the ${i.material} profile`);
        const set = Object.entries(i.settings || {}).map(([k, v]) => `${k} = ${v}`);
        if (set.length) parts.push(`${parts.length ? 'set' : 'Set the printer’s'} ${set.join(', ')}`);
        return parts.join('; ');
      }
      case 'look':
        return `Looked at the ${i.show === 'toolpath' ? 'toolpath' : i.show === 'shape' ? 'shape' : 'model'}`;
      case 'read_recipe':
        return 'Read the recipe';
      case 'request_feature':
        return `Proposed a new step: ${i.title || ''}`;
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
        } else if (p.kind === 'list') {
          const r = listValue(p, raw);
          if ('error' in r) {
            notes.push(`\`${key}\` ${r.error}`);
            continue;
          }
          v = r.value;
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
    if (name === 'set_printer') {
      const { printer, notes } = await app.setPrinter(input.printer, input.settings, input.material);
      return { content: (notes.length ? `Note: ${notes.join('; ')}.\n` : '') + `Printer: ${printer}` };
    }
    if (name === 'request_feature') return proposeRequest(input);
    return { content: `There is no tool called ${name}.`, isError: true };
  }

  const hasErrors = (result) => !result || result.diagnostics.some((d) => d.severity === 'error');

  // ---------------------------------------------------------------------------------------------
  // Feature requests: filed by the agent, built by Claude Code, picked up again here
  // ---------------------------------------------------------------------------------------------

  /** Adds to the note the model gets with the next message. */
  function addNote(text) {
    pendingNote = pendingNote ? `${pendingNote} Also: ${text}` : text;
  }

  /**
   * The agent's request, shown to the user to sign off: nothing is filed until they send it.
   * @param {import('./requests.js').RequestInput} input
   */
  function proposeRequest(input) {
    requests.add({ id: newId(), title: input.title.trim(), wanted: input.wanted.trim(), input, status: 'proposed', filedAt: Date.now() });
    renderRequests();
    return {
      content:
        'Not filed yet: the app shows the request to the user, who either sends it to be built or revises what they asked for. Tell the user in a sentence or two what the new step would do and that it is built only if they tap "Send to be built" (you will carry on by yourself once it is live), then stop.',
    };
  }

  /** The user revised instead: the proposal goes, and the agent hears why. */
  function reviseRequest(r) {
    requests.update(r.id, { resumed: true });
    renderRequests();
    addNote(`the user didn't send the feature request "${r.title}"; they want to change what they asked for instead. Don't propose it again unless they ask for it.`);
    input.focus();
  }

  /**
   * Files a signed-off request through the proxy, or, without one, as a link the user taps to
   * file it. Returns an error message, or '' when it went out.
   * @param {import('./requests.js').Request} r
   */
  async function fileRequest(r) {
    if (!r.input) return 'This request has nothing to file.';
    const issue = formatIssue(r.input, r.id);
    const base = { id: r.id, title: issue.title, wanted: r.wanted, filedAt: Date.now() };
    const s = settings();
    if (s.mode === 'proxy' && s.proxyUrl && secret()) {
      try {
        const res = await fetch(`${s.proxyUrl.replace(/\/+$/, '')}/feature-request`, {
          method: 'POST',
          headers: { Authorization: `Bearer ${secret()}`, 'content-type': 'application/json' },
          body: JSON.stringify(issue),
        });
        const data = await res.json().catch(() => ({}));
        if (res.ok && Number.isInteger(data.number)) {
          requests.add({ ...base, number: data.number, url: data.url, status: 'waiting' });
          renderRequests();
          schedulePoll(REQUEST_POLL_MS);
          const same = data.duplicate ? ' (the same request was already open, so that is the one being built)' : '';
          addNote(`the user sent the feature request "${issue.title}"; it is filed as #${data.number}${same}. The app brings you back here when it's done.`);
          return '';
        }
        if (res.status !== 501) return `Not sent: ${data.error?.message || `the proxy answered ${res.status}`}.`;
      } catch {
        // The proxy can't be reached: the user can still file it by hand.
      }
    }
    if (!repo) return "Not sent: this copy of the app doesn't know its GitHub repository.";
    requests.add({ ...base, fileUrl: newIssueUrl(repo, issue), status: 'unfiled' });
    renderRequests();
    schedulePoll(REQUEST_POLL_MS);
    return '';
  }

  const STATUS_TEXT = {
    proposed: 'needs your OK',
    unfiled: 'not filed yet',
    waiting: 'waiting for the builder',
    building: 'being built',
    shipped: 'ready',
    answered: 'answered',
    declined: 'not built',
    stuck: 'needs a person',
  };

  /** The strip of requests above the message box. */
  function renderRequests() {
    const list = $('chatRequests');
    const shown = requests.list().filter((r) => !r.resumed);
    list.hidden = !shown.length;
    list.replaceChildren(
      ...shown.map((r) => {
        const head = el('div', { className: 'request-head' }, [
          r.url ? el('a', { href: r.url, target: '_blank', rel: 'noopener', text: `#${r.number} ${r.title}` }) : el('span', { text: r.title }),
          el('span', { className: 'request-status', text: STATUS_TEXT[r.status] || r.status }),
        ]);
        const parts = [head];
        if (r.status === 'proposed') {
          parts.push(el('p', { text: r.input?.proposal || r.wanted }));
          parts.push(el('p', { className: 'request-ask', text: 'The AI can’t do this with the steps there are. Send it to be built (a Claude Code session on GitHub builds and tests the new step, usually within an hour), or revise what you asked for.' }));
          if (r.error) parts.push(el('p', { className: 'request-error', text: r.error }));
          const send = el('button', { className: 'btn primary', type: 'button', text: 'Send to be built' });
          const revise = el('button', { className: 'btn', type: 'button', text: 'Revise my request' });
          send.addEventListener('click', async () => {
            send.disabled = revise.disabled = true;
            const error = await fileRequest(r);
            if (error) {
              requests.update(r.id, { error });
              renderRequests();
            }
          });
          revise.addEventListener('click', () => reviseRequest(r));
          parts.push(el('div', { className: 'buttons tight' }, [send, revise]));
        } else if (r.status === 'unfiled' && r.fileUrl) {
          parts.push(el('p', { text: 'The AI asked for a new step. File the request on GitHub (one tap, signed in), and it gets built.' }));
          parts.push(el('div', { className: 'buttons tight' }, [el('a', { className: 'btn primary', href: r.fileUrl, target: '_blank', rel: 'noopener', text: 'File it on GitHub' })]));
        } else if (waitingForTemplate && ['shipped', 'answered'].includes(r.status)) {
          parts.push(el('p', { text: 'Done, and the project’s template was improved too (see Settings). Update to it, or keep your own recipe; then the chat carries on.' }));
          const update = el('button', { className: 'btn primary', type: 'button', text: 'Update to the template' });
          const keep = el('button', { className: 'btn', type: 'button', text: 'Keep mine' });
          update.addEventListener('click', async () => {
            update.disabled = keep.disabled = true;
            await (await app.templateOffer())?.update();
          });
          keep.addEventListener('click', async () => (await app.templateOffer())?.keep());
          parts.push(el('div', { className: 'buttons tight' }, [update, keep]));
        } else if (r.status === 'shipped' && !r.resumed) {
          parts.push(el('p', { text: 'The new step is live. Reload to get it; the chat carries on by itself.' }));
          const reload = el('button', { className: 'btn primary', type: 'button', text: 'Reload and carry on' });
          reload.addEventListener('click', () => reloadForStep());
          parts.push(el('div', { className: 'buttons tight' }, [reload]));
        } else if (['stuck', 'declined'].includes(r.status)) {
          if (r.message) parts.push(el('p', { text: r.message }));
          const dismiss = el('button', { className: 'btn', type: 'button', text: 'Dismiss' });
          dismiss.addEventListener('click', () => {
            requests.update(r.id, { resumed: true });
            renderRequests();
          });
          parts.push(el('div', { className: 'buttons tight' }, [dismiss]));
        }
        return el('li', { className: r.status === 'shipped' ? 'ready' : r.status === 'stuck' ? 'stuck' : '' }, parts);
      }),
    );
  }

  /** Requests still to check on, or shipped and waiting for the reload that brings their step. */
  const pending = () => requests.open().length > 0 || requests.list().some((r) => r.status === 'shipped' && !r.resumed);

  /** Checks on open requests now, and again later while any are pending. */
  function schedulePoll(delay) {
    if (pollTimer) clearTimeout(pollTimer);
    pollTimer = null;
    if (!pending()) return;
    pollTimer = setTimeout(async () => {
      pollTimer = null;
      await pollRequests();
    }, delay);
  }

  async function pollRequests() {
    const changed = await requests.check();
    if (changed.length) renderRequests();
    await carryOn();
    schedulePoll(REQUEST_POLL_MS);
  }

  /** Reloads the app for a shipped step (the new engine), marking it so this happens once. */
  function reloadForStep() {
    try {
      sessionStorage.setItem(RELOAD_FLAG, String(Date.now()));
    } catch {
      // Without session storage it may reload again later; harmless.
    }
    location.reload();
  }

  /** Whether the running engine has all of these steps. */
  async function hasSteps(steps) {
    if (!steps || !steps.length) return true;
    const catalog = await app.catalog();
    return steps.every((name) => new RegExp(`^${name}\\(`, 'm').test(catalog));
  }

  /**
   * Whether this page already runs a shipped step: the version it went live in (a new input on a
   * step the catalog already names looks the same by name alone), and its steps.
   * @param {import('./requests.js').Request} r
   */
  async function runsShipped(r) {
    const has = await hasVersion({ repo, version: r.version || '', running: runningVersion });
    if (has === false) return false;
    if (has === null) {
      // GitHub can't say: has the site moved on since this page was loaded?
      try {
        const live = await (await fetch('config.json', { cache: 'no-store' })).json();
        if (typeof live.version === 'string' && live.version && live.version !== runningVersion) return false;
      } catch {
        // No config to compare with: go by the steps' names.
      }
    }
    return hasSteps(r.steps);
  }

  /**
   * Picks the conversation up again for every request that's done: a message from the app
   * that says what happened, in the chat. A shipped step needs the new engine: the app reloads
   * by itself when nobody is using it, or asks. When the project's template was improved too,
   * the user first chooses whether to update to it.
   */
  async function carryOn() {
    const s = settings();
    const before = waitingForTemplate;
    waitingForTemplate = false;
    for (const r of requests.list()) {
      if (r.resumed || !['shipped', 'answered', 'declined', 'stuck'].includes(r.status)) continue;
      if (r.status === 'stuck') {
        pendingNote = `feature request #${r.number} ("${r.title}") needs a person to finish it: ${r.message || 'see the issue'}`;
        continue;
      }
      if (r.status === 'shipped' && !(await runsShipped(r))) {
        let reloaded = 0;
        try {
          reloaded = Number(sessionStorage.getItem(RELOAD_FLAG) || 0);
        } catch {
          reloaded = 0;
        }
        // The new site can take a minute to reach every server: retry, but not in a loop.
        const idle = document.hidden || Date.now() - lastActivity > IDLE_RELOAD_MS;
        if (idle && !busy && !app.project().editorDirty && Date.now() - reloaded > 3 * 60 * 1000) reloadForStep();
        continue;
      }
      if (r.status !== 'declined' && (await app.templateOffer())) {
        waitingForTemplate = true;
        continue;
      }
      if (!s.autoContinue || busy || !ready()) continue;
      requests.update(r.id, { resumed: true });
      renderRequests();
      const what =
        r.status === 'shipped'
          ? `is built and live${r.steps?.length ? `: the catalog now has ${r.steps.map((n) => `\`${n}\``).join(', ')}` : ''}`
          : r.status === 'answered'
            ? `was answered without new steps: ${r.message || 'see the issue'}`
            : `won't be built: ${r.message || 'see the issue'}`;
      await send(`(EasyCAD: feature request #${r.number} "${r.title}" ${what}.) Carry on with what I asked for: ${r.wanted}`);
    }
    if (waitingForTemplate !== before) renderRequests();
  }

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
    // A request that finished while this turn ran carries on after it.
    if (requests.list().some((r) => !r.resumed && ['shipped', 'answered', 'declined'].includes(r.status))) setTimeout(carryOn, 0);
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
    const auto = /** @type {HTMLInputElement} */ ($('aiAutoContinue'));
    auto.checked = s.autoContinue;
    auto.addEventListener('change', () => {
      saveSettings({ autoContinue: auto.checked });
      if (auto.checked) carryOn();
    });

    // Requests are checked when the app comes back into view, and it notices being used (so a
    // reload for a new step never interrupts anyone).
    for (const type of ['pointerdown', 'keydown']) document.addEventListener(type, () => (lastActivity = Date.now()), { passive: true });
    document.addEventListener('visibilitychange', () => {
      if (pending()) pollRequests();
    });

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
          if (typeof config.repo === 'string' && /^[\w.-]+\/[\w.-]+$/.test(config.repo)) repo = config.repo;
          if (typeof config.version === 'string' && /^[0-9a-f]{7,40}$/.test(config.version)) runningVersion = config.version;
        }
      } catch {
        // No config (local server): the address can be typed in AI settings.
      }
      // On GitHub Pages the repository follows from the address (owner.github.io/name/).
      if (!repo && location.hostname.endsWith('.github.io')) {
        const name = location.pathname.split('/').filter(Boolean)[0];
        if (name) repo = `${location.hostname.split('.')[0]}/${name}`;
      }
      wire();
      showSecretState();
      await load();
      renderHistory();
      renderRequests();
      // Requests that finished while the app was closed carry on now (this is a fresh load, so
      // the engine is the newest).
      pollRequests();
    },

    /**
     * The project changed outside the chat: a template or file was opened, or the project was
     * updated to its template's current version.
     * @param {'opened' | 'updated'} how
     */
    projectReplaced(how = 'opened') {
      undoSource = null;
      $('chatUndo').hidden = true;
      if (!history.length) return;
      if (how === 'updated') addNote("the user updated the project to its template's current version (their settings carried over). The recipe changed: read it before editing; it may already do what they asked for.");
      else pendingNote = 'the user opened a different project; earlier messages were about the previous one.';
    },

    /** The user answered the template update (updated or kept their own): carry on. */
    templateAnswered(updated) {
      if (!updated && history.length) addNote("the user kept their own recipe rather than the template's current version.");
      if (waitingForTemplate) carryOn();
    },

    get busy() {
      return busy;
    },
  };
}
