<!-- SPDX-FileCopyrightText: 2026 KOGA Mitsuhiro -->
<!-- SPDX-License-Identifier: MIT -->

# godot-kyber documentation site

This orphan `gh-pages` branch contains the site source. Keep this worktree
separate from the addon on `main`.

## Preview locally

Use Python 3.11 or later:

```powershell
python -m venv .venv
./.venv/Scripts/python -m pip install -r requirements.txt
./.venv/Scripts/python scripts/build.py --source-ref main --base-path /
./.venv/Scripts/python scripts/check.py
./.venv/Scripts/python -m http.server 8765 --bind 127.0.0.1 --directory dist
```

On Linux or macOS, use `.venv/bin/python`. Open `http://127.0.0.1:8765/` for
English or `http://127.0.0.1:8765/ja/` for Japanese.

## Content

- `scripts/build.py`: shared layout, landing pages, sidebar, and source mapping.
- `content/`: introduction pages in English and Japanese.
- `assets/`: styles and browser interactions.
- `main:docs/`: existing guides, imported with internal links rewritten.
- `main:demo/addons/kyber/icon.svg`: the addon icon, imported with its notices.

The generator resolves the addon ref to one commit for each build.
`dist/build-info.json` records that revision. The landing page code example
assumes an already-connected `KyberSession`; the guides cover connection setup.

This version includes the landing pages, introduction, and existing guides.
It does not provide a separate reference for every public method. Imported
guides retain the main branch's content. Corrections to those guides belong
on `main`.

SDK internals are currently in Japanese, and binary licensing is in English.
The corresponding pages in the other language show a language notice.

## Validate the GitHub Pages path

```powershell
./.venv/Scripts/python scripts/build.py --source-ref main --base-path /godot-kyber/ --output .preview/production
./.venv/Scripts/python scripts/check.py --output .preview/production
node --check assets/site.js
```

This branch contains source rather than generated HTML. Deployment should
build `dist/` and upload it through GitHub Pages Actions. Do not select legacy
branch publishing for the source directory. Deployment is not configured yet.
Generated output and local preview files are ignored.

Original site code is MIT-licensed. The addon icon retains the third-party
notices imported from `main` and displayed on the Credits page.
