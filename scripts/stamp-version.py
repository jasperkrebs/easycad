#!/usr/bin/env python3
"""Stamps the deploy's version onto every script, stylesheet and wasm URL of the web app.

    python3 scripts/stamp-version.py web <version>

GitHub Pages serves each file under the same URL from one deploy to the next, and its CDN can
briefly hand out a new index.html with an old app.js (or an old dom.js, or old wasm under new
glue) while a deploy settles: a mix of two versions, which fails to start. With `?v=<version>`
on every URL that loads code, each deploy's files are new URLs, so a page only ever gets files
of one deploy. CI runs this on the built site just before deploying it (never on the sources).
It's idempotent: stamping twice changes nothing.
"""

import pathlib
import re
import sys


def stamp(text: str, version: str, patterns: list[str]) -> str:
    def add(m: re.Match) -> str:
        url = m.group("url")
        if "?" in url:
            return m.group(0)
        return m.group(0).replace(url, f"{url}?v={version}", 1)

    for pattern in patterns:
        text = re.sub(pattern, add, text)
    return text


def main() -> int:
    if len(sys.argv) != 3:
        print(__doc__)
        return 2
    web, version = pathlib.Path(sys.argv[1]), sys.argv[2]
    if not re.fullmatch(r"[A-Za-z0-9._-]{1,64}", version):
        print(f"not a version: {version!r}")
        return 2
    # Relative imports and module URLs in the app's own code and the wasm glue.
    code = [
        r"""from\s+'(?P<url>\.{1,2}/[^']+\.m?js)'""",
        r"""import\('(?P<url>\.{1,2}/[^']+\.m?js)'\)""",
        r"""new URL\('(?P<url>\./[^']+\.js)', import\.meta\.url\)""",
        r"""new URL\('(?P<url>[^'/]+\.wasm)', import\.meta\.url\)""",
    ]
    files = sorted((web / "js").glob("*.js")) + sorted((web / "pkg").glob("*.js"))
    for f in files:
        f.write_text(stamp(f.read_text(), version, code))
    page = web / "index.html"
    page.write_text(
        stamp(
            page.read_text(),
            version,
            [r'href="(?P<url>styles\.css)"', r'src="(?P<url>js/app\.js)"'],
        )
    )
    print(f"stamped {len(files) + 1} files with v={version}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
