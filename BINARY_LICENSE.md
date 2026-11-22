<!-- SPDX-FileCopyrightText: 2026 KOGA Mitsuhiro -->
<!-- SPDX-License-Identifier: MIT -->

# Native addon licensing

The Kyber components included in this addon may be used under either:

- **AGPL-3.0-or-later**, with its applicable obligations; or
- **a valid commercial license agreement with Kyber SAS** that covers the
  included Kyber versions, modifications, and your use and distribution of
  this addon.

The public build is distributed under the AGPL option. A commercial agreement
may provide an alternative for the covered Kyber components. This project
does not grant or sublicense Kyber commercial rights. Obtain the agreement
from [Kyber SAS](https://kyber.tech/); its terms determine the permissions.
The upstream licensing summary is not a commercial license agreement.

Original godot-kyber code remains MIT-licensed under
`licenses/godot-kyber/LICENSE` in the addon. Other
dependencies retain their own licenses under `licenses/`. Neither the MIT
license nor a Kyber commercial agreement replaces those third-party terms.
The MIT SPDX identifier at the top of this document applies to the document,
not to the combined native binary.

For the AGPL option, distributors must provide the complete Corresponding
Source, including the matching SDK sources, modifications, and build scripts.
Including license texts alone does not satisfy that requirement. See
`licenses/godot-kyber/LICENSES/AGPL-3.0-or-later.txt` and
`licenses/godot-kyber/NOTICE.md` in the addon.

Platform build artifacts include the SDK's unmodified `COPYING.md` and
`COPYING.AGPLv3` in `licenses/kyber-sdk/`. The combined release keeps these
under `licenses/<platform>/kyber-sdk/` for each platform. `COPYING.md` uses
the upstream `AGPL-3.0` shorthand; the extracted SDK source headers specify
`AGPL-3.0-or-later`. Both upstream notices are preserved without rewriting.
