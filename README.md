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
- `content/`: website guides in English and Japanese.
- `content/guides.json`: bilingual guide summaries and related-page links.
- `assets/`: styles and browser interactions.
- `main:docs/`: existing guides, imported with internal links rewritten.
- `main:demo/addons/kyber/icon.svg`: the addon icon, imported with its notices.

The generator resolves the addon ref to one commit for each build.
`dist/build-info.json` records that revision. The landing page code example
assumes an already-connected `KyberSession`; the guides cover connection setup.

This version includes the landing pages, introduction, and existing guides.
It does not provide a separate reference for every public method. Imported
guides default to the main branch's content. A matching filename in `content/`
overrides an imported guide for website-specific editing; its relative links
still resolve from the original `docs/` path. Video, audio, multiplayer, capture/input, existing-host, and H.264
guides currently use these overrides. Keep API examples aligned with `main`.

Use `<!-- screenshot: Description of the required capture. -->` in a guide
to reserve a labeled screenshot area. Replace the marker with a real image
and descriptive alternative text when the capture is available.

All 14 guides are available in English and Japanese. The Japanese licensing
guide translates the binary notice and links to its original text. Keep that
translation aligned with `main:BINARY_LICENSE.md`.

## Validate the GitHub Pages path

```powershell
./.venv/Scripts/python scripts/build.py --source-ref main --base-path /godot-kyber/ --output .preview/production
./.venv/Scripts/python scripts/check.py --output .preview/production
node --check assets/site.js
```

This branch contains source rather than generated HTML. Deployment should
build `dist/` and upload it through GitHub Pages Actions. Do not select legacy
branch publishing for the source directory. The `Deploy documentation` workflow builds and validates the site on each
push to `gh-pages`, then publishes `dist/` through GitHub Pages Actions.
It fetches `origin/main` for the addon sources recorded in `build-info.json`.
Generated output and local preview files are ignored.

## Demo video

Place the composite recording at `assets/demo.webm` and rebuild. Both home
pages replace the placeholder with a video player. Playback uses native controls, loops, and supports inline mobile playback.
Autoplay is disabled. The current recording has no audio track.
The player reserves a 16:9 area; export the recording at that aspect ratio.
Without this file, the build keeps the labeled placeholder.

Original site code is MIT-licensed. The addon icon retains the third-party
notices imported from `main` and displayed on the Credits page.

The current hero recording shows macOS sending a Godot Viewport to Windows.
It includes each OS window frame, with top padding on Windows to align the
content below the title bars. The output is a silent, 12-second, 1280×720
WebM at 30 fps. This is an edited demonstration, not a latency benchmark.

Use `<!-- capture: name | Caption. -->` to insert
`assets/screenshots/name.png`. The builder reads the PNG dimensions and
preserves its aspect ratio. Editor captures have separate `-en` and `-ja`
files. Demo captures share the application's English UI.
