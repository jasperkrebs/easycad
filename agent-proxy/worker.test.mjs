// Tests for the AI proxy, run with `node --test agent-proxy/` (CI does). The Claude API is
// replaced by a stub, so nothing is sent anywhere and no key is needed.

import assert from 'node:assert/strict';
import { afterEach, beforeEach, test } from 'node:test';
import worker from './worker.js';

const ENV = { ANTHROPIC_API_KEY: 'sk-test-key', EASYCAD_PASSWORD: 'correct horse battery staple', ALLOWED_ORIGINS: 'https://app.example' };
const ORIGIN = 'https://app.example';

/** @type {{ url: string, init: RequestInit }[]} */
let upstreamCalls = [];
const realFetch = globalThis.fetch;

beforeEach(() => {
  upstreamCalls = [];
  globalThis.fetch = async (url, init) => {
    upstreamCalls.push({ url: String(url), init });
    const stream = new ReadableStream({
      start(c) {
        c.enqueue(new TextEncoder().encode('event: message_stop\ndata: {"type":"message_stop"}\n\n'));
        c.close();
      },
    });
    return new Response(stream, { status: 200, headers: { 'content-type': 'text/event-stream', 'request-id': 'req_1' } });
  };
});
afterEach(() => {
  globalThis.fetch = realFetch;
});

function post(path, body, { password = ENV.EASYCAD_PASSWORD, origin = ORIGIN, headers = {} } = {}) {
  const h = new Headers({ 'content-type': 'application/json', 'anthropic-version': '2023-06-01', ...headers });
  if (password !== null) h.set('Authorization', `Bearer ${password}`);
  if (origin) h.set('Origin', origin);
  return new Request(`https://proxy.example${path}`, { method: 'POST', headers: h, body: typeof body === 'string' ? body : JSON.stringify(body) });
}

const message = { model: 'claude-sonnet-5-5', max_tokens: 1000, messages: [{ role: 'user', content: 'hi' }] };

test('forwards a request with the key and streams the answer back', async () => {
  const res = await worker.fetch(post('/v1/messages?beta=true', message, { headers: { 'anthropic-beta': 'server-side-fallback-2026-07-01', 'x-stainless-os': 'MacOS' } }), ENV);
  assert.equal(res.status, 200);
  assert.equal(res.headers.get('content-type'), 'text/event-stream');
  assert.equal(res.headers.get('access-control-allow-origin'), ORIGIN);
  assert.match(await res.text(), /message_stop/);
  assert.equal(upstreamCalls.length, 1);
  const { url, init } = upstreamCalls[0];
  assert.equal(url, 'https://api.anthropic.com/v1/messages?beta=true');
  const sent = new Headers(init.headers);
  assert.equal(sent.get('x-api-key'), 'sk-test-key');
  assert.equal(sent.get('anthropic-beta'), 'server-side-fallback-2026-07-01');
  // The password and the browser's own headers never reach the Claude API.
  assert.equal(sent.get('authorization'), null);
  assert.equal(sent.get('x-stainless-os'), null);
  assert.deepEqual(JSON.parse(String(init.body)), message);
});

test('a wrong or missing password is refused without calling the API', async () => {
  for (const password of ['wrong', '', null]) {
    const res = await worker.fetch(post('/v1/messages', message, { password }), ENV);
    assert.equal(res.status, 401);
    const body = await res.json();
    assert.equal(body.error.type, 'authentication_error');
  }
  assert.equal(upstreamCalls.length, 0);
});

test('/check answers only with the right password', async () => {
  assert.equal((await worker.fetch(post('/check', {}), ENV)).status, 200);
  assert.equal((await worker.fetch(post('/check', {}, { password: 'nope' }), ENV)).status, 401);
});

test('only allowed pages, models, paths and sizes get through', async () => {
  const other = await worker.fetch(post('/v1/messages', message, { origin: 'https://evil.example' }), ENV);
  assert.equal(other.status, 403);
  assert.equal(other.headers.get('access-control-allow-origin'), null);

  const model = await worker.fetch(post('/v1/messages', { ...message, model: 'claude-fable-5-1' }), ENV);
  assert.equal(model.status, 400);
  assert.match((await model.json()).error.message, /only allows/);

  const big = await worker.fetch(post('/v1/messages', { ...message, max_tokens: 500000 }), ENV);
  assert.equal(big.status, 400);

  const path = await worker.fetch(post('/v1/files', message), ENV);
  assert.equal(path.status, 404);

  const notJson = await worker.fetch(post('/v1/messages', '{oops'), ENV);
  assert.equal(notJson.status, 400);

  const huge = await worker.fetch(post('/v1/messages', { ...message, pad: 'x'.repeat(9 * 1024 * 1024) }), ENV);
  assert.equal(huge.status, 413);

  assert.equal(upstreamCalls.length, 0);
  // Token counting is allowed (it has no max_tokens).
  const count = await worker.fetch(post('/v1/messages/count_tokens', { model: 'claude-haiku-5-5', messages: message.messages }), ENV);
  assert.equal(count.status, 200);
  assert.equal(upstreamCalls[0].url, 'https://api.anthropic.com/v1/messages/count_tokens');
});

test('preflight requests get CORS headers for allowed pages only', async () => {
  const preflight = (origin) =>
    worker.fetch(
      new Request('https://proxy.example/v1/messages', {
        method: 'OPTIONS',
        headers: { Origin: origin, 'Access-Control-Request-Method': 'POST', 'Access-Control-Request-Headers': 'authorization, x-stainless-os' },
      }),
      ENV,
    );
  const ok = await preflight(ORIGIN);
  assert.equal(ok.status, 204);
  assert.equal(ok.headers.get('access-control-allow-origin'), ORIGIN);
  assert.equal(ok.headers.get('access-control-allow-headers'), 'authorization, x-stainless-os');
  const bad = await preflight('https://evil.example');
  assert.equal(bad.headers.get('access-control-allow-origin'), null);
});

test('a proxy without its secrets says so', async () => {
  const res = await worker.fetch(post('/v1/messages', message), { ALLOWED_ORIGINS: ORIGIN });
  assert.equal(res.status, 500);
  assert.match((await res.json()).error.message, /secrets/);
});

test('the health check needs no password and shows no secrets', async () => {
  const res = await worker.fetch(new Request('https://proxy.example/'), ENV);
  assert.equal(res.status, 200);
  const text = await res.text();
  assert.ok(!text.includes('sk-test-key') && !text.includes('horse'));
});

// ---------------------------------------------------------------------------------------------
// Feature requests (POST /feature-request): filed as GitHub issues with a token only the proxy
// knows. GitHub is replaced by a stub too.
// ---------------------------------------------------------------------------------------------

const GH_ENV = { ...ENV, GITHUB_TOKEN: 'ghp-test-token', GITHUB_REPO: 'owner/easycad' };
const REQUEST = { title: 'Ring fill that follows the outline exactly', body: '<!-- easycad-feature-request v1 id:fr-abc123 -->\nWhat the user wants: …' };

/** A stub GitHub: `open` issues are listed; creating one records it. */
function stubGitHub(open = []) {
  const created = [];
  globalThis.fetch = async (url, init = {}) => {
    upstreamCalls.push({ url: String(url), init });
    const u = new URL(String(url));
    if (init.method === 'POST' && u.pathname === '/repos/owner/easycad/issues') {
      const issue = JSON.parse(String(init.body));
      created.push(issue);
      return Response.json({ number: 42, html_url: 'https://github.com/owner/easycad/issues/42' }, { status: 201 });
    }
    if (u.pathname === '/repos/owner/easycad/issues') return Response.json(open);
    return new Response('not found', { status: 404 });
  };
  return created;
}

test('files a feature request as a labelled issue, with the token only the proxy knows', async () => {
  const created = stubGitHub();
  const res = await worker.fetch(post('/feature-request', REQUEST), GH_ENV);
  assert.equal(res.status, 201);
  assert.deepEqual(await res.json(), { number: 42, url: 'https://github.com/owner/easycad/issues/42' });
  assert.equal(created.length, 1);
  assert.deepEqual(created[0], { title: REQUEST.title, body: REQUEST.body, labels: ['feature-request'] });
  const sent = new Headers(upstreamCalls[1].init.headers);
  assert.equal(sent.get('authorization'), 'Bearer ghp-test-token');
  assert.equal(res.headers.get('access-control-allow-origin'), ORIGIN);
});

test('feature requests: duplicates, too many open ones, bad input, no password, not set up', async () => {
  // The same title still open: that issue, nothing new.
  let created = stubGitHub([{ number: 7, title: REQUEST.title.toUpperCase(), html_url: 'https://github.com/owner/easycad/issues/7' }]);
  let res = await worker.fetch(post('/feature-request', REQUEST), GH_ENV);
  assert.deepEqual(await res.json(), { number: 7, url: 'https://github.com/owner/easycad/issues/7', duplicate: true });
  assert.equal(created.length, 0);
  // Five open ones: wait.
  created = stubGitHub([1, 2, 3, 4, 5].map((n) => ({ number: n, title: `request ${n}`, html_url: '' })));
  res = await worker.fetch(post('/feature-request', REQUEST), GH_ENV);
  assert.equal(res.status, 429);
  assert.equal(created.length, 0);
  // Pull requests in the list don't count.
  created = stubGitHub([1, 2, 3, 4, 5].map((n) => ({ number: n, title: `pr ${n}`, html_url: '', pull_request: {} })));
  res = await worker.fetch(post('/feature-request', REQUEST), GH_ENV);
  assert.equal(res.status, 201);
  // Only the app's own requests, one-line titles of a sensible length.
  for (const bad of [
    { ...REQUEST, body: 'please add a step' },
    { ...REQUEST, title: 'x' },
    { ...REQUEST, title: 'two\nlines' },
    { ...REQUEST, body: REQUEST.body + 'x'.repeat(20000) },
  ]) {
    res = await worker.fetch(post('/feature-request', bad), GH_ENV);
    assert.equal(res.status, 400, JSON.stringify(bad).slice(0, 60));
  }
  // The password is checked first.
  upstreamCalls = [];
  res = await worker.fetch(post('/feature-request', REQUEST, { password: 'wrong' }), GH_ENV);
  assert.equal(res.status, 401);
  assert.equal(upstreamCalls.length, 0);
  // Without a token or repo the proxy says how to set it up.
  res = await worker.fetch(post('/feature-request', REQUEST), ENV);
  assert.equal(res.status, 501);
  assert.match((await res.json()).error.message, /feature-requests\.md/);
});
