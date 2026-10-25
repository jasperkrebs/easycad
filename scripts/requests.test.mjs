// Tests for web/js/requests.js (feature requests), run with `node --test scripts/` (CI does):
// what an issue's state means, what the builder said, and the request's own format.

import assert from 'node:assert/strict';
import { test } from 'node:test';
import { MARKER, builderNote, checkInput, createRequests, formatIssue, hasVersion, newIssueUrl, statusOf } from '../web/js/requests.js';

const INPUT = {
  title: 'Text outlines from a font',
  wanted: 'Raised letters around the vase.',
  missing: 'No step makes curves from text.',
  proposal: 'text(content, size) -> List<Curve>',
  example: 'letters = text("A", size: 12)\n```\nrm -rf /\n```',
};

test('an issue says where its request is', () => {
  const labels = (...names) => names.map((name) => ({ name }));
  assert.equal(statusOf({ state: 'open', labels: labels('feature-request') }), 'waiting');
  assert.equal(statusOf({ state: 'open', labels: labels('feature-request', 'building') }), 'building');
  assert.equal(statusOf({ state: 'open', labels: labels('needs-human') }), 'stuck');
  assert.equal(statusOf({ state: 'open', labels: labels('needs-review') }), 'stuck');
  assert.equal(statusOf({ state: 'closed', state_reason: 'completed', labels: labels('shipped') }), 'shipped');
  assert.equal(statusOf({ state: 'closed', state_reason: 'not_planned', labels: labels('answered') }), 'answered');
  assert.equal(statusOf({ state: 'closed', state_reason: 'not_planned', labels: [] }), 'declined');
  // Closed by hand as done: shipped.
  assert.equal(statusOf({ state: 'closed', state_reason: 'completed', labels: ['feature-request'] }), 'shipped');
});

test("the builder's last comment is its word, with the new steps", () => {
  const note = builderNote([
    { body: 'Thanks!' },
    { body: 'Attempt 1 of 3.\n<!-- easycad-builder attempt 1 -->' },
    { body: 'Built and live.\n\nA `text` step.\n<!-- easycad-builder shipped steps: text, glyph_width -->' },
    { body: 'A comment by someone else' },
  ]);
  assert.deepEqual(note.steps, ['text', 'glyph_width']);
  assert.equal(note.message, 'Built and live.\n\nA `text` step.');
  // Names that aren't step names are dropped.
  assert.deepEqual(builderNote([{ body: '<!-- easycad-builder shipped steps: ok_1, Bad Name, x(y) -->' }]).steps, ['ok_1']);
  assert.deepEqual(builderNote([{ body: 'no builder here' }]), { message: '', steps: [], version: '' });
  // The commit it went live in, when the builder says.
  assert.equal(builderNote([{ body: 'Live.\n<!-- easycad-builder shipped steps: text -->\n<!-- easycad-builder shipped-in: 0cbc6df4648f -->' }]).version, '0cbc6df4648f');
});

test('a shipped step is only there once the app running here has its commit', async () => {
  const asked = [];
  const github = (status) => async (url) => {
    asked.push(String(url));
    return Response.json({ status });
  };
  const o = { repo: 'owner/easycad', version: 'aaa111', running: 'bbb222' };
  assert.equal(await hasVersion({ ...o, fetch: github('ahead') }), true);
  assert.equal(await hasVersion({ ...o, fetch: github('identical') }), true);
  assert.equal(await hasVersion({ ...o, fetch: github('behind') }), false, 'an older page needs a reload');
  assert.equal(await hasVersion({ ...o, fetch: github('diverged') }), false);
  assert.equal(asked[0], 'https://api.github.com/repos/owner/easycad/compare/aaa111...bbb222');
  asked.length = 0;
  assert.equal(await hasVersion({ ...o, running: 'aaa111ccc', fetch: github('behind') }), true, 'the same commit needs no question');
  assert.equal(await hasVersion({ ...o, version: '', fetch: github('behind') }), true, 'nothing to wait for');
  assert.equal(asked.length, 0);
  assert.equal(await hasVersion({ ...o, running: '', fetch: github('ahead') }), null, "a page that doesn't know its version can't tell");
  assert.equal(await hasVersion({ ...o, fetch: async () => new Response('', { status: 403 }) }), null);
});

test('requests are written in one format, with the marker and id first', () => {
  const issue = formatIssue(INPUT, 'fr-abc123');
  assert.equal(issue.title, INPUT.title);
  assert.ok(issue.body.startsWith(`${MARKER} id:fr-abc123 -->\n`));
  for (const part of [INPUT.wanted, INPUT.missing, INPUT.proposal]) assert.ok(issue.body.includes(part));
  // The example can't close its code block early.
  assert.equal(issue.body.match(/```/g).length, 2);
  const url = new URL(newIssueUrl('owner/easycad', issue));
  assert.equal(url.origin + url.pathname, 'https://github.com/owner/easycad/issues/new');
  assert.equal(url.searchParams.get('labels'), 'feature-request');
  assert.equal(url.searchParams.get('title'), INPUT.title);
  assert.equal(checkInput(INPUT), '');
  assert.match(checkInput({ ...INPUT, proposal: ' ' }), /proposal/);
  assert.match(checkInput({ ...INPUT, title: 'two\nlines' }), /one line/);
  assert.match(checkInput({ ...INPUT, wanted: 'x'.repeat(5000) }), /too long/);
});

test('checking asks GitHub only about open requests, and remembers what changed', async () => {
  const store = new Map();
  const storage = { getItem: (k) => store.get(k) ?? null, setItem: (k, v) => store.set(k, v) };
  const calls = [];
  const issue = { number: 7, html_url: 'https://github.com/owner/easycad/issues/7', state: 'closed', state_reason: 'completed', labels: [{ name: 'shipped' }] };
  const fetch = async (url) => {
    calls.push(String(url));
    if (String(url).endsWith('/issues/7')) return new Response(JSON.stringify(issue), { headers: { etag: '"x"' } });
    if (String(url).endsWith('/issues/7/comments?per_page=100')) return Response.json([{ body: 'Done.\n<!-- easycad-builder shipped steps: text -->' }]);
    return new Response('', { status: 404 });
  };
  const requests = createRequests({ repo: () => 'owner/easycad', fetch, storage });
  requests.add({ id: 'fr-1', title: 'Text', wanted: 'Letters', number: 7, status: 'waiting', filedAt: 0 });
  requests.add({ id: 'fr-2', title: 'Old', wanted: 'x', number: 8, status: 'shipped', resumed: true, filedAt: 0 });
  const changed = await requests.check();
  assert.deepEqual(changed.map((r) => [r.id, r.status, r.steps]), [['fr-1', 'shipped', ['text']]]);
  assert.ok(calls.every((c) => !c.includes('/issues/8')), 'finished requests are not asked about');
  assert.equal(requests.list().find((r) => r.id === 'fr-1').etag, '"x"');
  assert.equal(requests.open().length, 0);
});
