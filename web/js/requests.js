// Feature requests: when the agent finds that the steps can't do what the user asked for, it
// files a request (an issue in EasyCAD's GitHub repo). A Claude Code session builds the step,
// checks it and ships it (docs/feature-requests.md). This module writes the request, remembers it
// on this device, checks on it, and says when it's done, so the chat can carry on by itself.
//
// Filing goes through the password-protected proxy when there is one. Otherwise the user files
// it with one tap on a prefilled GitHub page, and the request is found again by its id. Checking
// uses GitHub's public API, no token needed.

const STORE = 'easycad.requests';
const API = 'https://api.github.com';
/** Every request starts with this; the proxy and the builder look for it. */
export const MARKER = '<!-- easycad-feature-request v1';
/** Builder comments carry this (docs/feature-requests.md). */
const BUILDER = '<!-- easycad-builder';
const LIMITS = { title: 100, wanted: 3000, missing: 3000, proposal: 5000, example: 5000 };

/**
 * @typedef {{ title: string, wanted: string, missing: string, proposal: string, example?: string }} RequestInput
 * @typedef {'unfiled' | 'waiting' | 'building' | 'shipped' | 'answered' | 'declined' | 'stuck'} Status
 * @typedef {{ id: string, title: string, wanted: string, number?: number, url?: string, fileUrl?: string,
 *   status: Status, steps?: string[], message?: string, filedAt: number, resumed?: boolean, etag?: string }} Request
 */

/** A request id, also written into the issue so it can be found without its number. */
export function newId() {
  const bytes = new Uint8Array(6);
  crypto.getRandomValues(bytes);
  return 'fr-' + [...bytes].map((b) => b.toString(36).padStart(2, '0')).join('').slice(0, 10);
}

/** Checks the agent's tool input; a message for the agent if something's off. */
export function checkInput(input) {
  for (const key of ['title', 'wanted', 'missing', 'proposal']) {
    if (typeof input?.[key] !== 'string' || !input[key].trim()) return `\`${key}\` is required`;
  }
  for (const [key, max] of Object.entries(LIMITS)) {
    if (typeof input[key] === 'string' && input[key].length > max) return `\`${key}\` is too long (${max} characters at most)`;
  }
  if (/[\r\n]/.test(input.title)) return '`title` must be one line';
  return '';
}

/** Code blocks can't be closed early by the text inside them. */
const fenced = (text) => String(text).replace(/`{3,}/g, "'''");

/**
 * The issue for a request: a title, and a body with the marker and id first.
 * @param {RequestInput} input
 * @param {string} id
 */
export function formatIssue(input, id) {
  const sections = [
    `${MARKER} id:${id} -->`,
    `Filed by EasyCAD's in-app agent for the owner. Request id: \`${id}\`. A Claude Code session builds it (docs/feature-requests.md).`,
    `## What the user wants\n\n${input.wanted.trim()}`,
    `## What's missing, and why the existing steps can't do it\n\n${input.missing.trim()}`,
    `## Proposed step\n\n${input.proposal.trim()}`,
  ];
  if (input.example && input.example.trim()) sections.push(`## Example recipe\n\n\`\`\`\n${fenced(input.example.trim())}\n\`\`\``);
  return { title: input.title.trim(), body: sections.join('\n\n') + '\n' };
}

/** GitHub's "new issue" page, filled in, for filing with one tap. */
export function newIssueUrl(repo, issue) {
  const q = new URLSearchParams({ title: issue.title, body: issue.body.slice(0, 6000), labels: 'feature-request' });
  return `https://github.com/${repo}/issues/new?${q}`;
}

/**
 * What an issue's state says about its request.
 * @param {{ state: string, state_reason?: string | null, labels?: Array<{ name: string } | string> }} issue
 * @returns {Status}
 */
export function statusOf(issue) {
  const labels = new Set((issue.labels || []).map((l) => (typeof l === 'string' ? l : l.name)));
  if (issue.state === 'closed') {
    if (labels.has('shipped')) return 'shipped';
    if (labels.has('answered')) return 'answered';
    return issue.state_reason === 'not_planned' ? 'declined' : 'shipped';
  }
  if (labels.has('needs-human') || labels.has('needs-review')) return 'stuck';
  if (labels.has('building')) return 'building';
  return 'waiting';
}

/**
 * The builder's last word on a request: its message (comments removed) and, when it shipped,
 * the new steps' names (`<!-- easycad-builder shipped steps: a, b -->`).
 * @param {Array<{ body?: string }>} comments
 */
export function builderNote(comments) {
  const last = [...comments].reverse().find((c) => String(c.body || '').includes(BUILDER));
  if (!last) return { message: '', steps: [] };
  const body = String(last.body);
  const steps = /<!-- easycad-builder shipped steps:([^>]*)-->/.exec(body)?.[1] || '';
  return {
    message: body.replace(/<!--[\s\S]*?-->/g, '').trim().slice(0, 4000),
    steps: steps
      .split(',')
      .map((s) => s.trim())
      .filter((s) => /^[a-z_][a-z0-9_]*$/.test(s)),
  };
}

/** Requests that still need checking. */
const open = (r) => ['unfiled', 'waiting', 'building', 'stuck'].includes(r.status);

/**
 * The requests filed from this device.
 * @param {{ repo: () => string, fetch?: typeof fetch, storage?: Storage }} o
 */
export function createRequests({ repo, fetch: get = (...a) => globalThis.fetch(...a), storage = globalThis.localStorage }) {
  /** @returns {Request[]} */
  function list() {
    try {
      const v = JSON.parse(storage.getItem(STORE) || '[]');
      return Array.isArray(v) ? v : [];
    } catch {
      return [];
    }
  }

  /** @param {Request[]} all */
  function save(all) {
    try {
      storage.setItem(STORE, JSON.stringify(all.slice(-20)));
    } catch {
      // No storage: requests last until the page closes.
    }
  }

  /** @param {Request} r */
  function add(r) {
    save([...list().filter((x) => x.id !== r.id), r]);
  }

  /** @param {string} id @param {Partial<Request>} patch */
  function update(id, patch) {
    save(list().map((r) => (r.id === id ? { ...r, ...patch } : r)));
  }

  const headers = { Accept: 'application/vnd.github+json' };

  /** Finds an issue filed by hand by its request id. */
  async function find(id) {
    const q = encodeURIComponent(`repo:${repo()} is:issue in:body "${id}"`);
    const res = await get(`${API}/search/issues?q=${q}`, { headers });
    if (!res.ok) return null;
    const items = (await res.json()).items || [];
    return items.find((i) => String(i.body || '').includes(id)) || null;
  }

  /**
   * Looks at every open request on GitHub; returns those that changed.
   * @returns {Promise<Request[]>}
   */
  async function check() {
    const changed = [];
    if (!repo()) return changed;
    for (const r of list().filter(open)) {
      try {
        let issue;
        if (!r.number) {
          issue = await find(r.id);
          if (!issue) continue;
        } else {
          const res = await get(`${API}/repos/${repo()}/issues/${r.number}`, {
            headers: r.etag ? { ...headers, 'If-None-Match': r.etag } : headers,
          });
          if (res.status === 304 || !res.ok) continue;
          issue = await res.json();
          r.etag = res.headers.get('etag') || undefined;
        }
        const status = statusOf(issue);
        const patch = { number: issue.number, url: issue.html_url, etag: r.etag, status };
        if (status !== r.status && !['waiting', 'building'].includes(status)) {
          const res = await get(`${API}/repos/${repo()}/issues/${issue.number}/comments?per_page=100`, { headers });
          if (res.ok) Object.assign(patch, builderNote(await res.json()));
        }
        update(r.id, patch);
        if (status !== r.status || patch.number !== r.number) changed.push({ ...r, ...patch });
      } catch {
        // Offline or rate-limited: try again next time.
      }
    }
    return changed;
  }

  return { list, add, update, check, open: () => list().filter(open) };
}
