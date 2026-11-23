<!-- SPDX-FileCopyrightText: 2026 KOGA Mitsuhiro -->
<!-- SPDX-License-Identifier: MIT -->

# ライブ音声と転送方式

[ドキュメント一覧](README_ja.md) | [English](audio.md)

## 音声の入力元を選ぶ

| 入力元 | 使用する方法 |
| --- | --- |
| 動画ファイルの音声 | 音声トラックを含むファイルを送信する |
| ゲーム音声・生成音 | 専用のGodot音声バスをキャプチャーする |
| マイク | 音声入力を有効にし、OSごとの録音権限を設定する |

同じ接続で映像と音声を受信する場合は、同じ`KyberSession`を使用します。両方のプレイヤーが動作している間はセッションへの参照を保持します。ファイル音声の送信と音声受信にはマイク権限は不要です。

## Godot音声バスを送信する

```gdscript
var sender := KyberFileVideo.new()
add_child(sender)
if sender.start_audio_sender("0.0.0.0", 4433):
    sender.capture_audio_bus("Music")
```

キャプチャ前に対象のバスを作成します。映像も送る場合は、`start_audio_sender()`の代わりに`start_viewport_sender()`を呼んでから音声バスを指定します。準備完了後に`get_connection_code()`で接続コードを取得し、受信側へ渡します。ファイル送信ではライブ入力ではなくファイルの音声トラックを使用します。

バスの末尾に`AudioEffectCapture`を追加し、ステレオPCMを状態付き線形補間で48 kHzへ変換します。ワーカーで20 ms単位のOpusへ圧縮します。映像と音声にはKyProtoのクロックを使います。`stop_audio_capture()`または送信停止で、追加したエフェクトを削除します。バスの並べ替えに対応します。対象バスやエフェクトが削除されると、エラーを通知して送信を止めます。キューは上限付きで、過負荷時には古い音声を飛ばします。受信者がいない間の音声は保持しません。

同じアプリで受信も行う場合は、専用の送信元バスを使い、受信音声の送り返しを避けてください。キャプチャ自体はローカル再生を追加しません。5つの対象プラットフォームに共通するGodot APIを利用します。今回のライブ音声の自動テストはデスクトップ向けで、モバイルの実マイクは未検証です。

<!-- capture: audio-bus-ja | Godotの音声バスレイアウト。例で使用する専用のMusicバスを表示した状態。 -->

## マイクを送信する

音声専用またはViewportの送信開始後に、`capture_audio_bus()`の代わりに`capture_microphone()`を呼びます。`AudioStreamMicrophone`と内部のミュートした音声バスで入力を取得し、同じ経路で送信します。エコー除去やノイズ抑制は行いません。

アプリの起動前にプロジェクト設定の`audio/driver/enable_input`を有効にします。権限要求とエクスポート設定はアプリ側で行います。アドオンは権限ダイアログを表示せず、プロジェクト設定も変更しません。キャプチャ開始の成功は初期化の成功を示し、録音権限や実際の入力音声を保証しません。GodotはWindows・macOS・Linux・Android・iOSのマイク入力に対応します。iOSでは音声セッションのカテゴリも［Record］または［Play and Record］に設定します。[Godotのマイク設定](https://docs.godotengine.org/en/stable/tutorials/audio/recording_with_microphone.html)と[入力権限](https://docs.godotengine.org/en/4.2/classes/class_projectsettings.html#class-projectsettings-property-audio-driver-enable-input)を参照してください。

デスクトップではOSのプライバシー設定と入力デバイスを確認します。モバイルのエクスポートには録音権限・用途説明と実行時の許可が必要です。ヘッドレスのDummy音声ドライバーでは実マイクを検証できません。

### macOSのマイク利用を設定する

アプリの起動前に、［プロジェクト設定］の［Audio］＞［Driver］＞［Enable Input］を有効にします。設定キーは`audio/driver/enable_input`です。

1. ［プロジェクト］＞［エクスポート］でmacOSのプリセットを選択します。
2. ［Codesign］＞［Entitlements］＞［Audio Input］を有効にします。設定キーは`codesign/entitlements/audio_input`です。
3. ［Privacy］＞［Microphone Usage Description］に用途を入力します。設定キーは`privacy/microphone_usage_description`です。英語の例は`Send microphone audio to the connected receiver.`です。
4. 更新したプリセットでアプリをエクスポートし、署名します。
5. エクスポートしたアプリを起動し、macOSのマイク利用要求を許可します。

日本語の用途説明は`privacy/microphone_usage_description_localized`に設定します。説明例は「マイクの音声を接続先の受信アプリへ送信します。」です。拒否済みの場合は、［システム設定］＞［プライバシーとセキュリティ］＞［マイク］で対象アプリを許可します。その後、アプリを再起動します。エディターとエクスポートしたアプリの権限は別なので、両方で確認します。[GodotのmacOSエクスポート設定](https://docs.godotengine.org/en/stable/classes/class_editorexportplatformmacos.html)を参照してください。

### iOSのマイク利用を設定する

1. ［プロジェクト設定］の［Audio］＞［Driver］＞［Enable Input］を有効にします。設定キーは`audio/driver/enable_input`です。
2. プロジェクト設定の［高度な設定］を有効にします。
3. ［Audio］＞［General］＞［iOS］＞［Session Category］を［Play and Record］に設定します。設定キーは`audio/general/ios/session_category`です。再生せず録音だけを行う場合は［Record］を選びます。
4. iOSのエクスポートプリセットで［Privacy］＞［Microphone Usage Description］に録音・送信の用途を入力します。設定キーは`privacy/microphone_usage_description`です。
5. 更新したプロジェクトをエクスポートし、Xcodeで再ビルド・インストールします。
6. 実機に表示されたマイク利用要求を許可します。

日本語の用途説明には`privacy/microphone_usage_description_localized`を使います。拒否済みの場合はiOSの［設定］で対象アプリのマイク利用を許可し、再起動します。エクスポートの用途説明を変更した場合は、再エクスポートと再ビルドが必要です。[GodotのiOSエクスポート設定](https://docs.godotengine.org/en/stable/classes/class_editorexportplatformios.html)と[マイク録音ガイド](https://docs.godotengine.org/en/stable/tutorials/audio/recording_with_microphone.html)を参照してください。

### マイク音声の送信を確認する

ライブ送信を開始して`capture_microphone()`を呼び、受信側を接続します。選択したマイクへ話し、受信側の音声出力を確認します。ハウリングを避けるため、ヘッドホンを使用します。無音の場合は、OSの録音権限、入力デバイス、各プラットフォームの設定を確認します。`capture_microphone()`が`true`を返すだけでは、録音許可を確認できません。

これらはマイク入力用の設定です。生成音・ゲーム音声のバス取得、ファイル音声の送信、音声受信にはマイク権限は不要です。アドオンは設定を自動変更しません。今回の実装では、macOS・iOSの実マイクによる録音は未検証です。

## AudioStreamPlayerで再生する

```gdscript
var stream := KyberAudioStream.new()
stream.host = "192.168.0.10"
stream.port = 4433
stream.set_connection_code(connection_code)
$AudioStreamPlayer.stream = stream
$AudioStreamPlayer.play()
```

独立した受信接続を作成します。`AudioStreamPlayer`・`AudioStreamPlayer2D`・`AudioStreamPlayer3D`の標準AudioStreamインターフェースで、音量、バス、空間音響を扱えます。停止すると独立接続を切断し、再生すると新たに接続します。エラーは`stream.get_last_error()`で確認します。接続コードはリソースへ保存しません。シークと自動再接続は未対応です。

映像と接続を共有する場合は次のように設定します。

```gdscript
session.start_receiver(host, port, connection_code)
var audio := KyberAudioStream.new()
audio.set_session(session)
$AudioStreamPlayer.stream = audio
var video := KyberVideoStream.new()
video.audio_enabled = false
video.set_session(session)
$VideoStreamPlayer.stream = video
$AudioStreamPlayer.play()
$VideoStreamPlayer.play()
```

`set_session()`は受信開始後、再生開始前に呼びます。その接続の音声Mailboxを参照するため、セッションを再起動したら再設定します。1セッションで音声を消費できるプレイヤーは1つです。2つ目はエラーになり、PCMを分けて消費することはありません。別の音声プレイヤーを使う場合は、例のように動画リソースの音声を無効にします。映像は推定音声再生位置に追従します。音声プレイヤーの停止では共有セッションを切断しません。終了時に明示的に閉じてください。一時停止中も受信を続け、古い音声の保持量には上限があります。音声と映像の一時停止は独立しているため、必要に応じて両方を止めます。

音声ミキサーではKyberの`kyaudioreg`とGodotの`AudioStreamPlaybackResampled`を使います。音声スレッドからシーンのノードにはアクセスしません。既存KyberホストのOpus音声も同じ再生経路を利用します。設定は本ガイドのホスト受信の節を参照してください。

## 既存Kyberホストから音声を受信する

`KyberSession`でホストへ接続します。`connected`シグナルの後で、音声を有効にして配信を要求します。

```gdscript
session.start_stream(display_id, 1280, 720, 20000000, true)
var stream := KyberVideoStream.new()
stream.set_session(session)
$VideoStreamPlayer.stream = stream
$VideoStreamPlayer.play()
```

音声を別のプレイヤーへ出す場合は、本ガイドの`KyberAudioStream`共有例を使います。`connect_to_host()`で接続を作成してから`set_session()`を呼びます。別のプレイヤーが音声を消費する場合は、`KyberVideoStream.audio_enabled`を無効にします。独立した`KyberVideoStream`の`source = KyberHost`では、`audio_enabled`がホストへの音声要求を制御します。低レベルの`KyberClient`は内部のジェネレーターで再生します。`KyberAudioStream`単独の接続コードはアドオン用プロトコルです。ホスト接続には`set_session()`を使います。

認証、エンドポイントの合意、再接続にはSDKを使います。SDKの音声エンドポイントから共通のOpusデコーダーと`kyaudioreg`へ渡します。映像のPTSも保持し、推定音声再生位置に合わせて表示します。ホスト音声の要求はSDKのReliableエンドポイントを使用します。ファイル送信用の`transport_mode`は適用しません。音量・ミュートはSDKのコマンドではなく、Godotプレイヤーで制御します。

Opusを48 kHz・ステレオへ復号します。モノラル・ステレオの`OpusHead`（mapping family 0）、pre-skip、出力ゲインに対応します。パケット長は2.5〜120 msに対応します。その他のコーデックや多チャンネル配置はエラーにします。画面配信に伴う音声要求が対象で、ホストへの音声専用要求は未公開です。ホスト側の音声バックエンドが利用できない場合は、要求が拒否されることがあります。映像だけを受信する場合は`false`を指定します。

デスクトップ・モバイル共通の経路として実装しています。公式KyProtoの実音声エンドポイントからSDK転送アダプターを通す試験と、SDKコールバックのテスト用実装によるGodot再生・再接続を検証対象にしています。実際のKyber controllerとの相互運用を確認したものではありません。固定SDKのREADMEが参照する`kymux/examples`は、その版には存在しません。[Kyber for Unityのデモ](https://gitlab.com/mfkl/kyber-unity/-/blob/464b6dd69b3f69cce5bf445f55e3c5784b548d7e/README.md)は、Unityをホストのワーカーとしてcontroller・公式クライアントと組み合わせます。受信機能をUnityへ埋め込むデモではありません。Godotの接続先として使うには、その構成を別途ビルドする必要があります。

## 転送方式とプロトコル互換性

送信開始前に`KyberFileVideo.transport_mode`を指定します。

| 値 | 転送方式 | 動作 |
|---|---|---|
| `0` | Reliable（既定） | KyProtoのReliable映像・音声。欠落時の再送で遅延が増える場合があります。 |
| `1` | Unreliable | KyProtoのデータグラム。欠落したJPEGを飛ばし、音声の欠落にはOpusの損失補間を使います。 |
| `2` | Unreliable + FEC | KyProtoのRaptorQで映像・音声を復元し、復元できない音声には損失補間を使います。 |

公式SDKのドライバーを利用しており、FECの独自実装ではありません。復元には追加の帯域を使い、すべての欠落を回復できるわけではありません。映像と音声に同じ方式を適用します。RPCはReliableのデータエンドポイントを維持します。動画・Viewportデモに選択UIがあり、Viewportでは音声バスとマイクも選べます。

Reliableは既存の`certificate_hash:token`形式とリビジョン2のペイロード・エンドポイントを維持します。映像は0、完了通知は2、クロックは4、音声は6です。Unreliableは`k3:1:certificate_hash:token`、FECは`k3:2:certificate_hash:token`を使い、受信側がコードから方式を選びます。`k3`は接続コード形式の識別子であり、SDKのABIではありません。古い受信側は新形式を拒否します。直前のリビジョン2対応アドオンと接続する場合はReliableを使います。リビジョン1の映像専用送信も引き続き受信できます。公式Kyberホストとは別のアプリ用プロトコルです。

`audio_dropped_packets`はMailboxの上書きと、転送層が通知した音声欠落の合計です。FECで回復済みの欠落は含めません。既存の性能統計は`KyberSession`と`KyberFileVideo`で引き続き取得できます。
