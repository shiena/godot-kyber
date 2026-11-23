<!-- SPDX-FileCopyrightText: 2026 KOGA Mitsuhiro -->
<!-- SPDX-License-Identifier: MIT -->

# License scope and third-party notices

Source-relative paths below describe the repository. In addon-only archives,
project license texts and a copy of this notice are under
`licenses/godot-kyber/`.

## Original project material

Original code, scripts, documentation, and synthetic test assets authored by
KOGA Mitsuhiro are available under the MIT license in `LICENSE` and
`LICENSES/MIT.txt`. File headers identify the applicable SPDX license.
`REUSE.toml` identifies original files whose formats cannot retain comments.
The MIT grant does not cover third-party material copied into this project.

## SDK-derived build output

All original project sources, including `sdk/native_player.rs` and the
extraction script, are MIT-licensed. The root and media Cargo packages declare
`MIT`. Third-party files retain their own licenses.

`scripts/prepare_sdk.py` reads player events, listener definitions, and window
handle types from the pinned SDK's `kyvlcplayer/src/player.rs` and
`window_handle.rs`. It verifies their Git blob IDs before extraction. These
sources are copyright 2022–2026 Kyber SAS, at KyCtl revision
`92c632f9e889917a34aaa7697fb88b4991054b1c`.

The script writes the combined player module only into the ignored
`.deps/kysdk/kyctl/kyclient/src/platform/desktop/player.rs` build tree. It
preserves upstream copyright and license notices and appends the original
MIT-licensed CPU adapter. The repository does not contain copied player type
definitions. Extraction does not relicense upstream code.

The fetched SDK and generated or patched SDK modules retain their applicable
upstream licenses. An MIT header identifies an adapter's original material,
not the combined SDK module or compiled library. The AGPL text in
`LICENSES/AGPL-3.0-or-later.txt` is retained for build and distribution notices.

## Native binaries and commercial licensing

See [native addon licensing](BINARY_LICENSE.md) for the AGPL and commercial
options. Build artifacts preserve the SDK root licensing summary and AGPL
text under `licenses/kyber-sdk/`; combined releases keep them under
`licenses/<platform>/kyber-sdk/`. The summary does not grant commercial rights.

Native binaries built with the public AGPL Kyber SDK are distributed as
combined works under AGPL-3.0-or-later. Preserve the MIT notices and all other
applicable dependency notices. Binary distributors must provide the complete
Corresponding Source under the AGPL, including the matching SDK sources,
modifications, and required build scripts. A binary archive and license texts
alone do not satisfy that source requirement. Network use may also trigger
AGPL section 13 obligations.

Kyber offers a separate commercial license. A licensee may use Kyber material
under that agreement only where it covers the relevant versions, modifications,
builds, and distribution. Obtaining a license does not automatically grant
source redistribution or sublicensing rights. Other dependencies retain their
own terms. This project does not grant or sublicense Kyber commercial rights
and does not label the combined library as MIT.

Official information: https://kyber.tech/ and the SDK's `COPYING.md`.
The applicable signed agreement determines commercial permissions.

## Addon icon

`demo/addons/kyber/icon.svg` combines the Godot Engine icon and Kyber mark.
The Godot icon is copyright 2017 Andrea Calabró, licensed under CC BY 4.0:
https://godotengine.org/press/ and https://godotengine.org/assets/press/icon_color_outline.svg.
It is scaled proportionally and placed at the lower right of the Kyber gradient icon. The background is
prerendered at 1024 pixels and embedded as a transparent PNG inside the SVG
to preserve the original filters in Godot; the foreground remains vector.
See `LICENSES/CC-BY-4.0.txt`.

The Kyber mark comes from https://gitlab.com/kyber/kyber/-/blob/main/static/img/kyber-icon-gradient.svg and retains its
owner's rights. Its use here does not grant an MIT license to that mark;
see `LICENSES/LicenseRef-Kyber-Logo.txt`. The original composition is MIT.
This is an unofficial integration icon, not an endorsement by either project.

## Other third-party material

`demo/addons/gut/` is a vendored copy of GUT. Its code retains the MIT license
and copyright notices in `demo/addons/gut/LICENSE.md` (Tom "Butch" Wesley).
Bundled fonts retain their original font licenses and embedded copyright
metadata; see `demo/addons/gut/fonts/OFL.txt`. This project's MIT grant does not
relicense these files.

Rust dependencies and system media components retain their respective licenses.
Native staging collects dependency notices under `addons/kyber/licenses/`.
Project license texts and this notice are included in that directory and in
release packages. The synthetic fixtures in `tests/media/` contain no
third-party media; their generation commands are documented there.
