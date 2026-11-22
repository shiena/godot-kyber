# SPDX-FileCopyrightText: 2026 KOGA Mitsuhiro
# SPDX-License-Identifier: MIT

extends GutTest

var server: KyberMultiplayerPeer
var client: KyberMultiplayerPeer
var port: int
var roots: Array[Node] = []


func before_each() -> void:
	server = KyberMultiplayerPeer.new()
	client = KyberMultiplayerPeer.new()
	watch_signals(server)
	watch_signals(client)
	var socket := PacketPeerUDP.new()
	assert_eq(socket.bind(0, "127.0.0.1"), OK)
	port = socket.get_local_port()
	socket.close()


func after_each() -> void:
	for node in roots:
		node.multiplayer.multiplayer_peer = null
		get_tree().set_multiplayer(null, node.get_path())
	roots.clear()
	server.close()
	client.close()


func _poll() -> void:
	server.poll()
	client.poll()


func _listen() -> bool:
	assert_eq(server.create_server(port, "127.0.0.1"), OK)
	var ready: bool = await wait_until(func():
		_poll()
		return not server.get_connection_code().is_empty(), 5.0)
	assert_true(ready, server.get_last_error())
	return ready


func _connect() -> bool:
	if not await _listen():
		return false
	assert_eq(client.create_client("127.0.0.1", port, server.get_connection_code()), OK)
	var ready: bool = await wait_until(func():
		_poll()
		return get_signal_emit_count(server, "peer_connected") == 1 and get_signal_emit_count(client, "peer_connected") == 1, 5.0)
	assert_true(ready, server.get_last_error() + " / " + client.get_last_error())
	return ready


func test_packets_ids_modes_and_target_routing() -> void:
	if not await _connect():
		return
	assert_eq(server.get_unique_id(), 1)
	assert_eq(client.get_unique_id(), 2)
	assert_eq(server.get_connection_status(), MultiplayerPeer.CONNECTION_CONNECTED)
	assert_signal_emitted_with_parameters(server, "peer_connected", [2])
	assert_signal_emitted_with_parameters(client, "peer_connected", [1])
	client.set_target_peer(1)
	assert_eq(client.put_packet("hello".to_utf8_buffer()), OK)
	assert_true(await wait_until(func():
		_poll()
		return server.get_available_packet_count() > 0, 5.0))
	assert_eq(server.get_packet_peer(), 2)
	assert_eq(server.get_packet_channel(), 0)
	assert_eq(server.get_packet_mode(), MultiplayerPeer.TRANSFER_MODE_RELIABLE)
	assert_eq(server.get_packet().get_string_from_utf8(), "hello")
	client.transfer_mode = MultiplayerPeer.TRANSFER_MODE_UNRELIABLE
	assert_eq(client.put_packet(PackedByteArray([1])), ERR_UNAVAILABLE)
	client.transfer_mode = MultiplayerPeer.TRANSFER_MODE_RELIABLE
	client.transfer_channel = 1
	assert_eq(client.put_packet(PackedByteArray([1])), ERR_UNAVAILABLE)
	client.transfer_channel = 0
	client.set_target_peer(99)
	assert_eq(client.put_packet(PackedByteArray([1])), ERR_INVALID_PARAMETER)
	client.set_target_peer(-1)
	assert_eq(client.put_packet("excluded".to_utf8_buffer()), OK)
	client.set_target_peer(-99)
	assert_eq(client.put_packet("included".to_utf8_buffer()), OK)
	assert_true(await wait_until(func():
		_poll()
		return server.get_available_packet_count() > 0, 5.0))
	assert_eq(server.get_packet().get_string_from_utf8(), "included")
	var oversized := PackedByteArray()
	oversized.resize(KyberMultiplayerPeer.MAX_PACKET_SIZE + 1)
	client.set_target_peer(1)
	assert_eq(client.put_packet(oversized), ERR_INVALID_PARAMETER)


func _rpc_root(label: String, peer: MultiplayerPeer) -> Node:
	var node = Node.new()
	node.set_script(load("res://tests/fixtures/rpc_node.gd"))
	node.name = label
	add_child_autofree(node)
	var api := SceneMultiplayer.new()
	get_tree().set_multiplayer(api, node.get_path())
	api.multiplayer_peer = peer
	roots.append(node)
	return node


func test_scene_multiplayer_rpc_round_trip() -> void:
	if not await _listen():
		return
	var server_root = _rpc_root("Server", server)
	assert_eq(client.create_client("127.0.0.1", port, server.get_connection_code()), OK)
	var client_root = _rpc_root("Client", client)
	assert_true(await wait_until(func(): return server_root.multiplayer.get_peers().size() == 1 and client_root.multiplayer.get_peers().size() == 1, 5.0))
	assert_eq(client_root.rpc_id(1, "receive_message", "Godot RPC over KyProto"), OK)
	assert_true(await wait_until(func(): return server_root.messages.size() == 1, 5.0))
	assert_eq(server_root.messages, [{"sender": 2, "text": "Godot RPC over KyProto"}])
	assert_eq(server_root.rpc_id(2, "receive_message", "reply"), OK)
	assert_true(await wait_until(func(): return client_root.messages.size() == 1, 5.0))
	assert_eq(client_root.messages, [{"sender": 1, "text": "reply"}])
	watch_signals(client_root.multiplayer)
	server.close()
	assert_true(await wait_until(func(): return client.get_connection_status() == MultiplayerPeer.CONNECTION_DISCONNECTED, 5.0))
	assert_signal_emitted(client_root.multiplayer, "server_disconnected")


func test_bad_token_and_refused_connections_never_join() -> void:
	if not await _listen():
		return
	var parts := server.get_connection_code().split(":")
	assert_eq(client.create_client("127.0.0.1", port, parts[0] + ":" + "0".repeat(64)), OK)
	assert_true(await wait_until(func():
		_poll()
		return client.get_connection_status() == MultiplayerPeer.CONNECTION_DISCONNECTED, 7.0))
	assert_signal_not_emitted(server, "peer_connected")
	server.refuse_new_connections = true
	assert_true(server.refuse_new_connections)
	assert_eq(client.create_client("127.0.0.1", port, server.get_connection_code()), OK)
	assert_true(await wait_until(func():
		_poll()
		return client.get_connection_status() == MultiplayerPeer.CONNECTION_DISCONNECTED, 7.0))
	assert_signal_not_emitted(server, "peer_connected")
	server.refuse_new_connections = false
	assert_eq(client.create_client("127.0.0.1", port, server.get_connection_code()), OK)
	assert_true(await wait_until(func():
		_poll()
		return client.get_connection_status() == MultiplayerPeer.CONNECTION_CONNECTED, 5.0))


func test_close_is_idempotent_and_invalid_config_is_rejected() -> void:
	assert_eq(server.create_server(0, "127.0.0.1"), ERR_INVALID_PARAMETER)
	assert_eq(server.create_server(port, "invalid"), ERR_INVALID_PARAMETER)
	assert_eq(client.create_client("127.0.0.1", port, "bad code"), ERR_CANT_CREATE)
	assert_eq(client.put_packet(PackedByteArray([1])), ERR_UNCONFIGURED)
	if not await _connect():
		return
	server.disconnect_peer(2, false)
	assert_signal_emitted_with_parameters(server, "peer_disconnected", [2])
	server.close()
	server.close()
	assert_eq(server.get_connection_status(), MultiplayerPeer.CONNECTION_DISCONNECTED)
	assert_eq(server.get_available_packet_count(), 0)
