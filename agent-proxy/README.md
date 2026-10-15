# EasyCAD AI proxy

A tiny Cloudflare Worker (`worker.js`, about 150 lines) that keeps your Claude API key off the
website. The app sends your password; the proxy checks it and forwards the request to the
Claude API with the key, streaming the answer back.

- **Secrets** live in Cloudflare, set by CI from GitHub repository secrets. They are never in
  this repo or in the website.
  - `ANTHROPIC_API_KEY`: the Claude API key.
  - `EASYCAD_PASSWORD`: the password the app asks for.
- **It only forwards what EasyCAD needs:**
  - the Messages API and token counting;
  - EasyCAD's three models (Sonnet 5.5, Opus 5.5, Haiku 5.5);
  - `max_tokens` up to 64,000, and requests up to 8 MB;
  - calls from this repo's GitHub Pages site and `localhost:8080` only.
- **A wrong password** gets a 401 after a short wait, so guessing is slow. Use a long one,
  e.g. 20+ random characters from a password manager.

Setup steps (Cloudflare account, the four GitHub secrets): [docs/api-key.md](../docs/api-key.md).
CI (`.github/workflows/ci.yml`, job "AI proxy") tests it, deploys it with Wrangler, sets the two
secrets and writes the proxy's address into the app's `config.json`.

```sh
node --test agent-proxy/worker.test.mjs   # tests, with the Claude API stubbed out
```
