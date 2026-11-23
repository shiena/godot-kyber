<!-- SPDX-FileCopyrightText: 2026 KOGA Mitsuhiro -->
<!-- SPDX-License-Identifier: MIT -->

# GodotのマルチプレイヤーRPC

[ドキュメント一覧](README_ja.md) | [English](multiplayer.md)

コマンドはリポジトリのルートで実行します。コード例のパスはルートからの相対パスです。

## GodotのマルチプレイヤーRPCを使う

`KyberMultiplayerPeer`は`MultiplayerPeerExtension`を実装しています。`multiplayer.multiplayer_peer`へ設定すると、Godot標準の`SceneMultiplayer`、`@rpc`、接続シグナルを使えます。ポーリングはGodotが自動で行います。パケットピアとして直接使う場合は、自分で`poll()`を呼び出してください。

現在はサーバー1台（ピアID `1`）とクライアント1台（ピアID `2`）に対応しています。QUIC上のKyProtoのReliableデータエンドポイントを使い、証明書の照合とトークン認証を行います。FFmpegや外部SDKランタイムは不要です。

### RPCデモを試す

1. デモを2つ起動し、両方で［Godot RPC］タブを開きます。
2. 片方で［Start server］を押します。同じPCでは`127.0.0.1`、ポート`4434`を使用します。
3. サーバーの接続コードをもう片方へコピーします。
4. 同じアドレスとポートを指定し、もう片方で［Connect］を押します。
5. どちらかでメッセージを入力し、［Send via RPC］を押します。

メッセージはGodotのRPCを通して届きます。実装例は`demo/multiplayer_demo.gd`を参照してください。現在、RPCと映像は別の接続と接続コードを使用します。

<!-- capture: rpc-sender | RPCサーバー側の送受信結果 -->

<!-- capture: rpc-receiver | RPCクライアント側の送受信結果 -->

### 結果を確認する

両方のインスタンスから異なるメッセージを送信します。相手側にそれぞれのメッセージが表示され、状態表示にピアが示されていることを確認します。これでRPC接続の双方向を確認できます。

デモでは動作して独自のシーンでは動かない場合、両側のノードパスと`@rpc`宣言を比較します。映像の接続だけではRPCの接続は確立されません。

### シーンに組み込む

両側でノードのパスとRPC宣言をそろえます。接続中はピアへの参照を保持してください。

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

開始処理は非同期です。`OK`は要求の受付を意味します。サーバーの待受開始後に`get_connection_code()`からコードを取得します。接続イベントは`multiplayer`の`connected_to_server`、`connection_failed`、`server_disconnected`、`peer_connected`、`peer_disconnected`で受け取れます。RPCの送信は相手の接続後に行ってください。通信エラーは`peer.get_last_error()`で確認できます。

| 項目 | 現在の対応範囲 |
|---|---|
| 転送モード | `TRANSFER_MODE_RELIABLE`のみ。他のモードは`ERR_UNAVAILABLE`を返します。RPCには`"reliable"`を指定してください。既定のRPCモードは未対応です。 |
| チャネル | `0`のみ。他のチャネルは`ERR_UNAVAILABLE`を返します。 |
| パケットサイズ | Godotのプロトコル情報を含め、最大`KyberMultiplayerPeer.MAX_PACKET_SIZE`（65,536バイト）。 |
| キュー | 上限付きキューで送受信量を制御します。送信キューが満杯の場合は`ERR_OUT_OF_MEMORY`を返します。 |
| 送信先 | 相手のピアID、全員宛ての`0`、指定ピアを除外する負のID。 |
| 終了 | `close()`はローカルの切断シグナルを出さずに状態を消去します。`disconnect_peer(id, force)`は`force`の指定に従います。 |
| 再接続 | サーバーを開始し直し、新しいコードを使用します。複数クライアントと中継は未対応です。 |

Godot同士の通信を対象としており、既存Kyberホストへの接続用ではありません。両側で同じGodotバージョンを使用してください。`MultiplayerSpawner`と`MultiplayerSynchronizer`の統合は未検証で、非信頼型の状態同期は未対応です。Godot 4.2.2と4.7.2で、それぞれ別プロセス間のRPC往復を確認しています。
