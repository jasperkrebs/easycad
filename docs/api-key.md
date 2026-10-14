# Your Claude API key (for EasyCAD's built-in agent)

EasyCAD's agent talks to Claude directly from the app, using **your own API key**. No server and
no account with us; the key stays on your device. You only need this from Phase 2 (the agent).
Everything else in EasyCAD works without it.

## Get a key (about 5 minutes)

1. **Sign in to the Claude Console:** <https://platform.claude.com>.
   - This is separate from the Claude app and your Claude subscription. API usage is
     pay-as-you-go and **not included in Claude Pro/Max**.
   - Create an account if you don't have one.
2. **Add credits:** Billing settings, <https://platform.claude.com/settings/billing>.
   - Start small (e.g. $5–$20).
   - Leave auto-reload off at first, so spending can never run away.
   - Spend limits can also be set in the Console.
3. **Create the key:** API keys page, <https://platform.claude.com/settings/keys> →
   **Create key**.
   - Name: `EasyCAD`.
   - Expiration: pick one (e.g. 90 days; you can always make a new key).
   - Linked account: yourself.
4. **Copy the key right away.** It starts with `sk-ant-` and is **shown only once**. Put it in
   your password manager (e.g. iCloud Keychain / Passwords).
5. **In EasyCAD:** Settings → Agent → paste the key. *(This screen arrives with Phase 2.)*

## Using it safely
- **Never share the key or paste it into chats, files or the repo.** Anyone with it can spend
  your credits.
- **Where EasyCAD keeps it:**
  - **Web app:** stored only in that browser on that device, and sent only to Anthropic's API
    (`api.anthropic.com`).
  - **Desktop app (later):** stored in the system keychain.
- **If it ever leaks:** delete it on the keys page and create a new one. The old key stops
  working immediately.
- **Use one key per device** if you like. Each can be deleted on its own.

## What it costs (Oct 2026 prices)
- **Moving sliders, exporting, previews and G-code:** free. They never use the AI.
- **Default model, Claude Opus 5.5:** $4 per million input tokens, $20 per million output tokens.
- **Rough estimate:** a design request costs a few cents up to about $1, depending on how many
  render-and-fix rounds it takes.
- **Cheaper models** (Sonnet 5.5, Haiku 5.5) are a setting you can pick.
- The Console's Usage page shows exactly what you spent.

Official guide: <https://platform.claude.com/docs/en/get-api-key>
