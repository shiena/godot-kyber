<!-- SPDX-FileCopyrightText: 2026 KOGA Mitsuhiro -->
<!-- SPDX-License-Identifier: MIT -->

# ビルド・検証・配布

[ドキュメント一覧](README_ja.md) | [English](development.md)

コマンドはリポジトリのルートで実行します。コード例のパスはルートからの相対パスです。

## このガイドを使う場面

アドオンの変更、ネイティブライブラリのビルド、リリースの作成を行う開発者向けの手順です。配布済みアドオンの利用やGodotでのデモ実行は、[インストール](../content/installation_ja.md)と[映像ガイド](video_ja.md)を参照してください。

## ビルド対象を選ぶ

| 対象 | 手順 |
| --- | --- |
| Windows | [ローカルでビルド・実行](#ローカル表示デモを動かす) |
| Linux | [Linuxのビルド](#linuxでビルドする) |
| macOS | [macOSのビルド](#macosでビルドする) |
| Android | このページのAndroidビルド・実機テストの節を参照 |
| iOS | このページのiOSビルド・実機テストの節を参照 |

対象環境のツールチェーンを使用します。モバイルではエクスポートツールと署名・端末設定も必要です。ネイティブコードを変更した後は、ライブラリを再ビルドしてからデモやテストを実行します。

## ローカル表示デモを動かす

Windows x86_64、Rust 1.94以降、Windows SDK付きのVisual Studio C++ビルドツール、Python 3、Git、CMake、LLVM（libclang）を使用します。
WSLやコーデックの別ビルドは不要です。Godot APIの対象は4.2のまま維持します。テスト全体はGodot 4.7.2で実行します。

1. リポジトリのルートでビルドします。

   ```powershell
   pwsh -File scripts/build.ps1
   ```

2. Godotで`demo/project.godot`を開きます。
3. プロジェクトを実行します。
4. ［Existing Kyber server］タブで［Local display demo］をクリックします。

動くグラデーションが表示されます。このデモはネットワークを使用しません。ビルド済みDLLは`demo/addons/kyber/bin/godot_kyber.dll`に配置されます。別のプロジェクトでは`demo/addons/kyber`を`addons/kyber`へコピーします。エディタープラグインの有効化は不要です。

## Linuxでビルドする

Ubuntu 24.04 x86_64、Rust 1.96.0、Python 3.11以降を使用します。WSLのUbuntuでビルドし、別のLinux端末へ配置できます。

```bash
sudo bash scripts/install_linux_deps.sh
bash scripts/build.sh --release
```

インストーラーはコンパイラー、SDKの依存ライブラリ、GStreamerの開発パッケージ、テスト用コーデックを導入します。SDKは`libgodot_kyber.so`へ組み込みます。
実行先にはGStreamer 1.20以降、app/videoライブラリ、コンテナープラグイン、利用するデコーダーが必要です。これらはアドオンへ同梱しません。
H.264受信には`h264parse`と、`avdec_h264`または`openh264dec`を使います。ファイル読み込みには`uridecodebin`とインストール済みプラグインを使います。コーデックの不足は`get_media_capabilities()`で確認できます。
RPCとJPEG受信はGStreamerなしで利用できます。ファイル音声には音声デコーダー、`libgstpbutils-1.0`、`audioconvert`、`audioresample`が必要です。H.264配信の高速化は[H.264ガイド](h264_ja.md)を参照してください。GPU直接共有は未実装です。

Linuxの検証と配布用アーカイブの作成を実行します。

```bash
python3 scripts/install_godot.py --destination "$HOME/.local/godot/4.7.2-stable"
export GODOT="$HOME/.local/godot/4.7.2-stable/Godot_v4.7.2-stable_linux.x86_64"
bash scripts/test_linux.sh
python3 scripts/package_linux.py
```

出力先は`target/packages/godot-kyber-<version>-linux-x86_64.tar.gz`です。アーカイブ内の`addons/kyber`を利用先へコピーします。
FFmpegコマンドはテスト素材の生成にのみ使います。LinuxではFFmpegベースのシステムGStreamerプラグインを利用できますが、配布物には同梱しません。
空き容量が少ない環境では、検証前に`CARGO_PROFILE_DEV_DEBUG=0`、`CARGO_PROFILE_TEST_DEBUG=0`、`CARGO_INCREMENTAL=0`を設定します。

2026年11月22日にWSLのUbuntu 24.04とSteam Deckで検証しました。DeckはSteamOS 3.8.28、GStreamer 1.26.4、Godot 4.7.2を使用しました。
初期の映像検証では、Deckでメディアの実動作テスト5件、GUT 44件・391アサーション、別プロセス間の映像・RPCテストが成功しました。
SDKを組み込んだビルドで、起動・接続失敗・終了も確認しました。Waylandの2ウィンドウで送信プレビューと受信映像を描画しました。この初期検証は映像のみを対象にしました。追加した音声の検証結果は後述します。
既存Kyberホストへの接続、Steamのゲームモード、ネイティブファイル選択操作は未検証です。デモの`use_native_dialog = true`は維持しています。
Ubuntu 24.04で作る配布物は同環境のシステムABIを対象とします。古いディストリビューションは未検証です。

## macOSでビルドする

Apple Silicon搭載Mac、Xcodeのコマンドラインツール、Rust 1.96.0、Python 3.11以降、CMake、Gitを使用します。リポジトリのルートで`bash scripts/build.sh --release`を実行します。
SDKは`libgodot_kyber.dylib`へ組み込み、Appleのシステムフレームワークへリンクします。実行時にコーデックの追加インストールは不要です。
HomebrewでRustを導入した場合は、コマンドのディレクトリを`PATH`へ追加します。

```bash
export PATH="$(brew --prefix rustup)/bin:$PATH"
```

検証では素材生成用のFFmpegを用意してから、次のコマンドを実行します。

```bash
export GODOT="$HOME/.local/godot/4.7.2-stable/Godot.app/Contents/MacOS/Godot"
bash scripts/test_macos.sh
python3 scripts/package_macos.py
```

配布物は`target/packages/godot-kyber-<version>-macos-arm64.tar.gz`です。単一アーキテクチャーのライブラリを含み、Universal Binaryではありません。
プロジェクトへの組み込み用の配布物です。エクスポートするアプリの署名と公証はアプリ側で行います。Intel Macは未検証です。
H.264受信にはVideoToolbox、ファイル読み込みにはAVFoundationを使います。対応形式はOSに依存します。ファイル音声はAVFoundationで読み込みます。GPU直接共有は未実装です。
macOSではCargo設定でRustのMach-Oシンボル削除を無効にしています。[Rust 1.96のLINKEDIT配置不具合](https://github.com/rust-lang/rust/issues/157750)によるロード失敗を防ぐためです。

2026年11月22日にmacOS 27.2.1のarm64 MacとGodot 4.7.2で検証しました。
初期の映像検証では、fmt、Clippy、Rustテスト8件、GUT 44件・391アサーション、別プロセス間の映像・RPC、SDKの起動・接続失敗・終了が成功しました。
Windows→MacとMac→Windowsでは、両端のデモ画面で映像を目視確認しました。
これはデモのJPEG/QUICプロトコルとOSのファイル読み込みを検証した結果です。既存Kyberホストとの相互接続は未検証です。

## 検証する

`demo/project.godot`は手動確認と自動テストを兼ねたプロジェクトです。[GUT 9.6.1](https://github.com/bitwes/Gut/tree/v9.6.1)を同梱しています。

### コマンドで一括実行する

FFmpegを用意し、リポジトリのルートで実行します。Windowsで再ビルドするときは、このプロジェクトのGodotエディターと実行中のゲームを終了しておきます。

```powershell
pwsh -File scripts/test.ps1
```

既定では`%USERPROFILE%/.local/godot/4.7.2-stable`のコンソール版を使用します。環境変数`GODOT`、または`-Godot`で変更できます。`-Godot`には実行ファイルか4.7.2のフォルダーを指定します。

```powershell
pwsh -File scripts/test.ps1 -Godot "C:/Tools/Godot/4.7.2-stable"
```

Rustのフォーマット、Clippy、ユニットテスト、Media Foundationの実デコードテストを実行します。続いてDLLをビルドし、GUTをヘッドレスで実行します。結果は`target/test-results/gut.xml`へ出力します。失敗、スキップ、テスト未検出、テスト数の不足は終了コード1で通知します。

### エディターでテストする

1. 初回は`pwsh -File scripts/test.ps1 -PrepareOnly`でテスト用拡張を準備します。一括実行済みの場合は不要です。
2. Godot 4.7.2で`demo/project.godot`を開きます。
3. `tests/test_runner.tscn`を開きます。
4. F6キーで現在のシーンを実行します。

GUTの画面にテスト結果が表示されます。F8キーで終了します。このシーンはCLIと同じ`.gutconfig.json`を使用します。

Godot下部の［GUT］パネルも使用できます。初回はパネルの設定に`res://tests/unit`と`res://tests/integration`を追加します。パネルの設定はGUTがユーザーデータ領域へ保存します。

### テスト範囲

| テスト | 件数 | 内容 |
|---|---:|---|
| Godot API | 8 | 初期状態、ローカル映像、テクスチャ再利用、切断、エラー通知、入力拒否 |
| デモシーン | 2 | TextureRectと3Dマテリアルへの反映・切断時の消去 |
| SDK内部ブリッジ結合 | 8 | 接続イベント、映像の画素値、Godot入力イベント、入力範囲、5回の再接続、終了時のワーカー停止 |
| 動画ファイル送受信 | 5 | 実QUIC通信での画素値、動画終了、認証拒否と再接続、不正な設定、モード切替 |
| マルチプレイヤーピア | 4 | パケットの送信先・上限、SceneMultiplayerのRPC、接続シグナル、認証拒否、接続受付の停止、終了処理 |
| 標準動画プレイヤー | 6 | 一時停止・再開、テクスチャの同一性、リソース保存、認証情報の保存除外、SDKテスト用実装、設定検証、受信画素、終了シグナル |
| 共有セッション | 14 | プレイヤーの独立性、同一SDK接続での入力、コールバックからの操作、エラー復旧、ノードの寿命、デモの詳細状態、縦横比、SDK再接続通知、配信復元、キャンセル、拡張入力、クリップボード、性能統計 |

Godot 4.7.2でGUT 47件の成功を確認しました。動画テストはFFmpegで生成した赤い動画を実際のKyProto / QUIC接続へ送り、受信テクスチャの画素値を検証します。GUTの後には別々のGodotプロセスを起動し、`VideoStreamPlayer`への映像受信・動画終了・RPCの往復を検証します。結果は`target/video-processes`と`target/rpc-processes`へ保存します。

SDKブリッジ結合テストは静的リンクしたテスト用SDKを使用し、既存Kyberホストとの通信は検証しません。必要なファイルやFFmpegがない場合はスキップせず失敗します。

GUTは開発用です。プラグインを別のゲームへ導入するときは`addons/kyber`だけをコピーします。
## GitHub Actions

`.github/workflows/ci.yaml`は、mainへのpush、PR、手動実行で起動します。

| ジョブ | 実行内容 |
|---|---|
| Rust formatting | Ubuntuでルートとmediaのフォーマット確認、リリースパッケージのテストを実行します。 |
| Windows build and Godot tests | WindowsでClippy、Rustテスト、Media Foundationの実デコードテスト、リリース版DLLのGUTテストを実行します。音声・映像ループバックとエクスポートも検証し、成功後に配布用ZIPを作成します。 |
| Linux build and Godot tests | Clippy、RustとGStreamerのテスト、GUT、別プロセス間の映像・RPC、SDKの起動と終了、音声・映像ループバックを検証し、配布用アーカイブを作成します。 |
| macOS build and Godot tests | Apple Siliconランナーでメディア、GUT、映像、RPC、音声・映像ループバックを検証し、検証済みdylibを配布用にまとめます。 |
| Android arm64 cross-build | リリース版アドオンと実機用ネイティブテストをビルドします。実機では実行しません。 |
| iOS arm64 cross-build | リリース版フレームワークをビルドします。配布用署名や実機での実行は行いません。 |

Rustは1.96.0、Godotは4.7.2-stableに固定しています。Godotは公式の配布先から取得し、SHA-512を照合します。FFmpegはWindowsではChocolatey、Linuxではapt、macOSではHomebrewで導入します。KyProtoのループバック通信とSDKを含むネイティブビルドをCIで実行します。既存ホストへの接続は未検証です。

Actionsの実行結果から、次のartifactをダウンロードできます。保存期間は14日です。

- `godot-kyber-windows-x86_64`: SDKを組み込んだ単一の拡張DLLを含むアドオンの配布ZIPです。
- `windows-test-results`: GUTのJUnit XMLと実行ログです。テストが失敗した場合も、生成済みの結果を保存します。

- `godot-kyber-linux-x86_64`: 検証済みLinuxアドオンのアーカイブです。
- `linux-test-results`: Linuxのテスト結果とログです。
- `godot-kyber-macos-arm64`: 検証済みmacOSアドオンのアーカイブです。
- `macos-test-results`: macOSのテスト結果とログです。
- `godot-kyber-android-arm64-build`: 依存物のライセンスを含むAndroidアドオンです。
- `godot-kyber-ios-arm64-build`: 依存物のライセンスを含むiPhoneOSフレームワークのアドオンです。

配布ZIPは次のコマンドでも作成できます。

```powershell
pwsh -File scripts/test.ps1 -Release
pwsh -File scripts/package.ps1
```

出力先は`target/packages/godot-kyber-<version>-windows-x86_64.zip`です。CIワークフローはGitHub Releaseの公開やバージョンの書き換えを行いません。

### アドオンとデモのパッケージを公開する

専用の［Release］ワークフローはgodot-servoと同じパッケージ構成を採用しています。Windows/Linux x86_64とmacOS/Android/iOS arm64の最適化ビルドを作成します。5プラットフォームの成果物を集約して、次の2つのZIPを生成します。

- `godot-kyber-v<version>.zip`: ZIP直下が`addons/kyber/`です。既存プロジェクトへ展開できます。プロジェクトと依存ライブラリのライセンスも同梱します。
- `godot-kyber-demo-v<version>.zip`: `godot-kyber-demo-v<version>/`内に、同じアドオン、デモシーン、ドキュメントを含むプロジェクトを配置します。

リリースではアドオンのビルドとパッケージ生成を実行します。APK・IPA・デスクトップアプリのエクスポートや実機テストは実行しません。通常のCIでは既存のテストを継続します。バイナリの欠落、アーキテクチャの不一致、モック、チェックサムの不一致があればパッケージ生成を失敗させます。

リリース前にクレートのバージョンとロックファイルを更新します。GitHub Actionsの［Release］を、`v0.1.0`のような一致するバージョンで実行します。全ビルドの成功後に2つのZIPを公開し、ビルド元のコミットへタグを付けます。既存タグは拒否し、バージョン更新のコミットは自動作成しません。

ダウンロードしたビルド成果物をローカルで集約する場合は、Python 3.11以降で実行します。

```sh
python3 scripts/package_release.py --staged staged --version v0.1.0
```

`staged`内には`addon-windows`、`addon-linux`、`addon-macos`、`addon-android`、`addon-ios`を用意します。それぞれにビルド済みアドオンの内容を配置します。出力先は`target/packages/release`です。モバイル向けのエクスポートには各SDK、Godotのエクスポートテンプレート、署名設定が別途必要です。iOSではMobileレンダラーとarm64実機を選択します。シミュレーター用バイナリは含みません。

## SDKを含むビルド環境

Windows SDK付きのVisual Studio C++ビルドツール、Rust、Python、Git、CMake、LLVMを用意します。
`scripts/build.ps1 -Release`で、固定SDKの取得、パッチ適用、拡張のビルド、デモへの配置を実行します。
SDKとC/C++ランタイムは静的リンクします。Media FoundationはWindowsのシステムライブラリから読み込みます。
VLC、FFmpeg、MinGWランタイム、外部SDKのDLLは同梱しません。

新しいチェックアウトでCargoを直接使う場合は、先に`python scripts/prepare_sdk.py`を実行します。

SDKの準備処理は、固定コミットとファイルハッシュを検証して再生用の互換定義を抽出します。
そこへMITのCPU出力アダプターを追加します。生成したSDKコードは`.deps/`内に置き、元のライセンス表記を維持します。
生成物をGit管理下のソースへコピーしないでください。
すべてのプラットフォームのビルドスクリプトが、この準備処理を自動実行します。
抽出処理を変更した場合は、準備後に`python scripts/test_prepare_sdk.py`を実行します。
CIではRustの整形確認の前に、生成結果、ライセンス表記、想定外のソースの拒否を検証します。
公開SDKを組み込んだバイナリには、引き続きAGPLの条件が適用されます。

エクスポートテストは、初回インポート前にGDExtensionを登録し、インポート完了を待ちます。
フレーム数による終了は初回のファイル走査と競合する可能性があります。
配信テストでは、受信側の接続・切断とは別のスレッドでフレームを生成します。
同期的な切断処理の待機中もキャプチャーを継続するためです。

共通バックエンドは`media/`にあります。変更時は、このクレートにもfmtとClippyを実行します。

```powershell
cargo fmt --manifest-path media/Cargo.toml --check
cargo clippy --manifest-path media/Cargo.toml --all-targets -- -D warnings
```

## 配布用とテスト用の区別

GUTは`mock-sdk`で組み込んだテスト専用SDKを使用します。Media Foundationと動画ファイル通信は実装本体を検証します。
`scripts/test.ps1 -Release`の終了時には配布用SDKのDLLへ戻します。実SDKの接続失敗と終了処理も確認します。
FFmpegはテスト動画の生成専用です。アドオンの実行には使用しません。

`-MockOnly`は実SDKの検証を省く開発用です。配布には使用できません。
`-PrepareOnly`はGUT用ビルドをデモに残します。通常版へ戻すには`scripts/build.ps1`を実行します。

CIはWindowsとLinuxでSDKとメディアアダプターをビルドします。配布スクリプトは検証したバイナリーのハッシュを照合します。
ZIPには単一の拡張DLLとライセンス文書を含めます。`addons/kyber`全体をコピーします。

同じGodotバージョンのWindows用エクスポートテンプレートを導入すると、`scripts/test_export.ps1`で書き出しを検証できます。
実行ファイルの起動、SDK初期化、動画ファイルの読み込み、コーデックDLLを同梱していないことを確認します。

既存コードは`connect_to_host()`の先頭の`library`引数と、`KyberVideoStream.sdk_library`への代入を削除します。
`KyberFileVideo.start_sender()`の`ffmpeg`引数も削除します。ホストのOpus音声も受信する場合は`audio`に`true`、映像だけの場合は`false`を指定します。
自動再接続は引き続き公式SDKが担当します。

`VideoStreamPlayer`は音声なしの場合も音声スレッドから再生状態を参照します。このためgodot-rustの`experimental-threads`を有効にしています。
映像・シーン操作はメインスレッドで実行します。Godot 4.7.2の初回ヘッドレスインポートでは、エディターフレームを進めてから終了します。

## ビルドと互換性を保守する

履歴をまとめ直す前のコミットメッセージに記載されていた、開発上の注意点を残します。

### CargoのメタデータをUTF-8で読み込む

`scripts/install_addon.py`で`cargo metadata`を実行するときは、`encoding="utf-8"`を維持します。過去に、PythonがWindowsの既定文字コードを使い、依存クレートのメタデータを読み込めない問題がありました。コンパイルではなく、WindowsとAndroidの配布物生成が停止していました。

CargoのJSON出力を読むツールを追加するときも、UTF-8を明示します。実行環境のロケールに依存させたり、復号できない文字を破棄したりしないでください。インストーラーは、このメタデータを使って配布用の依存ライセンスを収集します。

### SDKの内部IFを変更したら両側を再ビルドする

コールバックのシグネチャやSDKの内部IFを変更するときは、`sdk/`と`src/backend.rs`を同時に更新します。テストやパッケージ作成の前に、アドオン全体を再ビルドして配置し直します。

以前の自動再接続のコミットでは、ブリッジABI 2に合わせた別体のSDK DLLの再ビルドが必要でした。これは動的ロード方式での要件です。現在はSDKを組み込んでおり、外部SDKのパスや公開ABI番号はありません。現在のIFは[SDK統合ガイド](sdk.md)を参照してください。

### 送信側と受信側のプロトコル互換性を確認する

ファイル配信への音声追加時に、アプリケーションプロトコルをリビジョン2へ変更しました。その送信側を使うには、受信側も更新する必要があります。現在の受信側は、リビジョン1の映像専用送信にも対応します。ローカルで再ビルドしても、接続先にインストール済みの受信側は更新されません。

配布後の接続失敗を調べるときは、両端のバージョンを確認します。プロトコルのリビジョンとコーデックへの対応は別の条件です。リビジョン2への対応だけでは、H.264への対応を保証しません。通信形式の変更やコーデックの選択前に、[映像配信](video_ja.md)と[H.264の互換性](h264_ja.md)を確認してください。

## Androidでビルドして実機テストする

対象はarm64、API 24以降です。デスクトップエディターには、そのOS用のアドオンも必要です。デモにはGodot 4.7.2を使い、Androidのネイティブファイル選択にはAndroid 10以降が必要です。

WindowsにAndroid SDK、NDK 27.2.12479018、JDK 17、CMake、Ninja、Godotと同じバージョンのAndroidエクスポートテンプレートを導入します。Godotのエディター設定でAndroid SDKとJava SDKのパスを設定して実行します。

```powershell
rustup target add aarch64-linux-android
./scripts/build_android.ps1 -BuildTests
```

NDKの場所は`-Ndk <directory>`、最適化ビルドは`-Release`で指定します。SDKとC++ランタイムは静的に組み込み、メディアライブラリはAndroidのものを使います。ELFは16 KBページ向けに整列しますが、16 KB端末での動作は未検証です。

アドオン、依存関係のライセンス通知、ビルド情報を`target/android-addon/addons/kyber`へ配置します。デモにもAndroid用ライブラリを追加し、Windows用DLLは保持します。デモの**Android**プリセットでInternet権限を有効にしてエクスポートします。他のプロジェクトではエクスポートの包含フィルターに`addons/kyber/licenses/*`を追加します。AndroidのGDExtension依存関係には共有ライブラリを指定し、ライセンスのフォルダーは指定しません。デバッグ用キーストアは開発専用です。

USB接続した端末とWindowsを同じLANに接続して実行します。

```powershell
python scripts/test_android.py --adb <adb.exe> --serial <USB-serial> --godot <Godot.exe> --windows-ip <WindowsのLAN-IP> --device-ip <AndroidのLAN-IP>
```

専用の`org.godotengine.kybertest` APKをインストールします。デモ内にネイティブWindowsビルドと、デスクトップのテスト手順で生成したMP4が必要です。SDKの起動・接続失敗・終了、ループバック映像、LANの双方向映像、Androidがサーバー／クライアントとなるRPCを確認します。結果は`target/android-test`へ保存し、終了時にテストアプリを停止します。UDPの14434番と14435番へ通信できる必要があります。

クロスビルドした`native_media-*`テスト実行ファイルは`/data/local/tmp`へ転送します。`KYBER_MEDIA_FIXTURES`に転送先の`tests/media`を指定して実行すると、色、向き、時刻、全6フレーム、リセット、解像度変更、設定パケットの再利用を検証します。APKのテスト完了も終了コードとは別に確認します。

2026年11月22日の初期映像検証では、Android 14のarm64端末でネイティブメディア5件とGodotの映像・RPCテストが成功しました。実際の外部Kyberホスト、Android 14以外、16 KB端末は未検証です。バックグラウンド動作などの制限は[機能と制限](overview_ja.md)を参照します。

AndroidのCIジョブではリリース用アドオンと実機用テストをクロスビルドします。CI上では実機テストを実行せず、前述のADB手順で確認します。

目視確認では実機テストのコマンドに`--manual windows-to-android --video <video.mp4>`を追加します。両アプリを起動したままにし、WindowsのプロセスIDを`target/manual-windows-to-android`へ保存します。そのWindowsプロセスを停止してから、`--manual android-to-windows --skip-export`で逆方向へ切り替えます。終了時はWindows側のプロセスを停止し、Android側は`adb -s <serial> shell am force-stop org.godotengine.kybertest`で停止します。動画は停止するまでループ再生します。

2026年11月22日にWindows ↔ Androidのループ映像を両方向で目視確認しました。未最適化のAndroid送信は低速で、Windows受信側にフレーム欠落が発生しました。リリースビルドの速度と継続動作の性能は未検証です。

## iPhone向けにビルドしてテストする

Apple SiliconのMacにXcode、Rust、Python 3.11以降、CMake、Godot 4.7.2を用意します。iPhoneをXcodeとペアリングし、デベロッパーモードを有効にします。署名にはAppleの開発チームと有効な開発用証明書が必要です。

```sh
python3 scripts/install_ios_templates.py
bash scripts/build_ios.sh
python3 scripts/export_ios.py --godot /path/to/Godot --team YOURTEAMID
```

SDKは`GodotKyber.framework`に組み込みます。配布用アドオンは`target/ios-addon/addons/kyber`へ生成します。最適化する場合はビルドに`--release`を指定します。生成物はiPhone実機用のarm64のみで、シミュレーターには対応しません。

Xcodeで`target/ios/kyberdemo/export/kyberdemo.xcodeproj`を開きます。実行先に接続中のiPhone実機を選び、［Run］を実行します。キーチェーンから署名鍵へのアクセスを求められたら許可します。生成するプロジェクトの対象はiPhoneOSに限定しています。再エクスポートする前に、このプロジェクトを閉じます。

デモはMetalを使い、ローカルネットワークの利用目的を設定しています。iPhoneでローカルネットワークへのアクセスを許可します。ネイティブのファイル選択画面で選んだ動画はキャッシュへコピーするため、動画1本分の空き容量が必要です。ファイル音声に対応します。バックグラウンド通信には対応しません。

専用テストアプリは`--test`を付けてエクスポートします。`target/ios/kybertest/export/kybertest.xcodeproj`を開き、iPhone実機で実行します。その後、次のコマンドで検証します。

```sh
python3 scripts/test_ios.py --device <iPhone-UDID> --godot /path/to/Godot --mac-ip <Mac-LAN-IP> --device-ip <iPhone-LAN-IP>
```

デモ内にネイティブmacOSアドオンと、デスクトップのテスト手順で生成したMP4が必要です。iPhoneが送信側／サーバー、受信側／クライアントとなる映像・RPCを検証します。UDPの14500～14503番へ通信できる必要があります。結果は`target/ios-tests`以下へ保存し、終了時にテストアプリを停止します。

2026年11月22日にiOS 27.2のiPhone 15 Proで、SDKとループバックの実機テストが成功しました。Mac↔iPhoneの映像・RPCも両方向で成功しました。映像は各方向で640×360を36フレーム受信し、ドロップは0でした。他のiOSバージョン、iPad、App Store配布、継続動作の性能は未検証です。CIではリリース用フレームワークをビルドし、実機テストは実行しません。

同日にWindows↔iPhoneのループ映像も両方向で目視確認しました。Windows受信側の最終ログでは308フレームを受信し、ドロップは0でした。未最適化ビルドでの確認で、継続的なフレームレートは未検証です。UIKitのファイル選択機能は実装済みですが、iPhone実機での外部ファイル選択は未検証です。

その後、Windows→iPhoneでドロップ0でも周期的に停止する現象を再現しました。JPEGの圧縮・復号をQUICの通信スレッドから分離し、目視でも停止の解消を確認しました。計測した表示間隔の最大値は1,300 msから187 ms、95パーセンタイルは267 msから50 msへ改善しました。修正後の489件の表示観測では、200 msを超える間隔はありませんでした。短時間の実機比較であり、遅延の保証ではありません。修正は全ネイティブプラットフォームに適用されますが、この計測比較はWindows→iPhoneのみです。


## 拡張機能の検証

2026年11月22日にWindowsからMacとSteam Deckへ、描画したViewportを同時配信しました。両端末で映像を表示しました。Deckのプロセスを一時停止しても、Macの受信は継続しました。Deckの受信アプリを再起動すると配信中の映像へ再参加できました。

初回はDeckのJPEG復号が約55msかかり、送信間隔の約50msを超えていました。そのため、通信バッファに古い映像が溜まっていました。受信処理を復号から分離し、圧縮済みの最新1枚を残す方式へ修正しました。公式KyProtoのクロック同期で送信側の時刻基準を取得し、経過時間の計測と古いフレームの除外にも使用します。修正後、再起動後の大きな遅れが解消したことをユーザーが目視確認しました。

最終のデバッグビルドでは、Macで2,000フレーム以上、再起動後のDeckで980フレーム以上を表示しました。最後の120サンプルの送信元から復号完了までの平均推定時間は、Macが約71ms、Deckが約130msでした。物理画面の表示遅延ではありません。端末の処理速度による差は残ります。フレーム単位の同時表示は未検証で、保証していません。

再実行には`demo/tests/process/multi_video.gd`を使います。引数は役割、アドレス、ポート、作成済みの結果ディレクトリ、実行秒数、任意の動画パスです。`sender`で動画を省略すると、動くViewportを取得します。受信側には送信側の結果ディレクトリから`code.txt`をコピーします。各プロセスは計測JSONと描画PNGを保存します。結果ディレクトリに`stop`ファイルを作成すると終了します。

Rustテストでは途中参加、切断時の分離、EOF、遅い復号、SDKクリップボードパケットの往復、ゲームパッド変換を確認します。GUTでは拡張入力API、クリップボード通知、UTF-8とJSONのサイズ制限、計測項目の取得可否を確認します。実Kyberホストでの入力・クリップボード交換、モバイルOSの許可画面、新機能のモバイル実機動作は未検証です。

音声追加前の拡張機能検証では、Windowsでfmt、Clippy、Rust 16件、GUT 47件、別プロセスの映像・RPC、ネイティブランタイムのスモークテストが成功しました。LinuxとmacOSではfmt、Clippy、Rustテストが成功しました。Android arm64とiOS arm64もビルドできました。リリースパッケージの検証は5件成功しました。既存CIは追加したRust・GUTテストを自動検出します。この検証時点ではGitHub Actionsは実行していません。

### ファイル音声の検証

Windows・macOS・Linux・Android・iOSでは、ファイル音声を48 kHz・ステレオのPCMへ変換します。ネイティブテストには44.1 kHz・モノラルのWAVを使います。
Windows・Linux・macOSの標準テストスクリプトは、ネイティブ版の配置後に`test_audio_processes.py`を実行します。Godot音声バスへの出力、ループ、一時停止・再開を検証します。
LANテストでは、`tests/process/audio_video.gd`のポート引数の後に接続先アドレスと秒単位の実行時間を指定できます。送信側の`code.txt`を受信側の結果ディレクトリへコピーします。
結果には音声のサンプル数とエネルギー、映像フレーム数、推定した映像・音声の時刻差を記録します。実際のスピーカーと画面のずれを測定する値ではありません。

### モバイルの音声テスト

Androidのソースビルドでは、Opusの静的リンクにCMakeとNinjaが必要です。ビルドスクリプトでNDKのABIをarm64-v8a、APIを24に指定します。
ビルド後、前述の`test_android.py`に`--audio`を追加すると、音声テストアプリのエクスポート・インストールとWindows ↔ Androidの双方向テストを実行します。UDPの14600～14601番ポートを使用します。
iOSでは再ビルドし、`--test`でエクスポートしたアプリをインストールします。前述の`test_ios.py`に`--audio`を追加すると、Mac ↔ iPhoneをUDPの14610～14611番ポートで検証します。
どちらもFFmpegで44.1 kHzのAAC音声付き動画を作成し、ループ、一時停止・再開、Godot音声バスのサンプル、推定した映像・音声の時刻差を確認します。FFmpegはテスト時のみ必要です。

Opusをソースからビルドするため、全プラットフォームでCMakeが必要です。macOSでは`brew install cmake`で導入できます。

### プラットフォーム間の音声検証

2026年11月22日にWindows ↔ macOS、Windows ↔ Steam Deck、Windows ↔ Android、macOS ↔ iPhoneでファイル音声の双方向LANテストが成功しました。各方向で20秒間に83万以上のPCMフレームを受信側のGodot音声バスで取得し、ループと一時停止・再開を検証しました。観測した映像・音声の時刻差のp95最大値は、組み合わせ順に76.4 ms、25.8 ms、16.1 ms、8.2 msでした。これは推定再生時刻の計測であり、実際のスピーカーと画面の測定や聴感確認ではありません。Androidでは44.1 kHz・モノラルから48 kHz・ステレオへの変換を含む、ネイティブメディア6件も成功しました。

最終のLinux標準テストでは、フォーマット、Clippy、Rust 22件、GUT 47件、別プロセスの映像・RPC、映像音声ループバック、SDKの起動確認が成功しました。音声ループバックは528,384サンプルを取得し、時刻差のp95最大値は6.512 msでした。Windowsはフォーマット、Clippy、Rust 21件、macOSはGUT 47件と音声ループバックを含む標準テストが成功しました。リリースパッケージのテスト5件も成功しました。GitHub Actions自体は実行していません。

### ライブ音声と転送方式のテスト

デスクトップの標準テストでは`scripts/test_live_audio.py`と、3方式の映像・音声プロセステストを実行します。Godot音声バスに44.1 kHzのテスト音を生成し、受信側の音声を取得します。共有・独立AudioStream、停止直後の再生、二重消費の拒否、映像の無通信タイムアウトより長い音声専用接続を確認します。実マイクの録音は行いません。

`python scripts/test_audio_processes.py --godot <実行ファイル> --transport 2 --drop-every 11`は、開始から2秒後に下りUDPの11パケットごとに1つを落とします。映像・音声出力、ループ、一時停止・再開、推定同期差を確認します。この欠落条件での再生継続の検証であり、全欠落の復元や実際のスピーカーと画面の同期を保証するテストではありません。

GUTではマイクの入力設定、バスの並べ替え・後片付け、接続情報の非保存、不正接続のエラーも検証します。CIはWindows・Linux・macOSの標準スクリプトから実行します。モバイルでも共通実装をコンパイルしますが、Android・iOSの実マイクと権限は別途実機検証が必要です。

2026年11月22日、この実装はWindowsの標準テスト（整形、Clippy、Rust・ネイティブ22件、GUT 52件、映像・RPCの別プロセス通信、SDK起動・終了）を通過しました。Linuxも標準テスト（Rust・ネイティブ23件、GUT 51件）を通過し、後から追加した不正接続のケースを含む音声GUT全5件も確認しました。両環境で3方式の映像・音声通信、パケット欠落時の再生、音声バスの取得、独立・共有の音声再生を確認しました。音声と映像を別リソースで共有し、下りUDPを11パケットごとに1つ落とす試験の推定A/V同期差p95の最大値は、Windowsで101.668 ms、Linuxで40.720 msでした。Android arm64のリリースビルドも成功しました。実マイクの録音と、今回の新経路のmacOS・iOS実機テストは未実施です。GitHub Actions自体は実行していません。
