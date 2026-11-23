<!-- SPDX-FileCopyrightText: 2026 KOGA Mitsuhiro -->
<!-- SPDX-License-Identifier: MIT -->

# Godot multiplayer RPC

[Documentation](README.md) | [日本語](multiplayer_ja.md)

Run commands from the repository root. Paths in code examples are relative to that root.

## Use Godot multiplayer RPC

`KyberMultiplayerPeer` implements `MultiplayerPeerExtension`. Assign it to `multiplayer.multiplayer_peer` to use Godot's `SceneMultiplayer`, `@rpc`, and connection signals. Godot polls the peer automatically. If you use it directly as a packet peer, call `poll()` yourself.

The current transport supports one server (peer ID `1`) and one client (peer ID `2`). It uses a KyProto reliable data endpoint over QUIC, with certificate pinning and token authentication. It doesn't require FFmpeg or an external SDK runtime.

### Try the RPC demo

1. Launch two instances of the demo and open the **Godot RPC** tab in both.
2. Click **Start server** in the first instance. For a local test, use `127.0.0.1` and port `4434`.
3. Copy the server's connection code into the second instance.
4. Click **Connect** in the second instance, using the same address and port.
5. Enter a message in either instance and click **Send via RPC**.

Messages travel through Godot's RPC system. See `demo/multiplayer_demo.gd` for the complete example. RPC and video currently use separate connections and connection codes.

<!-- capture: rpc-sender | Messages on the RPC server -->

<!-- capture: rpc-receiver | Messages on the RPC client -->

### Check the result

Send a different message from each instance. Confirm that the other instance displays each message and that the connection status shows a peer. This verifies both directions of the RPC connection.

If the demo works but your own scene does not, compare the node paths and `@rpc` declarations on both peers. A video connection alone does not establish the RPC connection.

### Integrate with your scene

Both peers need matching node paths and RPC declarations. Keep the peer referenced for the duration of the connection:

```gdscript
extends Node

var peer := KyberMultiplayerPeer.new()

func start_host() -> Error:
    var result := peer.create_server(4434, "127.0.0.1")
    if result == OK:
        multiplayer.multiplayer_peer = peer
    return result

func join_host(host: String, code: String) -> Error:
    var result := peer.create_client(host, 4434, code)
    if result == OK:
        multiplayer.multiplayer_peer = peer
    return result

@rpc("any_peer", "call_remote", "reliable")
func receive_message(text: String) -> void:
    print(multiplayer.get_remote_sender_id(), ": ", text)

func send_to_server(text: String) -> void:
    receive_message.rpc_id(1, text)
```

Creation is asynchronous: `OK` means the request was accepted. Read `get_connection_code()` after the server starts listening. Use `multiplayer.connected_to_server`, `connection_failed`, `server_disconnected`, `peer_connected`, and `peer_disconnected` for connection events. Only send RPCs after the remote peer connects. Read `peer.get_last_error()` for transport failures.

| Capability | Current behavior |
|---|---|
| Transfer modes | `TRANSFER_MODE_RELIABLE` only. Other modes return `ERR_UNAVAILABLE`. Declare RPCs with `"reliable"`; the default RPC mode is unsupported. |
| Channels | Channel `0` only. Other channels return `ERR_UNAVAILABLE`. |
| Packet size | Up to `KyberMultiplayerPeer.MAX_PACKET_SIZE` (65,536 bytes), including Godot protocol overhead. |
| Queues | Bounded queues apply backpressure. A full outgoing queue returns `ERR_OUT_OF_MEMORY`. |
| Targets | Remote peer ID, broadcast `0`, and negative IDs for exclusion. |
| Shutdown | `close()` clears local state without emitting local disconnection signals. `disconnect_peer(id, force)` respects the `force` flag. |
| Reconnection | Restart the server and use its new code. Multi-client hosting and relaying aren't supported. |

This is a Godot-to-Godot protocol, not an existing Kyber host client. Use the same Godot version on both sides. `MultiplayerSpawner` and `MultiplayerSynchronizer` aren't part of the verified integration; unreliable state synchronization isn't supported. RPC round trips were verified between separate processes on Godot 4.2.2 and 4.7.2.
