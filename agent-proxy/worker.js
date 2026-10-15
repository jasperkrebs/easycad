// EasyCAD's AI proxy: a tiny Cloudflare Worker that holds the Claude API key, so the key never
// reaches a browser, the website or the public repo. The app sends its password; the proxy
// checks it, then forwards the request to the Claude API with the key and streams the answer
// straight back.
//
// Secrets (set by CI from GitHub repository secrets; never written in any file):
//   ANTHROPIC_API_KEY  the Claude API key
//   EASYCAD_PASSWORD   the password the app asks for
// Variable (wrangler.toml, filled in by CI): ALLOWED_ORIGINS, the web pages allowed to call it.
//
// It only forwards what the app needs: the Messages API (and token counting), for EasyCAD's
// three models, with a cap on max_tokens and on the request size. A wrong password waits a
// moment before answering, so guessing is slow.

const API = 'https://api.anthropic.com';
const PATHS = new Set(['/v1/messages', '/v1/messages/count_tokens']);
const MODELS = new Set(['claude-sonnet-5-5', 'claude-opus-5-5', 'claude-haiku-5-5']);
const MAX_TOKENS = 64000;
const MAX_BODY_BYTES = 8 * 1024 * 1024; // room for a few rendered views
const FORWARD = ['anthropic-version', 'anthropic-beta'];
const EXPOSE = ['request-id', 'retry-after', 'content-type'];
const WRONG_PASSWORD_DELAY_MS = 800;

export default {
  /**
   * @param {Request} request
   * @param {{ ANTHROPIC_API_KEY?: string, EASYCAD_PASSWORD?: string, ALLOWED_ORIGINS?: string }} env
   */
  async fetch(request, env) {
    const url = new URL(request.url);
    const origin = request.headers.get('Origin');
    const allowed = !origin || origins(env).includes(origin);
    const cors = corsHeaders(origin, allowed, request);

    if (request.method === 'OPTIONS') return new Response(null, { status: 204, headers: cors });
    if (request.method === 'GET' && url.pathname === '/') {
      return json({ ok: true, service: 'easycad-agent' }, 200, cors);
    }
    if (!allowed) return failure(403, 'permission_error', 'this page may not use the EasyCAD proxy', cors);
    if (!env.ANTHROPIC_API_KEY || !env.EASYCAD_PASSWORD) {
      return failure(500, 'api_error', 'the proxy has no API key or password yet: set both secrets (docs/api-key.md)', cors);
    }
    if (!(await passwordMatches(request, env.EASYCAD_PASSWORD))) {
      await new Promise((r) => setTimeout(r, WRONG_PASSWORD_DELAY_MS));
      return failure(401, 'authentication_error', 'wrong password', cors);
    }
    if (request.method === 'POST' && url.pathname === '/check') return json({ ok: true }, 200, cors);
    if (request.method !== 'POST' || !PATHS.has(url.pathname)) {
      return failure(404, 'not_found_error', `the proxy only forwards ${[...PATHS].join(' and ')}`, cors);
    }

    if (Number(request.headers.get('content-length') || 0) > MAX_BODY_BYTES) {
      return failure(413, 'request_too_large', 'the request is too large', cors);
    }
    const text = await request.text();
    if (text.length > MAX_BODY_BYTES) return failure(413, 'request_too_large', 'the request is too large', cors);
    let body;
    try {
      body = JSON.parse(text);
    } catch {
      return failure(400, 'invalid_request_error', 'the request is not JSON', cors);
    }
    if (!body || typeof body !== 'object' || !MODELS.has(body.model)) {
      return failure(400, 'invalid_request_error', `the proxy only allows ${[...MODELS].join(', ')}`, cors);
    }
    if (url.pathname === '/v1/messages') {
      const max = body.max_tokens;
      if (!Number.isInteger(max) || max < 0 || max > MAX_TOKENS) {
        return failure(400, 'invalid_request_error', `max_tokens must be a whole number up to ${MAX_TOKENS}`, cors);
      }
    }

    const headers = new Headers({ 'x-api-key': env.ANTHROPIC_API_KEY, 'content-type': 'application/json' });
    for (const name of FORWARD) {
      const value = request.headers.get(name);
      if (value) headers.set(name, value);
    }
    if (!headers.has('anthropic-version')) headers.set('anthropic-version', '2023-06-01');

    let upstream;
    try {
      upstream = await fetch(API + url.pathname + url.search, { method: 'POST', headers, body: text });
    } catch {
      return failure(502, 'api_error', 'the Claude API could not be reached', cors);
    }
    // Stream the answer through untouched: the app reads the server-sent events as they come.
    const out = new Headers(cors);
    for (const name of EXPOSE) {
      const value = upstream.headers.get(name);
      if (value) out.set(name, value);
    }
    return new Response(upstream.body, { status: upstream.status, headers: out });
  },
};

/** The pages allowed to call the proxy (wrangler.toml / CI). */
function origins(env) {
  return String(env.ALLOWED_ORIGINS || '')
    .split(',')
    .map((s) => s.trim())
    .filter(Boolean);
}

function corsHeaders(origin, allowed, request) {
  /** @type {Record<string, string>} */
  const headers = { Vary: 'Origin' };
  if (origin && allowed) {
    headers['Access-Control-Allow-Origin'] = origin;
    headers['Access-Control-Allow-Methods'] = 'GET, POST, OPTIONS';
    headers['Access-Control-Allow-Headers'] =
      request.headers.get('Access-Control-Request-Headers') || 'authorization, content-type, anthropic-version, anthropic-beta';
    headers['Access-Control-Expose-Headers'] = EXPOSE.join(', ');
    headers['Access-Control-Max-Age'] = '86400';
  }
  return headers;
}

/**
 * Compares the password from `Authorization: Bearer …` with the secret in constant time:
 * both are hashed first, so neither the length nor the first wrong character leaks.
 */
async function passwordMatches(request, secret) {
  const header = request.headers.get('Authorization') || '';
  const given = header.startsWith('Bearer ') ? header.slice(7) : '';
  const [a, b] = await Promise.all([sha256(given), sha256(secret)]);
  let diff = given.length === 0 ? 1 : 0;
  for (let i = 0; i < a.length; i++) diff |= a[i] ^ b[i];
  return diff === 0;
}

async function sha256(text) {
  return new Uint8Array(await crypto.subtle.digest('SHA-256', new TextEncoder().encode(text)));
}

function json(data, status, headers) {
  return new Response(JSON.stringify(data), { status, headers: { ...headers, 'content-type': 'application/json' } });
}

/** An error in the Claude API's own format, so the app's SDK reports it like any API error. */
function failure(status, type, message, headers) {
  return json({ type: 'error', error: { type, message } }, status, headers);
}
