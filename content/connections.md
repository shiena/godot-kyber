<!-- SPDX-FileCopyrightText: 2026 KOGA Mitsuhiro -->
<!-- SPDX-License-Identifier: MIT -->

# Connections and sessions

## Choose a connection type

| Remote application | Connection information | Entry point |
| --- | --- | --- |
| Addon file or Viewport sender | Address, port, connection code | `KyberSession.start_receiver()` |
| Existing Kyber host | Address, port, Basic credentials, certificate settings | `KyberSession.connect_to_host()` or `KyberClient` |
| Godot RPC peer | Address, port, RPC connection code | `KyberMultiplayerPeer` |

These connections are not interchangeable. A file sender is not a complete Kyber host. Video and RPC demos use separate connection codes, even when they run on the same computer.

## Connect on one computer or a LAN

For a local test, use `127.0.0.1` and the same port at both ends. For separate devices, bind the sender to `0.0.0.0` and connect to its LAN address. `0.0.0.0` is a listening address, not the receiver's destination.

Allow the selected UDP port through the firewall. Copy the connection code after the sender starts. Starting a new sending session generates a new code; replace the old code on every receiver.

These steps describe a direct LAN connection. They do not configure internet routing or a discovery service.

## Share a session between scene nodes

Use `KyberSession` when playback and connection controls belong to different parts of the scene. Attach it to `KyberVideoStream` with `set_session()` before assigning the resource to a `VideoStreamPlayer`.

The session owns the connection. The player controls presentation. Pausing a player freezes its image while the session continues receiving frames and updating status. Stopping a player does not close a shared session.

Video-only sessions can serve multiple players with separate pause controls. For an audio session, use one playback consumer and share its texture when displaying the image in multiple places. See the [video guide](../../docs/video.md) and [audio guide](../../docs/audio.md) for complete integration examples.

## Distinguish request acceptance from completion

A start method returning `true` means the request was accepted. Use `get_state()`, `get_status()`, and `get_last_error()` to follow the result. Session signals such as `state_changed` and `error_occurred` let the UI react without treating playback as connection status.

| Observation | Interpretation |
| --- | --- |
| Player paused | Presentation is paused; the shared connection may still be active |
| `FINISHED` | Normal file-transfer completion |
| `FAILED` | Inspect the retained error and status |
| `RECONNECTING` | The SDK host reconnection path is active |

## Reconnect and stop

For an existing host, set `session.auto_reconnect = true` before `connect_to_host()`. This uses Kyber SDK reconnection. File/Viewport and RPC connections do not use that automatic retry path.

A file/Viewport receiver can join again while the sender remains active, using the current connection code. After the sender restarts, obtain the new code. For the RPC demo, restart the server and reconnect with its new code.

Call `session.close()` when the shared connection is no longer needed. It disconnects all consumers. Removing the session from the scene tree also closes it. If only one display should pause, control its player instead of closing the session.
