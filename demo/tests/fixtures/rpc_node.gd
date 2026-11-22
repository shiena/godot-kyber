# SPDX-FileCopyrightText: 2026 KOGA Mitsuhiro
# SPDX-License-Identifier: MIT

extends Node

var messages: Array = []


@rpc("any_peer", "call_remote", "reliable")
func receive_message(message: String) -> void:
	messages.append({"sender": multiplayer.get_remote_sender_id(), "text": message})
