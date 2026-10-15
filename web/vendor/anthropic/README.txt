Anthropic TypeScript SDK 0.129.0 (npm "@anthropic-ai/sdk"), MIT, see LICENSE.
https://github.com/anthropics/anthropic-sdk-typescript

anthropic-sdk.min.mjs is the SDK bundled into one browser ES module, unmodified otherwise. It
includes three small dependencies (licenses in LICENSE-bundled.txt). Rebuild it with:

  npm install --ignore-scripts @anthropic-ai/sdk@0.129.0 esbuild@0.28.2
  echo "export { default, default as Anthropic } from '@anthropic-ai/sdk';" > entry.mjs
  npx esbuild entry.mjs --bundle --format=esm --platform=browser --target=es2022 \
    --minify --legal-comments=eof --outfile=anthropic-sdk.min.mjs

The app loads it only when the chat is used (web/js/agent.js).
