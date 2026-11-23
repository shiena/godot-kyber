<!-- SPDX-FileCopyrightText: 2026 KOGA Mitsuhiro -->
<!-- SPDX-License-Identifier: MIT -->

# Install the addon

## Choose a package

Download a ZIP from [GitHub Releases](https://github.com/shiena/godot-kyber/releases/latest).

| Package | Use it to |
| --- | --- |
| Addon-only ZIP | Add streaming to an existing Godot project |
| Demo ZIP | Try the included sender, receiver, and RPC scenes |

The addon supports Godot 4.2 and later 4.x releases. The full demo is tested with Godot 4.7.2. Use the latter for the walkthroughs on this site.

## Copy the addon

1. Extract the addon-only ZIP.
2. Copy the complete `addons/kyber` directory into your project.
3. Keep the `bin/` and `licenses/` directories together with `kyber.gdextension`.
4. Open the project in Godot, or restart the editor if it is already running.

Godot loads the GDExtension automatically. There is no editor plugin to enable.

<!-- capture: addon-files-en | Godot FileSystem dock showing res://addons/kyber with bin, licenses, icon.svg, and kyber.gdextension. -->

## Check the installation

Check the editor output for extension-loading errors. In the script editor, check that `KyberSession` is recognized. Then follow [Get started](getting-started.md) to test a complete connection.

If the extension fails to load, check the binary for your OS and CPU architecture. Copying only the `.gdextension` file is not enough.

## Prepare the target platform

Windows, macOS, Android, and iOS use system media frameworks. Linux requires GStreamer and codec plugins. See [platform support](../../docs/overview.md) for limitations and the [build guide](../../docs/development.md) for platform tools, mobile exports, and signing.

The Kyber SDK is embedded. You don't need to install it separately or configure its library path. Web exports are not supported.

## Update an existing installation

Stop the running project and close the editor before replacing native libraries. Back up local modifications, then replace the addon directory with the complete package from one release. Use compatible addon versions on the sender and receiver; see [transport compatibility](../../docs/audio.md).

Before distributing a project, read the [binary licensing notice](../../BINARY_LICENSE.md).
