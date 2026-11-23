<!-- SPDX-FileCopyrightText: 2026 KOGA Mitsuhiro -->
<!-- SPDX-License-Identifier: MIT -->

# 実装範囲と制約

[ドキュメント一覧](README_ja.md) | [English](overview.md)

このガイドでは、アドオンの実装範囲とKyber側に存在する未統合の機能を区別します。

## 組み込み方を選ぶ

| 目的 | GodotのAPI | ガイド |
| --- | --- | --- |
| シーンで映像を受信する | `VideoStreamPlayer`と`KyberVideoStream` | [映像](video_ja.md) |
| 接続を共有し、状態を確認する | `KyberSession` | [接続の共有](video_ja.md#接続を共有して入力と詳細状態を扱う) |
| 受信音声を再生する | `AudioStreamPlayer`と`KyberAudioStream` | [音声](audio_ja.md) |
| ファイルやViewportを送信する | `KyberFileVideo` | [キャプチャー](extensions_ja.md) |
| アプリ間でRPCを交換する | `KyberMultiplayerPeer` | [マルチプレイヤー](multiplayer_ja.md) |
| 既存のKyberホストから受信する | `KyberClient`またはホストセッション | [ホスト接続](host_ja.md) |

まだ導入していない場合は[インストール](../content/installation_ja.md)から始めます。上記のAPIを利用するためにSDKをビルドする必要はありません。

## 実装範囲

- `KyberVideoStream`による標準`VideoStreamPlayer`での受信
- `KyberSession`による接続共有、入力、詳細状態、統計の取得
- `KyberFileVideo`ノードによる動画ファイルの送信・受信、送信プレビュー、停止、ループ再生
- KyProtoを通信層とする`MultiplayerPeerExtension`、`KyberMultiplayerPeer`によるGodot RPC
- `KyberClient`ノードによる接続・切断と接続状態のシグナル
- Basic認証、システムのCAによるTLS検証、証明書フィンガープリントの指定
- ディスプレイ一覧の取得、H.264／Kymux配信の開始
- ソフトウェアデコードしたRGBA映像のテクスチャ化
- USB HIDキー、相対マウス移動、左右・中央ボタン、ホイール、標準ゲームパッドのボタン・軸の送信
- Viewport取得、複数クライアントへの動画配信、処理時間の統計、明示操作によるテキストのクリップボード交換
- ネットワーク接続なしで試せるローカル表示デモ

映像のみの場合は最新の1枚を保持します。音声再生時は、上限付きのキューに時刻付き映像を保持します。通信スレッドではGodot APIを呼びません。テクスチャ更新とシグナル通知はメインスレッドで処理します。

多くの映像経路にはCPUコピーとGPUへのアップロードが発生します。WindowsのD3D12では、H.264受信のGPU転送に対応します。対応範囲は[H.264](h264_ja.md)を参照してください。`KyberInputRelay`でタッチのマウス変換とゲームパッドの振動に対応します。公式ホストへのネイティブなマルチタッチとOIDCは未対応です。Windows・macOS・Linux・Android・iOSの動画ファイル音声は、Opus送信とGodotでの映像同期再生に対応します。音声バス・マイクのキャプチャと`KyberAudioStream`に対応します。既存ホストのOpus音声要求・受信も実装しています。実ホストとの相互運用は未検証です。[ライブ音声](audio_ja.md)を参照してください。

## プラットフォーム別のメディア処理

共通IFでは、`VideoDecoder`、`VideoSource`、圧縮パケット、RGBAフレームをGodotとKyber通信から分離しています。
WindowsはMedia FoundationでH.264の復号と動画ファイルの読み込みを行います。ファイル・Viewport配信ではH.264と高速化の方法を選択できます。D3D12受信ではCPUへの読み戻しを省けます。CompatibilityではCPU転送を維持します。符号化の入力画像はCPUメモリーを経由します。[H.264](h264_ja.md)を参照してください。
Linux x86_64はシステムのGStreamer 1.20以降でH.264の復号とファイル読み込みを行います。H.264受信には`h264parse`と、`avdec_h264`または`openh264dec`が必要です。ファイル形式の対応は、インストール済みのデマルチプレクサーとデコーダーに依存します。ファイル音声には`libgstpbutils-1.0`、`audioconvert`、`audioresample`と音声トラックのデコーダーも必要です。GStreamerは必要時に読み込みます。ライブラリやプラグインがなくても、RPCとJPEG受信は利用できます。
macOS arm64ではVideoToolboxでH.264を復号し、AVFoundationで動画を読み込みます。SDKは単一のdylibへ組み込み、OSのフレームワークは同梱しません。CPU上でRGBAへ変換します。Intel Macは未検証です。音声はAVFoundationで48 kHz・ステレオのPCMへ変換します。H.264配信の高速化は[H.264ガイド](h264_ja.md)を参照してください。GPU直接共有は未実装です。
Android arm64ではOS標準のMediaCodec、MediaExtractor、ImageReaderを使います（API 24以降）。H.264受信、動画ファイル、RPCをデスクトップと同じGodot APIで扱います。YUV420からRGBAへCPU上で変換し、対応ファイル形式は端末のコーデックに依存します。ハードウェアで設定できないH.264はAndroid標準のソフトウェアデコーダーへ切り替えます。HDR／BT.2020、バックグラウンド通信、GPU直接共有は未対応です。デモのネイティブファイル選択ではcontent URIをアプリ領域へコピーするため、動画1本分の空き容量が必要です。音声はモノラル・ステレオのPCM16または浮動小数点PCMを48 kHz・ステレオへ変換します。Android 14で検証し、その他のバージョンは未検証です。
iOS arm64ではmacOSと共通のバックエンドからAVFoundationとVideoToolboxを使います。SDKは`GodotKyber.framework`に組み込みます。AppleのフレームワークはOSのものを使います。音声はmacOSと共通のAVFoundation処理でPCMへ変換します。最低対象はiOS 15.0で、iOS 27.2のiPhone 15 Proで検証しています。デモはUIKitのネイティブファイル選択画面を使い、選んだ動画をアプリのキャッシュへコピーします。シミュレーター向けバイナリ、バックグラウンド通信、GPU直接共有は未実装です。App Store配布と他のiOSバージョンは未検証です。Webは対象外です。
`KyberSession`と`KyberFileVideo`の`get_media_capabilities()`で利用可否を取得できます。デコーダーがない場合も、JPEG受信とRPCは切り離して利用できます。
VLCとFFmpegは同梱しません。OSコーデックの利用は再配布する依存物を減らしますが、製品の特許上の義務がなくなることを保証するものではありません。

## Kyberに存在する未統合の機能

2026年11月21日時点の公式資料、0.29までのリリース情報、固定SDKを確認した比較です。統合上の差分を示すもので、実装予定を確約するロードマップではありません。Kyber側の対応状況はOS、バックエンド、SDKの版に依存します。Godot向けAPIがなくても、ホスト側だけで利用できる機能があります。既存ホストとの接続は引き続き未検証です。

| 機能 | Kyber側の対応 | Godotアドオンの現状 |
|---|---|---|
| 映像コーデック | クライアントSDKはH.264、HEVC、AV1の選択肢を定義しています。[クライアントSDK](https://gitlab.com/kyber/core/kyctl/-/blob/92c632f9e889917a34aaa7697fb88b4991054b1c/kyclient/src/lib.rs) | 既存ホストにはH.264を要求します。コーデック選択は未公開です。動画ファイルデモはJPEGまたはH.264を選べる別のアプリ専用プロトコルです。HEVCとAV1は未対応です。 |
| キャプチャ・ハードウェアエンコード | サーバーはNVENC、x264に対応し、AMF、QSV、VAAPIは実験的対応として記載されています。[サーバー設定](https://docs.kyber.tech/docs/GettingStarted/KyberServerConfig/) | `KyberFileVideo.start_viewport_sender()`でViewportを取得できます。CPUへの読み戻しと、JPEGまたはH.264の符号化を使います。H.264の高速化はOSとコーデックに依存します。[H.264](h264_ja.md)を参照してください。完全なKyberホスト機能はありません。外部ホストのハードウェアエンコードとGodotでのGPUテクスチャ共有は別の機能です。 |
| 音声 | 音声通信に対応し、SDKには再生機能とOpus対応があります。[0.29リリース情報](https://kyber.tech/article/v-029/) | Windows・macOS・Linux・Android・iOSの動画ファイルではPCM取得、Opus、KyProto音声通信、kyaudioreg、Godotの映像同期再生に対応します。マイク・音声バスのキャプチャと独立したKyberAudioStreamにも対応します。既存ホストのOpus音声受信も実装しています。実ホストとの相互運用は未検証です。 |
| 拡張入力 | ゲームパッドに対応し、入力SDKにはマウスホイールも定義されています。[Windows導入手順](https://docs.kyber.tech/docs/GettingStarted/Kyber-installation-notes-windows/)、[入力SDK](https://gitlab.com/kyber/core/kynput) | キーボード、相対マウス移動、3ボタン、ホイール、標準ゲームパッドのボタン・軸をSDKのパケットで転送します。`KyberInputRelay`でSDKの振動受信とタッチのマウス変換に対応します。Godot間では標準RPCでマルチタッチ・追加パドル・振動を交換します。公式ホストへのマルチタッチ・パドル入力は未対応です。[拡張入力](extensions_ja.md#拡張入力を転送する)を参照します。 |
| クリップボード | 公式クライアントはWindows・X11ホストとのクリップボード交換に対応します。[0.27リリース情報](https://kyber.tech/article/v-027/) | SDKのクリップボードパケットとGodotの`DisplayServer`を使うテキスト交換に対応します。読み取り、要求、OSへの反映は明示操作です。HTML、画像、自動同期は未対応です。[拡張機能のガイド](extensions_ja.md)を参照します。 |
| OIDC・セッション更新 | 公式クライアントはOIDCに対応し、0.29ではトークン交換・更新も追加されています。[0.26](https://kyber.tech/article/v-026/)、[0.29](https://kyber.tech/article/v-029/) | ホスト接続ではBasic認証情報とTLS設定を公開しています。OIDCログインやトークンの管理APIはありません。デモの接続コードは別の認証方式です。 |
| 複数のリモート受信者 | ホストはエンコード済みの映像を複数クライアントへ配信できます。[0.27リリース情報](https://kyber.tech/article/v-027/) | 動画ファイルとViewportは最大16の受信接続・接続処理を扱います。圧縮は共有し、送信は接続ごとに独立します。RPCは相手1台です。同じ`KyberSession`をローカルの複数プレイヤーで共有する機能とは異なります。公式の複数クライアント対応ホストへの参加は未検証です。 |
| 自動再接続 | 公式クライアントには自動リトライの設定があります。[0.27リリース情報](https://kyber.tech/article/v-027/) | 一部対応済みです。SDKホスト接続では`auto_reconnect`で公式SDKの再接続を有効にできます。動画ファイル通信とRPCは対象外です。[設定手順](video_ja.md#kyber-sdkの自動再接続を有効にする)を参照します。 |
| 詳細な遅延メトリクス | Webクライアントは処理段階ごとの遅延を表示できます。[0.27リリース情報](https://kyber.tech/article/v-027/) | 処理段階の所要時間、フレーム間隔、キュー待ち時間、送信元から復号完了までの推定経過時間を公開します。動画ファイル／Viewport受信は公式KyProtoのクロック同期を使います。既存ホストの各段階の計測と物理画面の表示遅延は未対応です。 |
| 別の通信方式 | 0.29にはネイティブWebSocket対応が記載されています。[0.29変更履歴](https://docs.kyber.tech/blog/) | WebSocketの選択やGodot Webエクスポートとの統合は未実装です。独自の動画・RPC経路はQUICを使います。 |
| ホスト管理 | 稼働状況を知らせるハートビートや、デスクトップ用の`kyber://`リンクがあります。[0.29リリース情報](https://kyber.tech/article/v-029/) | エージェント管理やURLハンドラーは未実装です。管理ポータル自体は公式にも将来の機能として記載されており、利用可能なアドオン機能には含めません。 |
| 仮想ディスプレイ | Windowsホストには仮想モニター設定がありますが、資料では未公開のドライバーが必要とされています。[サーバー設定](https://docs.kyber.tech/docs/GettingStarted/KyberServerConfig/) | 既存ディスプレイの一覧取得・選択に対応します。仮想ディスプレイの作成やドライバー管理は未実装です。 |

[公式の技術概要](https://kyber.tech/technology/)には、タッチ・ジェスチャー、映像と同期したセンサーデータ・メタデータ、適応的な画質制御、USB転送などのデータチャネル用途も記載されています。このアドオンでは、それらの統合や設定APIを公開していません。ファイル・Viewport・ライブ音声には`transport_mode`でKyProtoのUnreliableとRaptorQ FECを選択できます。RPCはReliableのままです。製品全体の説明だけでは、固定した公開SDKで各機能を利用できることまでは確認できません。Godot RPCはアプリ独自のメッセージを運ぶ機能であり、USB転送、汎用ファイル転送、センサーデータ連携を実装済みという意味ではありません。

動画ファイル／Viewportの受信では、復号が遅れる場合に圧縮済みの最新1枚を残します。クロック同期で共通の時刻基準を取得し、古い映像を除外します。ただし、複数端末の同時表示は保証しません。[計測とクリップボードの詳細](extensions_ja.md)を参照します。

## 参照実装とライセンス

Godotへの統合とドキュメントの構成では[godot-ndi](https://github.com/unvermuthet/godot-ndi)を参照しています。Kyber固有の制御や詳細状態は共有セッションから利用できます。

構成設計では[Kyber for Unity](https://gitlab.com/mfkl/kyber-unity)を参照しました。UnityのD3D11送信実装は流用していません。受信側は[Kyber SDK](https://gitlab.com/kyber/core/kysdk)の`kyclient`を使用します。

独自ファイルはMITで提供します。Kyber由来の定義はビルド時にGit管理外のSDKディレクトリへ抽出し、元のライセンスを維持します。公開版SDKを含むバイナリは結合物としてAGPLで配布します。Kyberの商用契約が対象コードの別条件での利用を許可する場合があります。本プロジェクトは商用利用権を付与しません。[適用範囲](../NOTICE.md)を参照します。ほかの依存物には各ライセンスが適用されます。
