<!-- SPDX-FileCopyrightText: 2026 KOGA Mitsuhiro -->
<!-- SPDX-License-Identifier: MIT -->

# Kyber SDK統合仕様

[ドキュメント一覧](README_ja.md) | [Documentation index](README.md)

## 固定したソース

Kyber 0.29系SDKを拡張へ組み込みます。

| リポジトリ | コミット |
|---|---|
| kysdk | `ebca32a08f6488ea23750943ded52119efd584d6` |
| kyctl | `92c632f9e889917a34aaa7697fb88b4991054b1c` |

kymux、kynput、kyutilはkysdkのサブモジュールで固定します。
`prepare_sdk.py`はバージョンを確認してパッチを適用します。再実行しても変更は重複しません。
再生イベント・リスナー・ウィンドウハンドルの定義は、KyCtlの固定コミットから抽出します。
抽出前にファイルハッシュを確認し、独自のMITアダプターと組み合わせて`.deps/`内へ生成します。
生成物には元の著作権・ライセンス表記を維持します。生成結果の検証は`python scripts/test_prepare_sdk.py`で実行します。

## 共通メディアIF

`media/`の`kyber-media`クレートはGodotとKyber通信に依存しません。

| IF | 役割 |
|---|---|
| `VideoDecoder` | 圧縮フレームの復号、残りフレームの取り出し、リセット |
| `VideoSource` | 動画ファイルから次の映像フレームを読み込み |
| `AudioSource` / `open_audio()` | 最初の音声トラックを読み込み。音声トラックがない場合は`None` |
| `AudioFrame` | 元のPTSと48 kHz・ステレオの浮動小数点PCM |
| `EncodedPacket` | Annex B形式のH.264アクセスユニット、マイクロ秒のPTS、キーフレーム情報 |
| `VideoFrame` | 表示寸法、PTS、上から下へ並べたRGBA8データ |
| `H264Stream` | 設定パケットの保持、欠落後のキーフレーム待ち、デコーダーへの受け渡し |
| `capabilities()` | バックエンド名、H.264復号、ファイル読み込みの可否と理由 |

WindowsではMedia FoundationのH.264デコーダー、色変換、Source Readerを使用します。
デコーダーの解像度変更、表示領域のクロップ、行ストライド、上下方向を処理します。
ファイル・Viewport配信にはH.264の符号化と復号、高速化の選択APIがあります。WindowsのD3D12受信ではGPU転送に対応します。その他の経路ではCPUメモリーを経由します。[H.264](h264_ja.md)を参照してください。既存ホストのOpus音声はGodotで再生します。
LinuxはシステムのGStreamerを必要時に読み込み、H.264復号とファイル読み込みを行います。GStreamerのライブラリやコーデックが不足しても、RPCとJPEG受信は利用できます。macOSではVideoToolboxとAVFoundationを利用します。Android arm64ではMediaCodec、MediaExtractor、ImageReaderを利用します。iOS arm64はmacOSと共通のAVFoundation・VideoToolbox処理を使います。
ファイル音声も各OSのメディアAPIで復号します。LinuxはGStreamerの音声変換プラグイン、Androidはモノラル・ステレオPCMの変換処理を使います。

## SDKとの接続

`sdk/native_desktop.rs`をSDKのKymuxメディア経路へ配置します。
VLC向けのIPCを経由せず、公式SDKの映像エンドポイントから圧縮パケットを受け取ります。
接続・認証・TLS・入力通信・自動再接続は公式SDKの処理を維持します。
SDKから再接続成功の通知を受けたGodot側は、最後の配信設定を再要求します。

映像はH.264、回転なしに対応します。未対応のコーデックや回転はエラーとして通知します。
欠落通知後はデコーダーをリセットし、次のキーフレームから再開します。
動画ファイルデモは別経路です。OSでファイルを読み込み、JPEGまたはH.264映像とOpus音声を同一接続内の別々のKyProtoエンドポイントへ送ります。Viewport送信にはGodot音声バスまたはマイクの音声を追加できます。

## 内部ブリッジ

`sdk/godot_bridge.rs`と`src/backend.rs`は同じビルドでリンクします。
外部SDKのパス指定やABI番号はありません。内部のC形式コールバックを使います。

| 関数 | 内容 |
|---|---|
| `kyclient_godot_create` | 設定JSONとコールバックを受け取り、接続を要求 |
| `kyclient_godot_start` | H.264配信と任意のOpus音声を要求。映像を復号できない場合は`-3`を返す |
| `kyclient_godot_input` | キー、相対マウス移動、ボタン、ホイール、ゲームパッドを送信 |
| `kyclient_godot_clipboard` | 提供するテキストの設定と相手のテキスト取得要求 |
| `kyclient_godot_destroy` | 停止とワーカー終了を待って破棄 |

Godotの公開APIでは数値エラーを説明文へ変換します。既存の接続状態、フレーム数、ドロップ数の表示は維持します。
`get_media_capabilities()`はOSのメディア機能を返し、接続状態に応じた`get_capabilities()`とは区別します。
`get_media_capabilities()`は`audio_playback`と`file_audio`を返します。`file_audio`はプラットフォーム用の音声読み込み実装があることを示し、個々のコーデックの利用可否を保証しません。`KyberSession.get_capabilities()`の`godot_audio`は`true`、`native_audio_request`も`true`です。既存ホストのOpus音声を要求し、共通デコーダーへ渡します。

入力用には`kynput_get_external_client_config()`を追加します。
OSのグローバル入力フックやダミー入力生成器は起動しません。
USB HID Usage IDは`keycode`でKyberのWindows Scan Code Set 1形式へ変換します。

## 寿命とスレッド

```text
公式Kyber SDKの映像エンドポイント
  → 上限4パケットのキュー
  → 専用スレッド上のVideoDecoder
  → RGBAコールバック
  → 最新1枚のMailbox
  → Godotメインスレッド上のImageTexture
```

COMとメディアオブジェクトは専用スレッドで作成・利用・破棄します。
ファイル読み込みも専用スレッドを使い、映像・音声イベントのキューは2件に制限します。音声は20 ms単位に分割します。JPEG圧縮は独立したワーカーと上限1フレームのキューで処理します。
ファイル受信では、ネットワーク読み出しとJPEG復号を分離します。音声再生時は最大24枚の時刻付き映像から、推定音声再生位置に合うフレームを選びます。音声バッファと再生速度差の調整には`kyaudioreg`を使います。Godotへの出力は`VideoStreamPlayback.mix_audio()`を使用します。
`close()`はSDK停止とデコードワーカーの終了を待ち、コールバック状態を最後に解放します。
同期停止のため、終了処理中はGodot側も待機します。長時間運用時の停止時間の上限は未検証です。

## ビルドと配布

`scripts/build.ps1 -Release`でSDKの取得からデモへの配置まで実行します。
WindowsのC++ビルドツール、Windows SDK、CMakeを使用します。WSLやVLCビルドは不要です。
SDK、入力処理、Opus、C/C++ランタイムは`godot_kyber.dll`へ静的リンクします。
Media FoundationだけはOSのライブラリをSystem32から解決します。未導入でも拡張全体のロードを妨げない構成です。
VLC、FFmpeg、外部SDKのDLLは同梱しません。

ビルドスクリプトは以前のマニフェストに記録されたVLCファイルを配置先から除去します。
ライセンス文書とエクスポート設定も再生成します。利用者は`addons/kyber`全体をコピーします。

## 検証範囲

RustテストではMedia FoundationとGStreamerの実復号、色・上下方向・PTS、解像度変更、クロップ、リセットを確認します。
GUTと2プロセステストでは動画ファイルの送受信とRPCを確認します。
SDKの起動・接続失敗・終了と、Godotで書き出した実行ファイルも検証対象です。

`mock-sdk`はテスト専用です。既定の`native-sdk`と同時に使用できず、配布スクリプトでも拒否します。
実Kyberホストでの認証・映像・入力・自動再接続は未検証です。Windows、macOS、Linux、Android、iOSの映像・音声・RPCの検証範囲は開発ガイドを参照します。追加した入力・クリップボードの実ホスト接続とモバイル実機検証は未実施です。

## 拡張入力と時計のパッチ

`sdk/kynput_clipboard.rs`と`sdk/kynput_clipboard_wrapper.rs`はSDK本来のテキスト用クリップボードパケットを公開します。独自のクリップボード通信形式は追加しません。Godotへの通知は既存のイベントコールバックを使い、JSONエスケープ後のテキストが収まるよう上限を512KiBにしています。テキスト本体の上限は60KiBです。

ゲームパッドの右スティック押し込みが右肩ボタンへ変換される箇所をビルド時に補正します。WindowsのSDK単調増加時計では、長期間起動した環境で乗算がオーバーフローしないよう中間計算を128ビットへ変更します。

## 既存ホストの音声アダプター

`kyclient_godot_create()`に音声パケット用コールバックを追加しています。映像コールバックにはPTSを追加しています。`sdk/`と`src/backend.rs`は同じビルドで更新する内部IFで、外部SDK DLL向けのABI番号は設けません。

`sdk/audio_forward.rs`をSDKへ配置し、公式の音声エンドポイントからAVPacketのヘッダーとペイロードを渡します。音声ペイロードの上限は4000バイトです。Godot側では共通のOpusデコーダーでPCMへ変換し、上限付きMailboxへ渡します。SDKワーカーはGodotのノードを操作しません。停止時は音声転送タスクも終了させ、SDKの破棄完了までコールバック用データを保持します。

音声要求は画面配信に付随し、Opusのモノラル・ステレオが対象です。公式ホストとの相互運用は未検証です。手順と検証範囲は[ライブ音声](audio_ja.md#既存kyberホストから音声を受信する)を参照してください。
