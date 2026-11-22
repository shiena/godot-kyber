<!-- SPDX-FileCopyrightText: 2026 KOGA Mitsuhiro -->
<!-- SPDX-License-Identifier: MIT -->

# godot-kyber

[English](README.md) | 日本語

> [Kyber](https://kyber.tech/)の映像通信とマルチプレイヤーRPCをGodotへ統合するGDExtensionです。

`KyberVideoStream`を設定すると、標準の`VideoStreamPlayer`で映像を受信できます。`KyberSession`ノードを追加すると、同じ接続で入力操作、詳細状態、統計を扱えます。`KyberMultiplayerPeer`はGodotのマルチプレイヤーAPIでRPCに利用できます。

[デモプロジェクト](https://github.com/shiena/godot-kyber/tree/main/demo)には、動画ファイルの映像・音声送受信、Viewport送信、既存ホストからの受信、RPCチャットが含まれます。映像の縦横比を維持し、表示の一時停止とは別に通信状態を確認できます。

## インストール

- 拡張の対象APIはGodot 4.2です。デモ全体とテストにはGodot 4.7.2を使用します。
- Windows/Linux x86_64、macOS arm64、Android arm64、iOS arm64向けメディアバックエンドを実装しています。Webは対象外です。
- Windowsでの動画受信とファイル読み込みにはMedia Foundationを使います。FFmpegコマンドは不要です。
- macOSではOS標準のVideoToolboxとAVFoundationを使います。
- iOSでも同じAppleのメディアフレームワークを使います。[iPhoneのビルド・テスト手順](docs/development_ja.md#iphone向けにビルドしてテストする)を参照します。
- AndroidではOS標準のMediaCodecとMediaExtractorを使います。[Androidのビルドと実機テスト](docs/development_ja.md#androidでビルドして実機テストする)を参照します。
- LinuxではシステムのGStreamerとコーデックプラグインを使います。[Linuxのセットアップ](docs/development_ja.md#linuxでビルドする)を参照します。
- Kyber SDKは拡張に組み込みます。VLC・FFmpegライブラリの同梱、SDKの別途インストール、パス指定は不要です。実ホストとの接続は未検証です。

後述の手順でソースからビルドし、`demo/addons/kyber`を利用先プロジェクトの`addons/kyber`へコピーします。エディタープラグインの有効化は不要です。

ビルド済みのWindows DLLは、成功した[GitHub Actionsの実行結果](https://github.com/shiena/godot-kyber/actions/workflows/ci.yaml)から取得できます。`godot-kyber-windows-x86_64` artifact内の配布ZIPを展開し、`addons/kyber`を利用先へコピーします。

Releaseワークフローは5プラットフォームを含むアドオンZIPとデモZIPを生成します。[リリース手順](docs/development_ja.md#アドオンとデモのパッケージを公開する)を参照します。モバイルへの導入には各プラットフォームのエクスポート環境と署名が必要です。

## デモを試す

拡張をビルドした後、`demo/project.godot`を開きます。2つのインスタンスを起動すると、動画ファイルの送受信やRPCメッセージの交換を試せます。動画の選択にはOS標準のファイルダイアログを使用します。

ネットワークを使わずに表示を確認する場合は、［既存Kyberサーバー受信］タブの［ローカル表示デモ］を選択します。

接続手順は[映像ガイド](docs/video_ja.md)と[RPCガイド](docs/multiplayer_ja.md)を参照します。

## ドキュメント

[ドキュメント一覧](docs/README_ja.md)から各ガイドを参照できます。

- [映像再生・共有セッション・入力操作](docs/video_ja.md)
- [GodotのマルチプレイヤーRPC](docs/multiplayer_ja.md)
- [既存Kyberホストとの接続](docs/host_ja.md)
- [実装範囲と制約](docs/overview_ja.md)
- [ビルド・テスト・CI・配布](docs/development_ja.md)

Viewport送信と複数クライアントへの動画配信に対応しています。[キャプチャ・入力・性能計測](docs/extensions_ja.md)を参照します。Windows・macOS・Linux・Android・iOSでは動画ファイルの音声をOpusで送信し、Godotの動画プレイヤーで同期再生できます。[ライブ音声](docs/audio_ja.md)では音声バス、マイク、`KyberAudioStream`を利用できます。映像・音声にはReliable・Unreliable・FECを選択できます。RPCは現在、クライアント1台、信頼性のある転送、チャネル0に対応します。詳細な制約は各ガイドに記載しています。

## ソースからビルドする

[開発ガイド](docs/development_ja.md)に従ってWindowsのビルドツールを用意します。リポジトリのルートで実行します。

```powershell
pwsh -File scripts/build.ps1 -Release
```

固定SDKの取得、依存ライブラリのビルド、アドオン一式の配置をまとめて実行します。テストや配布ZIPの作成手順は[開発ガイド](docs/development_ja.md)を参照します。

Linuxでは[セットアップ手順](docs/development_ja.md#linuxでビルドする)に従って依存パッケージを導入し、`bash scripts/build.sh --release`を実行します。

macOSでも同じビルドコマンドを使います。[macOSのセットアップ手順](docs/development_ja.md#macosでビルドする)を参照します。

## ライセンス

SDKの抽出スクリプトを含む独自実装は[MIT](LICENSE)で提供します。ビルド準備時に、固定したKyber SDKから互換定義を抽出します。生成先はGit管理外のビルドディレクトリです。SDKと生成モジュールは元のライセンスを維持します。ファイルごとのライセンスは先頭の表記と[REUSE.toml](REUSE.toml)で確認できます。

公開版Kyber SDKを組み込んだバイナリは、結合物としてAGPL-3.0-or-laterで配布します。配布者は完全な「対応するソース」を提供し、適用されるライセンス表示を維持する必要があります。独自ファイルのMIT表記は、結合バイナリ全体をMITにするものではありません。

[Kyberの商用ライセンス](https://kyber.tech/)を取得している場合は、契約対象のKyberコードにその契約を適用できます。改変、ビルド、再配布は契約の許可範囲に従います。本プロジェクトはKyberの商用利用権を付与しません。ほかの依存物には各ライセンスが適用されます。詳細は[ライセンスの適用範囲と第三者表示](NOTICE.md)を参照します。

AGPLとKyber SASの商用契約による利用の選択肢は、[バイナリ向けライセンス案内](BINARY_LICENSE.md)を参照してください。SDK公式の`COPYING.md`とAGPL本文もビルド成果物に同梱します。商用契約が対象とするバージョン・改変・利用・配布の範囲に従う必要があります。
