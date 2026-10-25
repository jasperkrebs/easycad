// End-to-end test of feature requests in the Chat tab, against stand-ins for the Claude API, the
// proxy and GitHub, so it costs nothing and needs no tokens:
// - the model can't build something and calls request_feature; the app shows the proposal, files
//   nothing until the user taps "Send to be built", then files it through the proxy and shows it
//   as waiting;
// - the request ships (GitHub says closed, labelled "shipped", with the builder's comment and the
//   commit it went live in, which this page has); when the app checks, the chat carries on by
//   itself with a message from the app, and the model answers;
// - a shipped step this page doesn't run yet (a step the engine lacks, or a commit newer than the
//   page's) asks for a reload instead;
// - when the project's template was improved too, the chat waits for "Update" or "Keep mine";
// - without a proxy that can file requests, the app offers a prefilled GitHub page instead;
// - "Revise my request" drops the proposal and tells the model so.
//
//   node scripts/request-test.cjs [screenshot-dir]
//
// Needs Playwright and a built engine, like scripts/smoke-test.cjs.

const { chromium } = require('playwright');
const { spawn } = require('node:child_process');
const http = require('node:http');
const path = require('node:path');

const root = path.join(__dirname, '..', 'web');
const webPort = 8500 + Math.floor(Math.random() * 400);
const shots = process.argv[2];
const PASSWORD = 'correct horse battery staple';
const REPO = 'owner/easycad';

const REQUEST = {
  title: 'Text outlines from a font',
  wanted: 'Raised letters "JASPER" around the vase.',
  missing: 'No step makes curves from text, and points or polylines would mean drawing every letter by hand.',
  proposal: 'text(content: Text, size: Length, font: Text = "sans") -> List<Curve>: closed outlines of each glyph, baseline on the X axis.',
  example: 'letters = text("JASPER", size: 12)',
};

/** What the scripted model says, from the conversation so far. */
function reply(messages) {
  const last = messages.at(-1);
  const blocks = Array.isArray(last.content) ? last.content : [{ type: 'text', text: last.content }];
  const results = blocks.filter((b) => b.type === 'tool_result');
  const text = blocks.map((b) => (b.type === 'text' ? b.text : '')).join(' ');
  if (/\(EasyCAD: feature request #\d+/.test(text)) return { text: 'Carrying on: the new step is here, building the letters now.' };
  if (results.length) return { text: 'I asked for a step that makes letters; I will carry on when it is ready.' };
  if (/simpler/.test(text)) return { text: 'Fine, a simpler version.' };
  if (/second/.test(text)) return { tool: { name: 'request_feature', input: { ...REQUEST, title: 'Knurled grip pattern' } } };
  if (/third/.test(text)) return { tool: { name: 'request_feature', input: { ...REQUEST, title: 'Brim ears' } } };
  return { tool: { name: 'request_feature', input: REQUEST } };
}

function sse(messages, model) {
  const r = reply(messages);
  const events = [];
  const send = (type, data) => events.push(`event: ${type}\ndata: ${JSON.stringify({ type, ...data })}\n\n`);
  send('message_start', {
    message: { id: 'msg', type: 'message', role: 'assistant', model, content: [], stop_reason: null, stop_sequence: null, usage: { input_tokens: 40, output_tokens: 1 } },
  });
  if (r.text) {
    send('content_block_start', { index: 0, content_block: { type: 'text', text: '' } });
    send('content_block_delta', { index: 0, delta: { type: 'text_delta', text: r.text } });
    send('content_block_stop', { index: 0 });
  } else {
    send('content_block_start', { index: 0, content_block: { type: 'tool_use', id: `toolu_${messages.length}`, name: r.tool.name, input: {} } });
    send('content_block_delta', { index: 0, delta: { type: 'input_json_delta', partial_json: JSON.stringify(r.tool.input) } });
    send('content_block_stop', { index: 0 });
  }
  send('message_delta', { delta: { stop_reason: r.tool ? 'tool_use' : 'end_turn', stop_sequence: null }, usage: { output_tokens: 50 } });
  send('message_stop', {});
  return events.join('');
}

/** The stand-in proxy (Messages API + /feature-request). */
function startMock(state) {
  const server = http.createServer((req, res) => {
    const cors = {
      'Access-Control-Allow-Origin': req.headers.origin || '*',
      'Access-Control-Allow-Methods': 'GET, POST, OPTIONS',
      'Access-Control-Allow-Headers': req.headers['access-control-request-headers'] || '*',
    };
    if (req.method === 'OPTIONS') {
      res.writeHead(204, cors);
      res.end();
      return;
    }
    let body = '';
    req.on('data', (c) => (body += c));
    req.on('end', () => {
      const route = new URL(req.url, 'http://mock').pathname;
      if (req.headers.authorization !== `Bearer ${PASSWORD}`) {
        res.writeHead(401, { ...cors, 'content-type': 'application/json' });
        res.end('{"type":"error","error":{"type":"authentication_error","message":"wrong password"}}');
        return;
      }
      if (route === '/check') {
        res.writeHead(200, { ...cors, 'content-type': 'application/json' });
        res.end('{"ok":true}');
      } else if (route === '/feature-request') {
        const issue = JSON.parse(body);
        state.filed.push(issue);
        if (!state.canFile) {
          res.writeHead(501, { ...cors, 'content-type': 'application/json' });
          res.end('{"type":"error","error":{"type":"not_configured","message":"feature requests are not set up on this proxy"}}');
          return;
        }
        res.writeHead(201, { ...cors, 'content-type': 'application/json' });
        res.end(JSON.stringify({ number: 42, url: `https://github.com/${REPO}/issues/42` }));
      } else if (route === '/v1/messages') {
        const json = JSON.parse(body);
        state.messages.push(json);
        res.writeHead(200, { ...cors, 'content-type': 'text/event-stream' });
        res.end(sse(json.messages, json.model));
      } else {
        res.writeHead(404, cors);
        res.end();
      }
    });
  });
  return new Promise((resolve) => server.listen(0, '127.0.0.1', () => resolve({ server, port: server.address().port })));
}

async function waitForServer(url) {
  for (let i = 0; i < 50; i++) {
    try {
      if ((await fetch(url)).ok) return;
    } catch {
      // not up yet
    }
    await new Promise((r) => setTimeout(r, 100));
  }
  throw new Error('web server did not start');
}

(async () => {
  const web = spawn('python3', ['-m', 'http.server', String(webPort), '--bind', '127.0.0.1'], { cwd: root, stdio: 'ignore' });
  const state = { filed: [], messages: [], canFile: true, issue: { number: 42, state: 'open', labels: [{ name: 'feature-request' }], html_url: `https://github.com/${REPO}/issues/42` }, comments: [] };
  const mock = await startMock(state);
  const failures = [];
  const check = (ok, message) => ok || failures.push(message);
  let browser;
  try {
    const url = `http://127.0.0.1:${webPort}/index.html`;
    await waitForServer(url);
    browser = await chromium.launch();
    // No service worker: it would answer config.json itself, past the stand-in below.
    const ctx = await browser.newContext({ viewport: { width: 1280, height: 900 }, serviceWorkers: 'block' });
    const page = await ctx.newPage();
    page.on('pageerror', (e) => failures.push(`page error: ${e.message}`));
    page.on('console', (m) => {
      if (m.type() === 'error' && !/401|501|Unauthorized|Not Implemented/.test(m.text())) failures.push(`console error: ${m.text()}`);
    });
    // The site's config names the proxy and the repository; GitHub's API is a stand-in.
    // This page is version bbb2222; the first step went live in aaa1111 (older), a later one in
    // ccc3333 (newer than the page).
    await page.route('**/config.json', (r) => r.fulfill({ json: { agentUrl: `http://127.0.0.1:${mock.port}`, repo: REPO, version: 'bbb2222' } }));
    const githubCalls = [];
    await page.route('https://api.github.com/**', (r) => {
      const u = new URL(r.request().url());
      githubCalls.push(u.pathname);
      if (u.pathname === `/repos/${REPO}/issues/42`) return r.fulfill({ json: state.issue, headers: { etag: `"v${state.issue.state}"`, 'access-control-allow-origin': '*' } });
      if (u.pathname === `/repos/${REPO}/issues/42/comments`) return r.fulfill({ json: state.comments, headers: { 'access-control-allow-origin': '*' } });
      if (u.pathname === '/search/issues') return r.fulfill({ json: { items: [] }, headers: { 'access-control-allow-origin': '*' } });
      if (u.pathname === `/repos/${REPO}/compare/aaa1111...bbb2222`) return r.fulfill({ json: { status: 'ahead' }, headers: { 'access-control-allow-origin': '*' } });
      if (u.pathname === `/repos/${REPO}/compare/ccc3333...bbb2222`) return r.fulfill({ json: { status: 'behind' }, headers: { 'access-control-allow-origin': '*' } });
      return r.fulfill({ status: 404, body: '' });
    });
    await page.goto(url);
    await page.waitForFunction(() => /Engine ready/.test(document.getElementById('status').textContent), null, { timeout: 30000 });
    await page.click('#tab-chat');
    await page.click('#aiSettings summary');
    await page.fill('#aiSecret', PASSWORD);
    await page.click('#aiSave');
    await page.waitForFunction(() => /accepted/.test(document.getElementById('aiSecretState').textContent), null, { timeout: 15000 });

    // The model can't do it and proposes a request; nothing is filed until the user sends it.
    await page.fill('#chatText', 'Put my name in raised letters around the vase.');
    await page.click('#chatSend');
    await page.waitForFunction(() => /I asked for a step that makes letters/.test(document.getElementById('chatLog').textContent), null, { timeout: 30000 });
    check(state.filed.length === 0, 'the request was filed before the user sent it');
    await page.waitForSelector('#chatRequests li:has-text("Text outlines from a font") button:has-text("Send to be built")');
    check(/needs your OK/.test(await page.textContent('#chatRequests')), 'the proposal is not shown as needing an OK');
    check((await page.textContent('#chatRequests')).includes(REQUEST.proposal), "the proposal doesn't say what would be built");
    if (shots) await page.screenshot({ path: path.join(shots, 'request-proposed.png') });
    await page.click('#chatRequests button:has-text("Send to be built")');
    await page.waitForSelector('#chatRequests li:has-text("#42 Text outlines from a font")');
    check(state.filed.length === 1, `expected one request filed, got ${state.filed.length}`);
    const issue = state.filed[0] || { title: '', body: '' };
    check(issue.title === REQUEST.title && issue.body.startsWith('<!-- easycad-feature-request v1 id:fr-'), `the request is not in the app's format: ${issue.title} / ${issue.body.slice(0, 60)}`);
    check(issue.body.includes(REQUEST.proposal) && issue.body.includes('## Example recipe'), 'the request lacks the proposal or the example');
    const toolResult = state.messages[1]?.messages.at(-1).content.find((c) => c.type === 'tool_result');
    check(/Not filed yet/.test(String(toolResult?.content)), `the model wasn't told the user signs it off: ${JSON.stringify(toolResult?.content)}`);
    check(/waiting for the builder/.test(await page.textContent('#chatRequests')), 'the request is not shown as waiting');
    check((await page.textContent('#chatLog')).includes('Proposed a new step: Text outlines from a font'), 'no chip for the request in the chat');
    if (shots) await page.screenshot({ path: path.join(shots, 'request-waiting.png') });

    // The builder ships it (here as a step the engine has, so no reload is needed); when the
    // app checks, the chat carries on by itself.
    state.issue = { ...state.issue, state: 'closed', state_reason: 'completed', labels: [{ name: 'feature-request' }, { name: 'shipped' }] };
    state.comments = [{ body: 'Built the step and shipped it.\n<!-- easycad-builder shipped steps: circle -->\n<!-- easycad-builder shipped-in: aaa1111 -->' }];
    const before = state.messages.length;
    await page.evaluate(() => document.dispatchEvent(new Event('visibilitychange')));
    await page.waitForFunction(() => /Carrying on: the new step is here/.test(document.getElementById('chatLog').textContent), null, { timeout: 30000 });
    const resume = state.messages[before]?.messages.at(-1).content.map((c) => c.text || '').join(' ');
    check(/\(EasyCAD: feature request #42 "Text outlines from a font" is built and live: the catalog now has `circle`\.\)/.test(resume), `the app's message is wrong: ${resume}`);
    check(/the user sent the feature request "Text outlines from a font"; it is filed as #42/.test(resume), `the model wasn't told the user sent it: ${resume}`);
    check(resume.includes(REQUEST.wanted), "the app's message doesn't repeat what the user wanted");
    check(!(await page.isVisible('#chatRequests li:has-text("#42")')), 'the finished request is still listed');
    check(githubCalls.includes(`/repos/${REPO}/issues/42/comments`), "the builder's comment wasn't read");
    if (shots) await page.screenshot({ path: path.join(shots, 'request-resumed.png') });

    // Shipped steps this page doesn't run yet: one the engine lacks, and one (a new input on a
    // step the engine already has) that went live after this page's version. The app asks for a
    // reload (it reloads by itself only when nobody is using it).
    const addRequests = (list) =>
      page.evaluate((list) => {
        const all = JSON.parse(localStorage.getItem('easycad.requests') || '[]').map((r) => ({ ...r, resumed: true }));
        localStorage.setItem('easycad.requests', JSON.stringify([...all, ...list]));
      }, list);
    const shipped = (n, title, more) => ({ id: `fr-test${n}`, title, wanted: `${title}, please.`, number: n, url: `https://github.com/${REPO}/issues/${n}`, status: 'shipped', filedAt: Date.now(), ...more });
    await addRequests([shipped(43, 'Hatch fill', { steps: ['hatch_fill'] }), shipped(44, 'Open rim on circle', { steps: ['circle'], version: 'ccc3333' })]);
    await page.reload();
    await page.waitForFunction(() => /Engine ready/.test(document.getElementById('status').textContent), null, { timeout: 30000 });
    await page.click('#tab-chat');
    await page.waitForSelector('#chatRequests li.ready:has-text("Hatch fill") button:has-text("Reload and carry on")', { timeout: 15000 });
    await page.waitForSelector('#chatRequests li.ready:has-text("Open rim on circle") button:has-text("Reload and carry on")', { timeout: 15000 });
    check(state.messages.length === before + 1, 'the chat carried on without the new step');
    check(githubCalls.includes(`/repos/${REPO}/compare/ccc3333...bbb2222`), "the app didn't ask whether it has the step's version");

    // Done, but the project's template was improved too (here: the stored project remembers an
    // older template): the chat waits for the user's answer, then carries on.
    await page.evaluate(() => {
      const p = JSON.parse(localStorage.getItem('easycad.project'));
      p.template = { ...p.template, base: 'an older version' };
      localStorage.setItem('easycad.project', JSON.stringify(p));
    });
    await addRequests([shipped(45, 'Wider base', { steps: ['circle'], version: 'aaa1111' })]);
    const waiting = state.messages.length;
    await page.reload();
    await page.waitForFunction(() => /Engine ready/.test(document.getElementById('status').textContent), null, { timeout: 30000 });
    await page.click('#tab-chat');
    await page.waitForSelector('#chatRequests li:has-text("Wider base") button:has-text("Keep mine")', { timeout: 15000 });
    check(!(await page.$eval('#templateUpdate', (e) => e.hidden)), 'no template update offered in Settings');
    await page.waitForTimeout(1000);
    check(state.messages.length === waiting, 'the chat carried on before the user answered the template update');
    if (shots) await page.screenshot({ path: path.join(shots, 'request-template.png') });
    await page.click('#chatRequests button:has-text("Keep mine")');
    await page.waitForFunction(() => (document.getElementById('chatLog').textContent.match(/Carrying on/g) || []).length === 2, null, { timeout: 30000 });
    const kept = state.messages[waiting]?.messages.at(-1).content.map((c) => c.text || '').join(' ');
    check(/feature request #45/.test(kept) && /kept their own recipe/.test(kept), `the model wasn't told the user kept their recipe: ${kept}`);
    check(await page.$eval('#templateUpdate', (e) => e.hidden), 'the template offer is still shown after "Keep mine"');

    // Without a proxy that can file requests, the app offers GitHub's page, filled in.
    state.canFile = false;
    await page.fill('#chatText', 'A second thing it cannot do.');
    await page.click('#chatSend');
    await page.waitForSelector('#chatRequests li:has-text("Knurled grip pattern") button:has-text("Send to be built")', { timeout: 30000 });
    await page.waitForFunction(() => !document.getElementById('chatStop').checkVisibility(), null, { timeout: 30000 });
    await page.click('#chatRequests li:has-text("Knurled grip pattern") button:has-text("Send to be built")');
    await page.waitForSelector('#chatRequests a:has-text("File it on GitHub")', { timeout: 30000 });
    const link = await page.getAttribute('#chatRequests a:has-text("File it on GitHub")', 'href');
    check(String(link).startsWith(`https://github.com/${REPO}/issues/new?`) && /labels=feature-request/.test(String(link)) && /Knurled\+grip\+pattern/.test(String(link)), `bad GitHub link: ${link}`);
    const overflow = await page.evaluate(() => document.documentElement.scrollWidth > window.innerWidth);
    check(!overflow, 'page scrolls sideways');
    if (shots) await page.screenshot({ path: path.join(shots, 'request-unfiled.png') });

    // "Revise my request": the proposal goes, nothing is filed, and the model hears why.
    const filed = state.filed.length;
    await page.fill('#chatText', 'A third thing it cannot do.');
    await page.click('#chatSend');
    await page.waitForSelector('#chatRequests li:has-text("Brim ears") button:has-text("Revise my request")', { timeout: 30000 });
    await page.waitForFunction(() => !document.getElementById('chatStop').checkVisibility(), null, { timeout: 30000 });
    await page.click('#chatRequests li:has-text("Brim ears") button:has-text("Revise my request")');
    check(!(await page.isVisible('#chatRequests li:has-text("Brim ears")')), 'the revised proposal is still shown');
    const asked = state.messages.length;
    await page.fill('#chatText', 'Make it simpler instead.');
    await page.click('#chatSend');
    await page.waitForFunction(() => /Fine, a simpler version/.test(document.getElementById('chatLog').textContent), null, { timeout: 30000 });
    const revised = state.messages[asked]?.messages.at(-1).content.map((c) => c.text || '').join(' ');
    check(/didn't send the feature request "Brim ears"/.test(revised), `the model wasn't told the user revised: ${revised}`);
    check(state.filed.length === filed, 'the revised request was filed');
    console.log(`filed ${state.filed.length} requests, ${state.messages.length} model requests, GitHub checked ${githubCalls.length} times`);
    await ctx.close();
  } catch (err) {
    failures.push(String(err && err.stack ? err.stack : err));
    failures.push(`requests filed: ${state.filed.length}, model requests: ${state.messages.length}`);
  } finally {
    if (browser) await browser.close();
    web.kill();
    mock.server.close();
  }
  if (failures.length) {
    console.error(failures.join('\n'));
    process.exit(1);
  }
  console.log('request test passed');
})();
