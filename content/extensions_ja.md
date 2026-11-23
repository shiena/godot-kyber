<!-- SPDX-FileCopyrightText: 2026 KOGA Mitsuhiro -->
<!-- SPDX-License-Identifier: MIT -->

# キャプチャ・入力・性能計測

[ドキュメント一覧](README_ja.md) | [English](extensions.md)

## 機能と接続方式を選ぶ

| やりたいこと | 使用する接続 |
| --- | --- |
| Godotで描画したシーンを複数端末へ配信する | `KyberFileVideo`のファイル・Viewport用プロトコル |
| 既存ホストを操作する・クリップボードを交換する | 配信中のSDKホストセッション |
| アプリ間でRPCを送る | `KyberMultiplayerPeer`。[RPCガイド](multiplayer_ja.md)を参照 |

映像を接続するだけで、すべての入力・クリップボード機能が利用できるわけではありません。シーンに組み込む前に、各節の接続条件を確認します。

## Viewportを複数の受信者へ送る

```gdscript
var sender := KyberFileVideo.new()
add_child(sender)
sender.start_viewport_sender($SubViewport, "0.0.0.0", 4433, 24)
```

デモの［Viewport streaming］で動くシーンを送信できます。画面に表示しないSubViewportは`UPDATE_ALWAYS`に設定します。描画用ディスプレイサーバーが必要です。ヘッドレスでのキャプチャはエラーを返します。

`RenderingServer.frame_post_draw`の後に取得します。GPUからの読み戻しはGodotのメインスレッドで行います。リサイズとJPEG圧縮はワーカーで行います。出力は640×360、最大24fpsです。縦横比を維持し、余白を黒で埋めます。取得元の寸法は縦横とも4096ピクセル以内です。ここではJPEGの取得経路を説明しています。H.264の高速化とGPU転送の条件は[H.264ガイド](h264_ja.md)を参照してください。

受信者は同じアドレス、ポート、接続コードを指定します。動画ファイルとViewportは、圧縮結果を共有して複数の受信者へ送ります。受信接続と接続処理の合計上限は16です。接続ごとに送信待ちの最新1枚を保持するため、遅い受信者が他の受信者を待たせません。音声キューは50パケットまで保持し、この範囲を超えると古いパケットを飛ばします。送信処理がタイムアウトした受信者は切断します。

`get_connected_clients()`は認証とエンドポイント準備が済んだ接続数です。`get_skipped_client_frames()`は各接続で飛ばした圧縮済みフレームの合計です。送信中に切断した受信者は、再度接続できます。この経路に自動再接続はありません。

RPCは引き続き相手1台です。動画配信のプロトコルは公式Kyberホストとは別です。現行プロトコルはリビジョン2で、クロック用エンドポイント4と音声用エンドポイント6を使用します。新しい受信側はリビジョン1の映像専用送信にも対応します。古い受信側はリビジョン2を受信できません。Viewportには[音声バスまたはマイク](audio_ja.md)を追加できます。Unreliable・FECの接続コードには新しい受信側が必要です。[プロトコル互換性](audio_ja.md#転送方式とプロトコル互換性)を参照してください。

<!-- capture: viewport-sender | Viewportの送信画面（受信者1台の例） -->

<!-- capture: viewport-receiver | Viewportの受信画面 -->

## 処理時間を調べる

`KyberFileVideo.get_performance_statistics()`、`KyberClient.get_performance_statistics()`、`KyberSession.get_statistics().performance`で取得します。

| 項目 | 意味 |
|---|---|
| `capture` | Viewportの読み戻しとRGBA変換 |
| `encode`、`decode` | CPUでのJPEG変換・圧縮、または復号 |
| `receive_interval` | 動画パケットの受信間隔 |
| `decode_queue` | 受信からJPEG復号開始までの待ち時間 |
| `receive_to_ready` | 受信から復号済みRGBAが用意されるまでの時間 |
| `ready_interval` | 復号済み映像またはプレビュー映像の生成間隔 |
| `queue` | RGBA生成からGodotが取り出すまでの時間 |
| `texture_update` | テクスチャ作成・更新のCPU処理時間 |
| `display_interval` | テクスチャ更新間隔。物理画面の更新間隔ではない |
| `source_to_receive`、`source_to_ready` | 送信側の単調増加時計を基準とした推定経過時間 |
| `av_sync` | 推定音声再生位置に対する表示対象フレームの遅れ。実際のスピーカーと画面のずれではない |

各項目は`available`と`samples`を持ちます。取得済みの場合は`last_ms`、`mean_ms`、`max_ms`、`p95_ms`も返します。間隔の項目には`fps`もあります。直近120サンプルを保持します。`audio_received_samples`は音声Mailboxへ渡したステレオPCMフレーム数です。`audio_playback_pts_us`はマイクロ秒単位の推定再生位置です。計測できない段階は0ではなく未取得として返します。

動画ファイル／Viewportでは、公式KyProtoの`ClockSyncClientProtocol.sync_average(3)`とサーバー側の`serve()`を使います。オフセットは10秒ごとに更新します。初回同期と更新の待ち時間は最大5秒で、失敗時は受信を終了します。`clock_synchronized`、`clock_offset_ms`、`clock_rtt_ms`は最後に成功した推定結果です。往路と復路の遅延差により誤差が生じます。

ViewportのPTSは取得時刻、動画ファイルのPTSは送信側での再生予定時刻です。`source_to_ready`は圧縮・通信・復号を含みますが、画面表示は含みません。`end_to_end_latency_available`はfalseのままです。共通クロックがあっても、複数端末のフレーム単位の同時表示は保証しません。既存ホスト用の再生経路では、共通クロックに基づく表示制御は未実装です。

受信処理は復号を待たずにパケットを読み出します。復号が遅れた場合は圧縮済みの最新1枚へ進みます。同期時計で500ms以上古いと判定した映像は復号前に捨てます。受信側のドロップ数は、圧縮済み映像のスキップと復号済み映像の上書きを含みます。

## ホイールとゲームパッドを送る

`KyberSession`と`KyberClient`の`send_input_event(event)`で、`InputEventMouseButton`、`InputEventJoypadButton`、`InputEventJoypadMotion`を渡せます。

- `send_mouse_wheel(dx, dy)`: −1〜1の有限値を指定します。Xの正方向は右、Yの正方向は下です。ホイールの解放イベントは送りません。
- `send_gamepad_button(device, button, pressed)`: デバイス番号0〜31、Godot標準ボタン0〜14を指定します。
- `send_gamepad_axis(device, axis, value)`: Godotの軸0〜5を指定します。スティックは−1〜1、トリガーは0〜1です。
- `send_gamepad_connection(device, connected)`: リモート側に接続・取り外しを通知します。最初の入力でも自動的に接続します。

`Input.joy_connection_changed`で取り外しを検出したときや、入力キャプチャを解除するときに切断を通知します。デモはフォーカス喪失とEscキーでもゲームパッドを解放します。入力には配信中のSDKホスト接続が必要です。実際の操作可否はホストOSの入力機能と権限にも依存します。拡張入力には後述の`KyberInputRelay`を使います。既存ホストとの入力の相互運用は未検証です。

## Godotのクリップボードとテキストを交換する

接続前に`KyberSession.clipboard_enabled = true`を設定します。`KyberClient`は接続前に`set_clipboard_enabled(true)`を呼びます。既定値はfalseです。

```gdscript
session.clipboard_enabled = true
session.clipboard_text_received.connect(func(text): received_text.text = text)
# Call from explicit UI actions after starting the host stream.
session.send_system_clipboard()
session.request_clipboard_text()
# Apply received text with a separate user action.
session.apply_clipboard_text(received_text.text)
```

`send_clipboard_text(text)`はOSのクリップボードを読まずに指定テキストを提供します。KyberのChange、DataRequest、Data、DataUnavailableパケットを使います。`clipboard_available`で相手のテキスト提供を通知します。`request_clipboard_text()`で取得を要求し、`clipboard_text_received(text)`で受け取ります。取得できない場合は`clipboard_unavailable`で通知します。

受信だけではOSのクリップボードを書き換えません。提供したテキストは、置き換えるか切断するまで接続先が取得できます。UTF-8で60KiBを超えるテキストは拒否します。対応経路はSDKホスト接続です。動画ファイル、Viewport、RPCには自動でクリップボード交換を追加しません。HTMLと画像は未対応です。送信APIの成功はローカルSDKへの受け渡しを示し、相手の貼り付け完了を保証しません。

`is_system_clipboard_supported()`で`DisplayServer.FEATURE_CLIPBOARD`を実行時に確認します。`send_system_clipboard()`は`clipboard_get()`、`apply_clipboard_text()`は`clipboard_set()`をメインスレッドで呼びます。

| 対象 | Godot APIとOSの制約 |
|---|---|
| Windows | 読み書きAPIに対応 |
| macOS | 対応。OSの版とアクセス設定により許可が必要な場合がある |
| Linux | 現行GodotのX11・Waylandで対応。セッションやコンポジターの制約に従う |
| Android | 対応。Android 10以降はバックグラウンドからの読み取りを制限 |
| iOS | 対応。iOS 16以降はプログラムからの読み取りで許可を求める場合がある |
| ヘッドレス | 実行時に対応を確認。OSのクリップボードがあるとは仮定しない |

APIへの対応と、OSからのアクセス許可は別です。Godotの文字列を返す読み取りAPIでは、空の内容と取得拒否を常に区別できるわけではありません。モバイルOSの許可画面と実ホストとのクリップボード交換は未検証です。

出典: [Godot DisplayServer](https://docs.godotengine.org/en/4.6/classes/class_displayserver.html)、[Androidの制限](https://developer.android.com/about/versions/10/privacy/changes)、[Apple UIPasteControl](https://developer.apple.com/documentation/uikit/uipastecontrol)、[macOSのアクセス設定](https://developer.apple.com/documentation/appkit/nspasteboard/accessbehavior-swift.enum)。

## 拡張入力を転送する

Nodeに`res://addons/kyber/input_relay.gd`を設定します。`_input()`または`_unhandled_input()`の片方から`send_input_event(event)`を呼びます。

公式Kyberホストを操作する場合は、`session`に配信中の`KyberSession`を設定します。既定の`touch_as_mouse`は、最初の指の押下と解放を左マウスボタンに変換します。ドラッグは相対マウス移動になります。タッチパッドと同じ操作で、絶対座標やマルチタッチではありません。ほかの指は無視します。タッチとマウスの両方を転送する場合は、Godotのタッチからマウスを生成する設定を無効にします。

Kyber標準の振動パケットは、`KyberClient`と`KyberSession`の`gamepad_rumble_received(device, weak, strong, duration)`で通知します。リレーの`local_gamepad_device`には、ホストへ送ったコントローラーIDを指定します。一致する通知を`rumble_received`で伝えます。`vibration_enabled`を有効にすると、`Input.start_joy_vibration()`と`stop_joy_vibration()`で適用します。SDKの負の継続時間は、公式実装と同じ5秒として扱います。

Godot間の入力では`session`を未設定にします。接続済みの`KyberMultiplayerPeer`を`multiplayer.multiplayer_peer`に設定します。両側のリレーノードのパスを一致させます。`remote_peer_id`は相手のIDです。サーバー側は2、クライアント側は1にします。0はRPC入力を無効にします。

```gdscript
func _unhandled_input(event: InputEvent) -> void:
    $InputRelay.send_input_event(event)

func on_remote_collision() -> void:
    $InputRelay.send_rumble(0.25, 0.75, 0.1)
```

RPCでは`InputEventScreenTouch`、`InputEventScreenDrag`、`JOY_BUTTON_PADDLE1`～`JOY_BUTTON_PADDLE4`を転送します。タッチのインデックスは0～31です。座標と移動量は送信側Viewportで正規化し、受信側のサイズに合わせます。映像に余白がある場合は、アプリ側で座標を補正します。標準のキー・マウス・ゲームパッド入力には、既存のセッション入力経路またはアプリのRPC処理を使います。

受信時は`input_received(event)`を通知します。`inject_received_input`を有効にすると、`Input.parse_input_event()`でも適用します。イベントのデバイスIDは予約値`-2`です。このIDの再送信を拒否して循環を防ぎます。InputMapの対象は全デバイスまたはこのIDに設定します。振動は`vibration_enabled`で有効にし、`local_gamepad_device`に適用します。アプリ内の振動API呼び出しは捕捉しません。遠隔振動には`send_rumble()`を明示的に呼びます。

GodotのReliable RPCとKyberのQUIC経路を使います。両側にリレースクリプトが必要です。公式Kyberの入力パケットや動画ファイルのプロトコルは変更しません。受信は設定した相手だけを許可します。フォーカス喪失、切断、ノード削除で保持中の入力と振動を解除します。接続先、セッション、入力設定の変更前に`reset_input()`を呼びます。

Windows・macOS・Linux・Android・iOSで共通のGodot APIを使います。タッチ、パドル、振動の可否はOS対応と機器に依存します。端末本体の振動には自動変換しません。SDKの振動パケット、タッチ変換、不正な要求の拒否、Kyber接続での双方向RPCを自動テストしています。今回追加した機能の実機振動とモバイルのタッチ操作は未検証です。
