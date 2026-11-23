<!-- SPDX-FileCopyrightText: 2026 KOGA Mitsuhiro -->
<!-- SPDX-License-Identifier: MIT -->

# はじめての送受信

同梱のデモを2つ起動して、動画を送受信します。まず同じPCで確認してから、ネットワーク上の別端末へ接続します。

## デモをダウンロードする

[GitHub Releases](https://github.com/shiena/godot-kyber/releases/latest)から、**demo**のZIPをダウンロードします。展開後、`project.godot`をGodot **4.7.2**で開きます。このバージョンでデモ全体を検証しています。

アドオンはGodot 4.2以上（4.x）に対応しています。デモとテストにはGodot 4.7.2を使います。モバイルでは、プラットフォームのエクスポートツールと署名設定も必要です。

## 2つのインスタンスを接続する

1. デモを2つ起動します。
2. 送信側で動画ファイルの送信モードを選び、ネイティブダイアログから動画を開きます。
3. 送信を開始し、接続コードをコピーします。
4. 受信側で受信モードを選びます。同じPCで試す場合は、接続先を`127.0.0.1`にし、送信側と同じポートを指定します。
5. 接続コードを貼り付けて接続します。両方の画面に動画が表示されます。
6. 確認が終わったら送受信を停止します。

LANで接続する場合、送信側は`0.0.0.0`で待ち受けます。受信側には送信端末のLANアドレスを指定し、使用するUDPポートをファイアウォールで許可します。接続コードは送信セッションごとに生成されます。

<!-- capture: file-sender | 送信側のプレビュー（接続コードは非表示） -->

<!-- capture: file-receiver | 受信映像とフレーム統計 -->

## 自分のプロジェクトへ導入する

同じリリースから**アドオンのみ**のZIPをダウンロードします。`addons/kyber`を、`bin/`と`licenses/`を含めてプロジェクトへコピーします。プロジェクトを開き直すとGDExtensionが読み込まれます。エディタープラグインの有効化は不要です。

```text
my-project/
├── project.godot
└── addons/
    └── kyber/
        ├── kyber.gdextension
        ├── icon.svg
        ├── bin/
        └── licenses/
```

## GodotのAPIを選ぶ

| やりたいこと | 使用するAPI |
| --- | --- |
| 受信映像の再生 | `VideoStreamPlayer`と`KyberVideoStream` |
| 接続の共有・状態の確認 | `KyberSession` |
| 受信音声の再生 | `AudioStreamPlayer`と`KyberAudioStream` |
| Viewportの送信 | `KyberFileVideo.start_viewport_sender()` |
| GodotのRPC | `KyberMultiplayerPeer` |

使い方と制限は、[映像ガイド](../../docs/video_ja.md)、[音声ガイド](../../docs/audio_ja.md)、[RPCガイド](../../docs/multiplayer_ja.md)を参照してください。

## プラットフォームの要件を確認する

WindowsとAppleプラットフォームではOSのメディア機能、AndroidではMediaCodecを使用します。LinuxではGStreamerと対応するコーデックプラグインが必要です。Kyber SDKの別途インストールは不要です。Webには対応していません。

[対応機能と制限](../../docs/overview_ja.md)と[開発ガイド](../../docs/development_ja.md)も確認してください。

## 配布前に確認する

このリポジトリのソースコードはMITライセンスです。公開バイナリはKyber SDKとの結合物としてAGPL-3.0-or-laterで配布します。対象のKyberコードには、条件を満たすKyber SASの商用契約を適用できる場合があります。ほかの依存物には各ライセンスが適用されます。

アドオンやデモを再配布する前に、[バイナリ向けライセンス案内（英語）](../../BINARY_LICENSE.md)を確認してください。
