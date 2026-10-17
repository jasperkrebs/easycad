// End-to-end test of the in-app agent (Chat tab) against a scripted stand-in for the Claude API
// and the proxy, so it costs nothing and gives the same answers every time.
//
//   node scripts/agent-test.cjs [screenshot-dir]
//
// The stand-in speaks the proxy's protocol (password as a bearer token, /check) and the Messages
// API's streaming format. It plays one conversation: the model writes a recipe, looks at it,
// edits it, and answers. The test checks what the app sends (model, caching, tools, password,
// an append-only history, a rendered picture) and what it does (recipe changed, undo, chat
// saved across a reload, a wrong password explained). Needs Playwright and a built engine,
// like scripts/smoke-test.cjs.

const { chromium } = require('playwright');
const { spawn } = require('node:child_process');
const http = require('node:http');
const path = require('node:path');

const root = path.join(__dirname, '..', 'web');
const webPort = 8000 + Math.floor(Math.random() * 500);
const shots = process.argv[2];
const PASSWORD = 'correct horse battery staple';

const CYLINDER = `easycad 0.1
project "Test cylinder"
printer "Bambu P1P 0.4"
note "A plain cylinder for the agent test."

group "Shape"
param height = 80 mm in 20..200 "Height"
param radius = 30 mm in 10..80 "Radius"

group "Print"
param layer = 0.3 mm in 0.1..1 "Layer height"
param line = 0.6 mm in 0.3..1.2 "Line width"

base = circle(radius)
wall = loft([base, base |> move(z: height)])
path = vase_spiral(wall, layer: layer, width: line)

show wall
print path
`;

/** The scripted conversation: what the model "says" at each round. */
function reply(round) {
  if (round === 0) {
    return {
      thinking: 'A cylinder: one circle lofted up, printed in vase mode.',
      text: "I'll write a cylinder recipe with height and radius as settings.",
      tool: { name: 'write_recipe', input: { recipe: CYLINDER } },
    };
  }
  if (round === 1) return { tool: { name: 'look', input: { show: 'both' } } };
  if (round === 2) return { tool: { name: 'edit_recipe', input: { edits: [{ old: 'param height = 80 mm', new: 'param height = 120 mm' }] } } };
  return { text: 'Done: a 120 mm cylinder. Try **Radius** in the settings.' };
}

/** One answer as server-sent events, split into small pieces like the real stream. */
function sse(round, model) {
  const r = reply(round);
  const events = [];
  const send = (type, data) => events.push(`event: ${type}\ndata: ${JSON.stringify({ type, ...data })}\n\n`);
  send('message_start', {
    message: {
      id: `msg_${round}`,
      type: 'message',
      role: 'assistant',
      model,
      content: [],
      stop_reason: null,
      stop_sequence: null,
      usage: { input_tokens: 40, cache_creation_input_tokens: round === 0 ? 9000 : 300, cache_read_input_tokens: round === 0 ? 0 : 9000, output_tokens: 1 },
    },
  });
  let index = 0;
  if (r.thinking) {
    send('content_block_start', { index, content_block: { type: 'thinking', thinking: '', signature: '' } });
    send('content_block_delta', { index, delta: { type: 'thinking_delta', thinking: r.thinking } });
    send('content_block_delta', { index, delta: { type: 'signature_delta', signature: `sig-${round}` } });
    send('content_block_stop', { index });
    index++;
  }
  if (r.text) {
    send('content_block_start', { index, content_block: { type: 'text', text: '' } });
    for (const piece of r.text.match(/.{1,12}/gs)) send('content_block_delta', { index, delta: { type: 'text_delta', text: piece } });
    send('content_block_stop', { index });
    index++;
  }
  if (r.tool) {
    send('content_block_start', { index, content_block: { type: 'tool_use', id: `toolu_${round}`, name: r.tool.name, input: {} } });
    const json = JSON.stringify(r.tool.input);
    for (const piece of json.match(/.{1,40}/gs)) send('content_block_delta', { index, delta: { type: 'input_json_delta', partial_json: piece } });
    send('content_block_stop', { index });
  }
  send('message_delta', { delta: { stop_reason: r.tool ? 'tool_use' : 'end_turn', stop_sequence: null }, usage: { output_tokens: 420 } });
  send('message_stop', {});
  return events;
}

/** The stand-in proxy + API. Records every request for the checks below. */
function startMock() {
  const requests = [];
  const server = http.createServer((req, res) => {
    const cors = {
      'Access-Control-Allow-Origin': req.headers.origin || '*',
      'Access-Control-Allow-Methods': 'GET, POST, OPTIONS',
      'Access-Control-Allow-Headers': req.headers['access-control-request-headers'] || '*',
      'Access-Control-Expose-Headers': 'request-id',
    };
    if (req.method === 'OPTIONS') {
      res.writeHead(204, cors);
      res.end();
      return;
    }
    let body = '';
    req.on('data', (c) => (body += c));
    req.on('end', () => {
      const authorized = req.headers.authorization === `Bearer ${PASSWORD}`;
      if (!authorized) {
        res.writeHead(401, { ...cors, 'content-type': 'application/json' });
        res.end(JSON.stringify({ type: 'error', error: { type: 'authentication_error', message: 'wrong password' } }));
        return;
      }
      if (new URL(req.url, 'http://mock').pathname === '/check') {
        res.writeHead(200, { ...cors, 'content-type': 'application/json' });
        res.end('{"ok":true}');
        return;
      }
      if (new URL(req.url, 'http://mock').pathname !== '/v1/messages') {
        res.writeHead(404, cors);
        res.end();
        return;
      }
      const json = JSON.parse(body);
      requests.push({ headers: req.headers, body: json });
      // Which round this is: how many tool results the conversation already holds.
      const round = json.messages.flatMap((m) => (Array.isArray(m.content) ? m.content : [])).filter((b) => b.type === 'tool_result').length;
      res.writeHead(200, { ...cors, 'content-type': 'text/event-stream', 'request-id': `req_${round}` });
      const events = sse(round, json.model);
      let i = 0;
      const tick = () => {
        if (i < events.length) {
          res.write(events[i++]);
          setTimeout(tick, 5);
        } else res.end();
      };
      tick();
    });
  });
  return new Promise((resolve) => server.listen(0, '127.0.0.1', () => resolve({ server, requests, port: server.address().port })));
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
  const mock = await startMock();
  const failures = [];
  const check = (ok, message) => ok || failures.push(message);
  let browser;
  try {
    const url = `http://127.0.0.1:${webPort}/index.html`;
    await waitForServer(url);
    browser = await chromium.launch();
    for (const [name, opts] of [
      ['desktop', { viewport: { width: 1366, height: 900 } }],
      ['phone', { viewport: { width: 390, height: 844 }, isMobile: true, hasTouch: true, deviceScaleFactor: 2 }],
    ]) {
      mock.requests.length = 0;
      const ctx = await browser.newContext(opts);
      const page = await ctx.newPage();
      page.on('pageerror', (e) => failures.push(`${name}: page error: ${e.message}`));
      page.on('console', (m) => {
        if (m.type() === 'error' && !/401|Unauthorized/.test(m.text())) failures.push(`${name}: console error: ${m.text()}`);
      });
      await page.goto(url);
      await page.waitForFunction(() => /Engine ready/.test(document.getElementById('status').textContent), null, { timeout: 30000 });
      const original = await page.inputValue('#recipe');

      // Connect: proxy address and password, checked without spending anything.
      await page.click('#tab-chat');
      if (!(await page.isVisible('#chatSetup'))) failures.push(`${name}: no setup hint before the AI is connected`);
      await page.click('#aiSettings summary');
      await page.fill('#aiProxyUrl', `http://127.0.0.1:${mock.port}`);
      await page.dispatchEvent('#aiProxyUrl', 'change');
      await page.fill('#aiSecret', 'wrong password');
      await page.click('#aiSave');
      await page.waitForFunction(() => /refused/.test(document.getElementById('aiSecretState').textContent), null, { timeout: 15000 });
      await page.fill('#aiSecret', PASSWORD);
      await page.click('#aiSave');
      await page.waitForFunction(() => /accepted/.test(document.getElementById('aiSecretState').textContent), null, { timeout: 15000 });
      check(!(await page.isVisible('#chatSetup')), `${name}: setup hint still shown after connecting`);

      // One message: the model writes, looks, edits and answers.
      await page.fill('#chatText', 'Make me a cylinder, 120 mm tall.');
      await page.click('#chatSend');
      await page.waitForFunction(() => /Done: a 120 mm cylinder/.test(document.getElementById('chatLog').textContent), null, { timeout: 30000 });
      await page.waitForFunction(() => !document.getElementById('chatStop').checkVisibility(), null, { timeout: 15000 });
      const recipe = await page.inputValue('#recipe');
      check(/param height = 120 mm/.test(recipe) && /project "Test cylinder"/.test(recipe), `${name}: the agent's recipe wasn't applied`);
      check((await page.textContent('#projectName')) === 'Test cylinder', `${name}: project name not updated`);
      const chips = await page.locator('#chatLog .tool').allTextContents();
      check(chips.length === 3, `${name}: expected 3 tool steps in the chat, got ${chips.length}: ${chips.join(' | ')}`);
      check((await page.locator('#chatLog .tool.ok').count()) === 3, `${name}: tool steps not marked as done`);
      check(/this chat \$0\.0[1-9]/.test(await page.textContent('#chatCost')), `${name}: cost not shown: ${await page.textContent('#chatCost')}`);
      if (shots) await page.screenshot({ path: path.join(shots, `agent-${name}-chat.png`), fullPage: name === 'phone' });

      // What the app sent.
      const reqs = mock.requests;
      check(reqs.length === 4, `${name}: expected 4 model requests, got ${reqs.length}`);
      const first = reqs[0];
      check(first.headers.authorization === `Bearer ${PASSWORD}` && !first.headers['x-api-key'], `${name}: the password must be the only credential sent`);
      check(String(first.headers['anthropic-beta']).includes('server-side-fallback-2026-07-01'), `${name}: fallback beta header missing`);
      const b = first.body;
      check(b.model === 'claude-sonnet-5-5' && b.output_config?.effort === 'medium' && b.stream === true, `${name}: model/effort/stream wrong`);
      check(b.thinking?.type === 'adaptive' && b.fallbacks === 'default', `${name}: thinking or fallbacks wrong`);
      check(b.system?.[0]?.cache_control?.type === 'ephemeral' && b.cache_control?.type === 'ephemeral', `${name}: caching not requested`);
      check(b.system[0].text.length > 4000 && /circle\(r: Length, tolerance: Length = 0\)/.test(b.system[0].text), `${name}: system prompt lacks the catalog`);
      const toolNames = b.tools.map((t) => t.name);
      check(JSON.stringify(toolNames) === JSON.stringify([...toolNames].sort()), `${name}: tools not in a fixed order`);
      check(b.tools.every((t) => t.input_schema), `${name}: a tool has no schema`);
      check(String(b.messages[0].content[0].text).startsWith('<project>'), `${name}: first message lacks the project context`);
      // The system prompt and tools are identical on every request (that's what makes caching work).
      check(reqs.every((r) => r.body.system[0].text === b.system[0].text && JSON.stringify(r.body.tools) === JSON.stringify(b.tools)), `${name}: system prompt or tools changed between requests`);
      // Append-only history: each request repeats the previous one's messages unchanged.
      for (let i = 1; i < reqs.length; i++) {
        const prev = JSON.stringify(reqs[i - 1].body.messages);
        const next = JSON.stringify(reqs[i].body.messages.slice(0, reqs[i - 1].body.messages.length));
        check(prev === next, `${name}: request ${i + 1} rewrote earlier messages`);
      }
      // The assistant's thinking (with its signature) is passed back unchanged.
      const back = reqs[1].body.messages[1].content.find((c) => c.type === 'thinking');
      check(back && back.signature === 'sig-0', `${name}: thinking block not passed back`);
      // The look tool returned a picture.
      const lookResult = reqs[2].body.messages.at(-1).content[0];
      const image = Array.isArray(lookResult.content) && lookResult.content.find((c) => c.type === 'image');
      check(image && image.source.media_type === 'image/jpeg' && image.source.data.length > 5000, `${name}: look didn't return a picture`);
      if (shots && image) require('node:fs').writeFileSync(path.join(shots, `agent-${name}-look.jpg`), Buffer.from(image.source.data, 'base64'));
      const writeResult = reqs[1].body.messages.at(-1).content[0];
      check(/No problems\./.test(String(writeResult.content)) && /Prints: \d+ layers/.test(String(writeResult.content)), `${name}: run result not reported back`);

      // Undo puts the recipe back as it was before the message.
      await page.click('#chatUndoButton');
      await page.waitForFunction((src) => document.getElementById('recipe').value === src, original, { timeout: 15000 });

      // The chat survives a reload (saved on the device).
      await page.reload();
      await page.waitForFunction(() => /Engine ready/.test(document.getElementById('status').textContent), null, { timeout: 30000 });
      await page.click('#tab-chat');
      await page.waitForFunction(() => /Done: a 120 mm cylinder/.test(document.getElementById('chatLog').textContent), null, { timeout: 15000 });

      // A wrong password is explained, not a crash.
      await page.click('#aiSettings summary').catch(() => {});
      await page.fill('#aiSecret', 'nope');
      await page.click('#aiSave');
      await page.waitForFunction(() => /refused/.test(document.getElementById('aiSecretState').textContent), null, { timeout: 15000 });
      await page.fill('#chatText', 'Make it taller.');
      await page.click('#chatSend');
      await page.waitForFunction(() => /refused the password/.test(document.getElementById('chatLog').textContent), null, { timeout: 30000 });
      const overflow = await page.evaluate(() => document.documentElement.scrollWidth > window.innerWidth);
      check(!overflow, `${name}: page scrolls sideways`);
      if (shots) await page.screenshot({ path: path.join(shots, `agent-${name}-error.png`), fullPage: name === 'phone' });
      console.log(`${name}: ${reqs.length} model requests, chips: ${chips.join(' | ')}`);
      await ctx.close();
    }
  } catch (err) {
    failures.push(String(err && err.stack ? err.stack : err));
  } finally {
    if (browser) await browser.close();
    web.kill();
    mock.server.close();
  }
  if (failures.length) {
    console.error(failures.join('\n'));
    process.exit(1);
  }
  console.log('agent test passed');
})();
