// Shared by the browser tests (smoke-test, agent-test, request-test): the web server, and the
// stand-in for the AI proxy that agent-test and request-test talk to instead of the real one.

const http = require('node:http');

/** Waits until `url` answers (the python web server takes a moment to start). */
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

/** One server-sent event of the Messages API stream. */
const sseEvent = (type, data) => `event: ${type}\ndata: ${JSON.stringify({ type, ...data })}\n\n`;

/**
 * The stand-in proxy: answers the browser's CORS preflight, refuses a wrong password as the real
 * proxy does, answers `/check`, and hands every other request to `route` with its path and body.
 * @param {string} password
 * @param {(req: { path: string, body: string, headers: object, cors: object }, res: http.ServerResponse) => void} route
 * @returns {Promise<{ server: http.Server, port: number }>}
 */
function startStandIn(password, route) {
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
    // The body's bytes, decoded once at the end: decoding chunk by chunk garbles a character
    // split across two chunks (the "·" in a tool result), and the test then sees a rewrite.
    const chunks = [];
    req.on('data', (c) => chunks.push(c));
    req.on('end', () => {
      const body = Buffer.concat(chunks).toString('utf8');
      const path = new URL(req.url, 'http://mock').pathname;
      if (req.headers.authorization !== `Bearer ${password}`) {
        res.writeHead(401, { ...cors, 'content-type': 'application/json' });
        res.end('{"type":"error","error":{"type":"authentication_error","message":"wrong password"}}');
      } else if (path === '/check') {
        res.writeHead(200, { ...cors, 'content-type': 'application/json' });
        res.end('{"ok":true}');
      } else route({ path, body, headers: req.headers, cors }, res);
    });
  });
  return new Promise((resolve) => server.listen(0, '127.0.0.1', () => resolve({ server, port: server.address().port })));
}

module.exports = { sseEvent, startStandIn, waitForServer };
