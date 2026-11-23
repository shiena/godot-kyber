<!-- SPDX-FileCopyrightText: 2026 KOGA Mitsuhiro -->
<!-- SPDX-License-Identifier: MIT -->

# インストール

## パッケージを選ぶ

[GitHub Releases](https://github.com/shiena/godot-kyber/releases/latest)からZIPをダウンロードします。

| パッケージ | 用途 |
| --- | --- |
| アドオンのみのZIP | 既存のGodotプロジェクトに送受信機能を追加する |
| デモ付きZIP | 同梱の送信・受信・RPCシーンを試す |

アドオンはGodot 4.2以上（4.x）に対応しています。デモ全体はGodot 4.7.2で検証しています。このサイトの操作手順を試す場合は4.7.2を使用します。

## アドオンをコピーする

1. アドオンのみのZIPを展開します。
2. `addons/kyber`フォルダー全体を自分のプロジェクトへコピーします。
3. `bin/`、`licenses/`、`kyber.gdextension`が揃っていることを確認します。
4. Godotでプロジェクトを開きます。すでに開いている場合はエディタを再起動します。

GDExtensionは自動で読み込まれます。エディタープラグインを有効にする操作は不要です。

<!-- capture: addon-files-ja | Godotのファイルシステムドック。res://addons/kyber内のbin、licenses、icon.svg、kyber.gdextensionが見える状態。 -->

## 読み込みを確認する

エディタの出力に拡張の読み込みエラーがないことを確認します。スクリプトエディタで`KyberSession`が認識されたら、[はじめての送受信](getting-started_ja.md)へ進みます。

読み込めない場合は、OSとCPUアーキテクチャーに合うバイナリがあるか確認します。`.gdextension`ファイルだけでは動作しません。

## プラットフォームを準備する

Windows、macOS、Android、iOSではOSのメディア機能を利用します。LinuxにはGStreamerとコーデックプラグインが必要です。制限は[対応機能](../../docs/overview_ja.md)、ビルドツール・モバイルへのエクスポート・署名設定は[開発ガイド](../../docs/development_ja.md)を参照してください。

Kyber SDKは組み込み済みです。別途インストールしたり、ライブラリのパスを設定したりする必要はありません。Webへのエクスポートには対応していません。

## 既存のアドオンを更新する

プロジェクトを停止し、エディタを閉じてからネイティブライブラリを置き換えます。ローカルの変更をバックアップし、同じリリースのパッケージ全体でアドオンを更新します。送信側と受信側のバージョンの組み合わせは、[転送方式の互換性](../../docs/audio_ja.md)を確認してください。

プロジェクトを配布する前に、[バイナリ向けライセンス案内](../../BINARY_LICENSE.md)も確認してください。
