<!-- SPDX-FileCopyrightText: 2026 KOGA Mitsuhiro -->
<!-- SPDX-License-Identifier: MIT -->

# 映像受信と動画ファイル送受信

[ドキュメント一覧](README_ja.md) | [English](video.md)

アドオンを導入済みで、Godotでデモプロジェクトを開ける状態から説明します。

## 動画ファイルを送受信する

送信側はWindowsではMedia Foundation、macOS/iOSではAVFoundation、LinuxではシステムのGStreamer、AndroidではMediaExtractorで動画を読み込みます。FFmpegコマンドや外部SDKランタイムは不要です。対応形式はシステムのコーデックに依存します。

ファイル動画デモは既定でJPEGフレームをKyProto/QUICで送信します。H.264も選択できます。設定は[H.264ガイド](h264_ja.md)を参照してください。全プラットフォーム共通で、JPEGの圧縮・復号を別スレッドへ分離しています。これにより、CPU処理がQUICの送受信や応答を待たせることを防ぎます。既存KyberホストのH.264受信は別の実装です。

### Godotからデモを起動する

1. Godotでデモプロジェクトの`project.godot`を開きます。
2. ［デバッグ］メニューから［実行インスタンスのカスタマイズ…］を開きます。
3. ［複数インスタンスを有効］をオンにします。
4. インスタンス数を［2］に設定します。
5. ［プロジェクトを実行］（F5）をクリックします。デモが2つ起動します。

設定項目には英語名を併記しています。表示名はエディタの言語設定によって異なります。

<!-- capture: run-instances-ja | Godotの実行インスタンス設定ダイアログ。複数インスタンスを有効にし、起動数を2にした状態。 -->

### 送信側と受信側を接続する

1. 片方を［Sender］にし、［Open video…］で動画を選択します。H.264のMP4など、システムが読み込める形式を利用します。その他の形式はシステムのコーデックに依存します。
2. ［Start streaming］を押し、表示された接続コードをコピーします。受信側が接続すると、動画の先頭から送信を開始します。
3. もう片方を［Receiver］にし、接続コードを貼り付けて［Connect and receive］を押します。同じPCではIPを`127.0.0.1`、ポートを両方とも`4433`にします。
4. 送信側にプレビュー、受信側に受信映像が表示されます。［Stop］またはモード切替で通信を終了します。

LAN内の別PCに送る場合は、送信側の待受IPを`0.0.0.0`、受信側の接続先を送信PCのIPv4アドレスにします。指定したUDPポートをファイアウォールで許可してください。接続コードは送信開始ごとに変わり、証明書のSHA-256と認証トークンを含みます。

映像はアスペクト比を保って640×360へ変換し、最大24 fpsのJPEGフレームとしてKyProtoの映像エンドポイントへ送ります。既定の転送方式はReliableです。送信中は同じ接続コードで複数の受信者が参加・再参加できます。Windows・macOS・Linux・Android・iOSでは音声トラックがある動画の音声も送信します。シークと切断後の自動再接続は未対応です。[Viewportと複数受信者の詳細](extensions_ja.md)を参照します。ループをオフにすると動画終了時に通信を終了し、最後のフレームを表示します。

このデモのペイロードと接続手順はアプリ専用です。既存KyberホストのH.264配信とは互換ではありません。通信層は[SDK統合仕様](sdk.md)で固定したKyber公式ソースを使用しています。

| `KyberFileVideo` API | 内容 |
|---|---|
| `start_sender(file, bind_ip, port, repeat)` | 絶対パスの動画を指定して待受を開始します。 |
| `get_connection_code()` | 待受開始後にコードを返します。 |
| `start_receiver(host, port, code)` | 送信側へ接続します。 |
| `get_texture()` / `get_frame_count()` | 最新テクスチャ／表示フレーム数を返します。 |
| `is_active()` / `has_failed()` / `get_status()` | 非同期処理の状態を返します。開始APIの`true`は要求の受付を意味します。 |
| `stop()` | ワーカーを停止し、テクスチャとコードを消去します。 |
<!-- capture: file-sender | 送信側のプレビュー -->

<!-- capture: file-receiver | 受信側の再生画面 -->

## VideoStreamPlayerで映像を受信する

`KyberVideoStream`はGodotの`VideoStream`を継承します。標準の`VideoStreamPlayer`へ設定して映像を受信できます。単独利用と、`KyberSession`を共有する利用方法があります。

### 接続を共有して入力と詳細状態を扱う

接続前に`KyberSession`ノードをシーンへ追加します。動画ファイルデモはこの方式を使い、映像の縦横比も維持します。

```gdscript
@onready var session: KyberSession = $KyberSession
@onready var player: VideoStreamPlayer = $VideoStreamPlayer

func receive(host: String, port: int, connection_code: String) -> void:
    if not session.start_receiver(host, port, connection_code):
        $Status.text = session.get_last_error()
        return
    var stream := KyberVideoStream.new()
    stream.set_session(session)
    player.stream = stream
    player.play()

func _process(_delta: float) -> void:
    $Status.text = session.get_status()
    if player.paused:
        $Status.text += " | 表示は一時停止中"
```

SDK受信では`KyberClient`と同じ引数で`session.connect_to_host()`を呼び出します。`connected`の通知後に`get_displays()`から対象を選択します。続いて`start_stream(display_id, width, height, bitrate, audio)`を呼び出し、動画リソースにセッションを設定します。`send_input_event()`を含む入力APIも同じ接続を使います。既存の`KyberClient`と`KyberFileVideo`のAPIは維持し、セッション内部でその実装を再利用しています。

| API | 用途 |
|---|---|
| `get_state()` | `DISCONNECTED`、`CONNECTING`、`CONNECTED`、`STARTING`、`STREAMING`、`FAILED`、`DEMO`、`FINISHED`、`RECONNECTING` |
| `get_status()` / `get_last_error()` | 現在の詳細メッセージ／保持したエラー。ファイル転送の通信状態も取得可能 |
| `get_statistics()` | `received_frames`、`dropped_frames`、`width`、`height`。受信数は受信ノードが取り出したフレーム数 |
| `get_capabilities()` | 入力、ディスプレイ選択、シーク、Godot音声、ネイティブ音声要求の現在の対応状況 |
| `state_changed`、`status_changed`、`error_occurred` | 遅延通知。コールバック内でセッションの参照や操作が可能 |
| `connected`、`disconnected`、`displays_changed`、`frame_received` | 接続・映像の通知。`connected`はSDKのディスプレイ情報取得完了を通知 |
| `close()` | 全利用者の接続を終了。セッションをシーンツリーから外した場合も切断 |

セッションはプレイヤーとは独立して受信処理を進めます。一時停止はそのプレイヤーの表示だけを止め、受信、状態更新、入力操作は継続します。プレイヤーを停止・解放しても共有接続は切断しません。映像のみの場合は複数のプレイヤーで1つのセッションを共有し、それぞれ独立して一時停止できます。音声付きセッションでは、音声キューの読み出し先が1つのためプレイヤーは1つにします。複数箇所に表示する場合は、そのプレイヤーのテクスチャを共有します。利用終了時にはセッションを明示的に閉じます。シーンツリー全体を一時停止した場合は、通常のNodeと同じく`process_mode`に従います。

`set_session()`は実行時専用で、ノード参照は`.tres`へ保存しません。プレイヤーへリソースを設定する前に呼び出します。共有時はセッションの接続設定を使い、リソース側のホスト・配信元設定は使いません。テクスチャと表示フレーム数は各プレイヤーが持ち、受信統計はセッションが管理します。受信フレームの分配にGPUからの画像読み戻しは行いません。

`FINISHED`はファイル転送の正常終了を表します。認証の進捗は通信層から取得できる情報だけを扱い、再生状態から推測しません。表示を一時停止しても、受信状態やエラーを確認できます。動画ファイルの音声はGodotの動画プレイヤーから出力します。`get_capabilities().godot_audio`は`true`、`.native_audio_request`は`true`、`.seek`は`false`です。既存ホストへの`start_stream(..., true)`でOpus音声を要求します。

### Kyber SDKの自動再接続を有効にする

`connect_to_host()`の前に`session.auto_reconnect = true`を設定します。既存ホスト向けデモにも、この設定があります。`KyberClient`を直接使う場合は、接続前に`set_auto_reconnect(true)`を呼びます。設定変更は次の接続から有効で、既定は無効です。

ブリッジは公式SDKの`ConnectConfig.auto_reconnection`へ設定を渡します。固定SDKは接続喪失後、1秒間隔で最大10回の再接続を行います。成功すると試行回数をリセットします。回数と間隔はSDK内部の既定値であり、Godot側の設定項目ではありません。初回の接続失敗は、この仕組みでは再試行しません。Godot側に再試行タイマーやSDKクライアントを作り直す処理はありません。

SDKの`Reconnecting`イベントを`RECONNECTING`と`get_reconnect_attempts()`へ反映します。共有プレイヤーは復旧中も有効な状態を維持します。`Reconnected`を受け取った際、SDKが`should_restart_streaming = true`を通知した場合だけ、最後に受け付けた配信設定を再要求します。`ReconnectionFailed`は`FAILED`になります。セッションを閉じるとネイティブクライアントを破棄し、SDKが保留中の処理をキャンセルします。共有プレイヤーの停止は表示だけを止めます。

組み込みSDKの機能を利用します。SDKの自動再接続が有効な場合、`DataPlaneClosed`は接続中断として通知し、再接続前にクライアントを破棄しないようにしています。

動画ファイル受信、RPC、単独動画リソースでは、この機能を有効にしていません。独自の動画ファイル通信は公式SDKホストへの接続ではないためです。テストでは同一のテスト用ハンドルからSDKイベントを模擬し、配信復元、最終的な失敗、キャンセルを検証します。固定SDKにブリッジを組み込んだ型チェックも実施済みです。実際のKyberホストでの再接続は未検証です。

公式実装: [接続設定と再接続処理](https://gitlab.com/kyber/core/kyctl/-/blob/92c632f9e889917a34aaa7697fb88b4991054b1c/kyclient/src/lib.rs)。

### 動画リソースを単独で使う

```gdscript
var stream := KyberVideoStream.new()
stream.source = KyberVideoStream.FILE_TRANSFER
stream.host = "127.0.0.1"
stream.port = 4433
stream.set_connection_code(connection_code)
$VideoStreamPlayer.stream = stream
$VideoStreamPlayer.play()
# Pause presentation; resume at the latest received frame.
$VideoStreamPlayer.paused = true
$VideoStreamPlayer.paused = false
```

共有セッションを指定しない場合、接続は`VideoStreamPlayback`の実装が管理します。`stop()`で切断し、配信が終了するとプレイヤーの`finished`シグナルが発火します。停止後も最終フレームを保持します。表示を消すには`player.stream = null`を設定します。

`player.get_video_texture()`は`TextureRect`、`Sprite2D`、3Dマテリアルに設定できます。再生中は同じテクスチャオブジェクトを更新します。更新を続けるには、プレイヤーをシーンツリー内に置きます。

Inspectorでリソースを作成し、`ResourceSaver`で設定を`.tres`へ保存できます。認証情報はプレイヤーへリソースを設定する前に渡します。パスワードと接続コードは保存対象に含めません。設定はプレイヤーが再生インスタンスを生成するときにコピーします。設定変更後はリソースを再設定します。`get_last_error()`と`get_frame_count()`は最後に生成した再生インスタンスの状態を返すため、プレイヤーごとに別のリソースを使用します。

| Source | 設定 |
|---|---|
| `FILE_TRANSFER` | `host`、`port`、`set_connection_code()`。動画ファイルデモのJPEG配信を受信 |
| `KYBER_HOST` | `host`、`port`、`username`、`fingerprint`、`set_password()`。初期値では最初のディスプレイを選択（`display_id = -1`） |
| `LOCAL_DEMO` | ネットワークを使わずに動くローカル映像 |

`KYBER_HOST`では再生前に`width`、`height`、`bitrate`も設定できます。`KyberClient`と同じSDKブリッジを使います。テスト用SDKによる状態遷移を検証しています。実ホストでの受信は未検証です。

ライブ受信のため長さは不明で、シークには対応しません。一時停止しても送信側は停止しません。映像のみの場合は最新フレームを保持します。音声付きのファイル受信では、音声の再生位置に合わせて映像を選びます。VideoStreamPlayerの音量と音声バスを利用できます。

### Godotの入力イベントを転送する

入力キャプチャ中の`_input()`から`KyberClient.send_input_event(event)`を呼び出します。キーボード、相対マウス移動、3ボタン、ホイール、標準ゲームパッドのボタン・軸に対応します。[拡張入力](extensions_ja.md#ホイールとゲームパッドを送る)を参照します。物理キーコードを優先し、未設定の場合は論理キーコードを使います。キーリピート、未対応キー、ホイールの解放、未対応のゲームパッド操作は`false`を返します。マウス移動量は整数へ丸め、SDKの符号付き16ビット範囲へ制限します。修飾キーは左側のHIDコードに変換します。

既存ホスト向けデモでは、このAPIとキャプチャ解除時のキー解放を使用します。変換表にないHIDコードは従来の`send_key()`で送信できます。`KyberSession`も同じ入力APIを提供し、`VideoStreamPlayer`と共有する接続へ入力を送信できます。

## メディア機能の確認

`KyberSession`または`KyberFileVideo`の`get_media_capabilities()`で、`backend`、`h264_decode`、`file_video`、`h264_encode`、`audio_playback`、`file_audio`、`jpeg_receive`、`reason`を取得できます。
`file_video`はシステムの動画読み込みAPIの利用可否を示します。`file_audio`はプラットフォーム用の音声読み込み実装があることを示し、コーデックの利用可否を保証しません。未対応のコーデックや不足したLinux音声プラグインは、ファイルを開く際に非同期エラーになります。
既存コードは`start_sender()`の`ffmpeg`引数を削除します。ホストの音声も要求する場合は`audio = true`、映像だけの場合は`false`を指定します。

## 動画ファイルの音声

Windowsの送信側は、Media Foundationで最初の音声トラックを読み込みます。48 kHz・ステレオのPCMへ変換し、Opusで圧縮します。
macOSとiOSではAVFoundation、LinuxではGStreamerの`audioconvert`と`audioresample`で同じPCM形式に変換します。AndroidではMediaExtractorとMediaCodecで復号し、モノラル・ステレオのPCM16または浮動小数点PCMを、状態を保持する線形補間で48 kHz・ステレオへ変換します。Androidのその他のチャンネル構成は未対応です。OSから返る音声ブロックは20 ms単位に分割し、映像と時刻順に送信します。
Opusは20 ms単位、128 kbit/sで送信します。静的リンクするため、追加のコーデックDLLは不要です。
音声は映像と同じ接続内のKyProto音声エンドポイントで送ります。
音声バッファの調整と再生速度差の補正には、Kyberの`kyaudioreg`を利用します。

`KyberVideoStream`は`VideoStreamPlayback.mix_audio()`にPCMを渡します。`VideoStreamPlayer`の音量、音声バス、一時停止を利用できます。
低レベルの`KyberFileVideo`は、内部の`AudioStreamGenerator`と`AudioStreamPlayer`で音声を再生します。
音声キューの読み出し先は1つです。1セッションに接続する動画プレイヤーは1つにします。

受信側は、PCMの待機量とGodotが報告する出力遅延から音声の再生位置を推定します。この位置に合わせて映像を選択します。
初期設定のバッファはKyber側が約80 ms、Godot側が最大40 msです。別途、デバイスの出力遅延が加わります。
`performance.av_sync`は、推定した音声位置に対する表示対象フレームの遅れです。実際のスピーカーと画面のずれを測定する値ではありません。
映像エンコードには独立したキューを使い、映像処理が音声送信を遅らせないようにしています。

ファイル送受信プロトコルはリビジョン2です。新しい受信側はリビジョン1の映像専用送信にも対応します。古い受信側ではリビジョン2を受信できません。
ファイルからの音声抽出はWindows・macOS・Linux・Android・iOSで実装しています。既存ホストのOpus音声受信も実装しています。実ホストとの相互運用は未検証です。音声バス・マイクのキャプチャ、`KyberAudioStream`、転送方式の選択は[ライブ音声](audio_ja.md)を参照してください。

送信開始前に`KyberFileVideo.audio_enabled = false`を設定すると、映像のみを送信します。送信側OSが音声コーデックに対応しないファイルでも、映像のみの送信を選択できます。

## H.264

ファイル・Viewport送信でH.264を選べます。JPEGが既定値です。設定、OSごとの制限、プロトコル互換性は[H.264ガイド](h264_ja.md)を参照してください。
