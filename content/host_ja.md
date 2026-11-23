<!-- SPDX-FileCopyrightText: 2026 KOGA Mitsuhiro -->
<!-- SPDX-License-Identifier: MIT -->

# 既存Kyberホストとの接続

[ドキュメント一覧](README_ja.md) | [English](host.md)

アドオンを導入済みの状態から説明します。

## 接続方式を確認する

このページは、アドレスとBasic認証情報を使って既存のKyberホストに接続する手順です。アドオンのファイル・Viewport送信とは接続コードとプロトコルが異なります。そちらに接続する場合は[映像ガイド](video_ja.md)を参照してください。

［Local display demo］はネットワーク接続なしでテスト画像を表示します。表示の組み込みを確認できますが、ホスト認証や配信の検証にはなりません。


## ホストに接続する

Kyberホストのアドレス、Basic認証情報を用意します。自己署名証明書を使用する場合は、ホスト側で確認したSHA-256フィンガープリントを指定します。

1. ホスト名、ユーザー名、パスワードを入力します。デモの接続ポートは8080です。
2. 自己署名証明書の場合は、フィンガープリントを入力します。
3. ［Connect］をクリックします。

デモは先頭のディスプレイを1280×720、20Mbpsで要求します。音声の要求はチェックボックスで選択します。接続情報はファイルに保存しません。

音声の再生方法は[既存Kyberホストから音声を受信する](audio_ja.md#既存kyberホストから音声を受信する)を参照してください。

入力を送る場合はチェックボックスをオンにし、映像をクリックします。Escキーまたはウィンドウのフォーカス移動で解除します。デモは`send_input_event()`でキーボード、マウス、ホイール、標準ゲームパッドを送信します。[入力とクリップボードのAPI](extensions_ja.md)を参照します。
<!-- capture: host-settings | 既存ホストの接続設定（未接続） -->

## GDScriptで使う

```gdscript
extends Node

@onready var receiver: KyberClient = $KyberClient
@onready var screen: TextureRect = $TextureRect

func _ready() -> void:
    receiver.connected.connect(_connected, CONNECT_DEFERRED)
    receiver.frame_received.connect(_frame, CONNECT_DEFERRED)
    receiver.error_occurred.connect(push_error, CONNECT_DEFERRED)
    receiver.connect_to_host(
        "stream.example.net", 8080,
        OS.get_environment("KYBER_USERNAME"),
        OS.get_environment("KYBER_PASSWORD"), ""
    )

func _connected() -> void:
    var displays := receiver.get_displays()
    if not displays.is_empty():
        receiver.start_stream(displays[0].id, 1280, 720, 20000000, false)

func _frame() -> void:
    screen.texture = receiver.get_texture()
```

シグナル内から同じRustノードを呼ぶため、`CONNECT_DEFERRED`を指定します。`connect_to_host()`と`start_stream()`の戻り値は要求の受付結果です。接続完了・配信開始はシグナルまたは`get_state()`で確認します。

3D空間に表示する場合は、`demo/spatial_screen.gd`をPlaneMeshの`MeshInstance3D`へ設定します。`client_path`には`KyberClient`ノードを指定します。

| API | 内容 |
|---|---|
| `connect_to_host(host, port, username, password, fingerprint)` | 組み込みSDKで接続を要求します。既存の接続は終了します。 |
| `start_stream(display_id, width, height, bitrate, audio)` | 接続完了後に呼びます。出力サイズは各辺1〜4096pxです。 |
| `close()` | SDKを停止してフレーム・テクスチャを破棄します。停止完了まで待機します。 |
| `get_texture()` | 最新の`ImageTexture`を返します。フレーム受信前と切断後は`null`です。 |
| `get_displays()` | `id`、`width`、`height`の辞書配列を返します。 |
| `get_received_frames()` / `get_dropped_frames()` | 表示処理したフレーム数／待機中に置換したフレーム数を返します。 |
| `send_key(usage, pressed)` | USB HID Keyboard Usage IDを送ります。例: Aは4です。 |
| `send_mouse_motion(dx, dy)` | -32768〜32767の相対移動を送ります。 |
| `send_mouse_button(button, pressed)` | 0＝左、1＝右、2＝中央を送ります。 |

入力APIの`true`はローカルでの受付を意味します。ホストでの操作完了を保証しません。接続前に`set_auto_reconnect(true)`で公式SDKの自動再接続を有効にできます。手動で接続し直す場合は、再び`connect_to_host()`を呼びます。[SDKの自動再接続](video_ja.md#kyber-sdkの自動再接続を有効にする)を参照します。
