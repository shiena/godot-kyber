# SPDX-FileCopyrightText: 2026 KOGA Mitsuhiro
# SPDX-License-Identifier: MIT
"""Build the standalone Pages site using documentation from the addon branch."""
import argparse
import html
import hashlib
import json
import pathlib
import posixpath
import re
import shutil
import struct
import subprocess
from urllib.parse import quote, unquote, urlsplit

import markdown
from markdown.extensions.toc import slugify_unicode

ROOT = pathlib.Path(__file__).resolve().parents[1]
REPO = 'https://github.com/shiena/godot-kyber'
GUIDES = [
    ('installation', 'Install the addon', 'インストール', None),
    ('getting-started', 'Get started', 'はじめての送受信', None),
    ('overview', 'Features & platforms', '対応機能・プラットフォーム', 'overview'),
    ('video', 'Video & shared sessions', '映像・セッション管理', 'video'),
    ('audio', 'Audio & microphones', '音声・マイク', 'audio'),
    ('extensions', 'Capture, input & clipboard', 'キャプチャー・入力・クリップボード', 'extensions'),
    ('multiplayer', 'Multiplayer RPC', 'マルチプレイヤーRPC', 'multiplayer'),
    ('connections', 'Connections & sessions', '接続とセッション管理', None),
    ('host', 'Existing Kyber hosts', '既存Kyberホストへの接続', 'host'),
    ('h264', 'H.264 & GPU acceleration', 'H.264・GPU処理', 'h264'),
    ('troubleshooting', 'Troubleshooting', 'トラブルシューティング', None),
    ('development', 'Build, test & distribute', 'ビルド・テスト・配布', 'development'),
    ('sdk', 'SDK internals', 'SDK統合仕様', 'sdk'),
    ('license', 'Licensing', 'ライセンス', None),
]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--source-ref', default='main')
    parser.add_argument('--base-path', default='/godot-kyber/')
    parser.add_argument('--output', type=pathlib.Path, default=ROOT / 'dist')
    args = parser.parse_args()
    guide_info = json.loads((ROOT / 'content/guides.json').read_text(encoding='utf-8'))
    ref = subprocess.check_output(['git', 'rev-parse', '--verify', args.source_ref + '^{commit}'], cwd=ROOT, text=True).strip()
    base = '/' + args.base_path.strip('/') + '/' if args.base_path.strip('/') else '/'
    out = args.output.resolve()
    if out == ROOT or ROOT not in out.parents:
        raise ValueError('Output must be a subdirectory of the site worktree')
    out.mkdir(parents=True, exist_ok=True)

    def source(path):
        return subprocess.check_output(['git', 'show', f'{ref}:{path}'], cwd=ROOT)

    def url(path='', lang='en'):
        return base + ('ja/' if lang == 'ja' else '') + path

    def guide(slug, lang):
        return url('guide/' + slug + '/', lang)

    def asset(path):
        version = ''
        if path in ('site.css', 'site.js', 'demo.webm'):
            digest = hashlib.sha256((ROOT / 'assets' / path).read_bytes()).hexdigest()[:12]
            version = '?v=' + digest
        return base + 'assets/' + path + version

    def save(path, text):
        dest = out / path
        dest.parent.mkdir(parents=True, exist_ok=True)
        dest.write_text(text, encoding='utf-8', newline='\n')

    assets = out / 'assets'
    assets.mkdir(exist_ok=True)
    for name in ('site.css', 'site.js'):
        shutil.copy2(ROOT / 'assets' / name, assets / name)
    demo_video = ROOT / 'assets/demo.webm'
    if demo_video.is_file():
        shutil.copy2(demo_video, assets / 'demo.webm')
    (assets / 'icon.svg').write_bytes(source('demo/addons/kyber/icon.svg'))
    if (ROOT / 'assets/screenshots').is_dir():
        shutil.copytree(ROOT / 'assets/screenshots', assets / 'screenshots', dirs_exist_ok=True)
    for name in ('CC-BY-4.0.txt', 'LicenseRef-Kyber-Logo.txt', 'MIT.txt', 'AGPL-3.0-or-later.txt'):
        dest = out / 'LICENSES' / name
        dest.parent.mkdir(exist_ok=True)
        dest.write_bytes(source('LICENSES/' + name))

    labels = {slug: (en, ja) for slug, en, ja, _ in GUIDES}
    def label(slug, lang):
        return labels[slug][lang == 'ja']

    def navigation(lang, current):
        groups = [('Start here', 'はじめに', ['installation', 'getting-started', 'overview']),
                  ('Guides', '使い方', ['video', 'audio', 'extensions', 'multiplayer']),
                  ('Connections & tuning', '接続・設定', ['connections', 'host', 'h264', 'troubleshooting']),
                  ('Development', '開発・配布', ['development', 'sdk', 'license'])]
        result = f'<a class="guide-index-link" href="{url("guide/", lang)}">{"ユーザーガイド" if lang == "ja" else "User guide"}</a>'
        for en, ja, slugs in groups:
            result += f'<section><h2>{ja if lang == "ja" else en}</h2>'
            for slug in slugs:
                active = ' aria-current="page"' if slug == current else ''
                result += f'<a href="{guide(slug, lang)}"{active}>{label(slug, lang)}</a>'
            result += '</section>'
        return result

    def shell(lang, title, body, slug='', toc=''):
        ja = lang == 'ja'
        other = 'en' if ja else 'ja'
        switch = guide(slug, other) if slug else url(lang=other)
        home = not slug
        nav = f'<aside class="sidebar" id="sidebar" aria-label="Documentation">{navigation(lang, slug)}</aside>' if slug else ''
        right = f'<aside class="on-page"><p>{"このページの内容" if ja else "On this page"}</p>{toc}</aside>' if toc else ''
        search = 'ドキュメントを検索' if ja else 'Search documentation'
        description = 'GodotとKyberで映像・音声・入力をつなぐGDExtension。' if ja else 'Video, audio, and remote input through familiar Godot APIs. A Kyber GDExtension for five native platforms.'
        return f'''<!doctype html>
<html lang="{lang}" data-base="{base}" data-language="{lang}">
<head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1">
<title>{html.escape(title)} · godot-kyber</title><meta name="description" content="{description}">
<meta name="color-scheme" content="dark light"><link rel="icon" href="{asset('icon.svg')}" type="image/svg+xml">
<link rel="stylesheet" href="{asset('site.css')}"><script src="{asset('site.js')}" defer></script></head>
<body class="{'home' if home else 'documentation'}">
<a class="skip-link" href="#main">{'本文へ移動' if ja else 'Skip to content'}</a>
<header class="header"><div class="header-inner">
<a class="brand" href="{url(lang=lang)}"><img src="{asset('icon.svg')}" width="34" height="34" alt=""><span>godot<span class="brand-separator">-</span>kyber</span></a>
<span class="header-divider"></span><a class="docs-link" href="{url('guide/', lang)}">{'ドキュメント' if ja else 'Documentation'}</a>
<div class="header-actions"><button class="search-open" aria-label="{search}"><span>⌕</span><span class="search-label">{'検索' if ja else 'Search docs'}</span><kbd>/</kbd></button>
<a class="language" href="{switch}" lang="{other}" aria-label="{'Switch to English' if ja else '日本語に切り替える'}">{'EN' if ja else '日本語'}</a>
<button class="theme-toggle" aria-label="{'配色を切り替える' if ja else 'Toggle color theme'}">◐</button>
<a class="github-link" href="{REPO}">GitHub <span aria-hidden="true">↗</span></a>
{'<button class="menu-toggle" aria-label="Documentation menu" aria-controls="sidebar" aria-expanded="false">☰</button>' if slug else ''}</div></div></header>
<div class="{'docs-layout' if slug else 'home-layout'}">{nav}<main id="main">{body}</main>{right}</div>
<footer class="footer"><a class="brand" href="{url(lang=lang)}">godot-kyber</a><span>{'Godotのための、Kyberストリーミング。' if ja else 'Kyber streaming, at home in Godot.'}</span><div><a href="{guide('license', lang)}">{'ライセンス' if ja else 'License'}</a><a href="{url('credits/', lang)}">{'クレジット' if ja else 'Credits'}</a><a href="{REPO}/releases">Releases ↗</a></div></footer>
<dialog id="search-dialog" aria-label="{search}"><form method="dialog"><button class="search-close" aria-label="Close">×</button></form><label for="search-input">{search}</label><input id="search-input" name="q" type="search" autocomplete="off" placeholder="{'映像、音声、H.264…' if ja else 'Video, audio, H.264…'}"><p class="search-status" aria-live="polite"></p><div id="search-results"></div></dialog>
</body></html>'''

    def home(lang):
        ja = lang == 'ja'
        t = lambda en, jp: jp if ja else en
        cards = [
            ('01', 'video', 'Video streaming', 'Godotのプレイヤーで映像を再生', 'Receive video with VideoStreamPlayer. Share connection state and input through KyberSession.', 'VideoStreamPlayerで映像を受信。KyberSessionで接続状態と入力を共有します。'),
            ('02', 'audio', 'Audio streaming', '音声の送受信', 'Stream file audio, a Godot audio bus, or a microphone. Play through AudioStreamPlayer.', '動画の音声、Godotの音声バス、マイクを送信。AudioStreamPlayerで再生します。'),
            ('03', 'extensions', 'Capture and remote input', 'キャプチャーとリモート入力', 'Capture a Viewport, forward input, and exchange clipboard text. Inspect session statistics.', 'Viewportのキャプチャー、入力の転送、テキストのクリップボード交換、統計情報の確認。'),
        ]
        features = ''.join(f'<a class="feature-card" href="{guide(slug, lang)}"><h3>{t(en, jp)}</h3><p>{t(desc, jdesc)}</p></a>' for n, slug, en, jp, desc, jdesc in cards)
        platforms = [('Windows', 'x86_64', 'Media Foundation'), ('macOS', 'arm64', 'AVFoundation · VideoToolbox'), ('Linux', 'x86_64', 'GStreamer'), ('Android', 'arm64', 'MediaCodec'), ('iOS', 'arm64', 'AVFoundation · VideoToolbox')]
        rows = ''.join(f'<tr><th scope="row"><span class="platform-dot"></span>{name}</th><td>{arch}</td><td>{backend}</td></tr>' for name, arch, backend in platforms)
        return f'''<section class="hero"><div class="hero-copy"><p class="product-kind">Godot 4 GDExtension</p>
<h1>{t('Video, audio and input<br>for Godot.', 'Godotで映像・音声・<br>入力を送受信。')}</h1>
<p class="hero-description">{t('Bring video, audio, and remote input into your Godot project—with the nodes and resources you already know.', '映像・音声・リモート入力を、使い慣れたGodotのノードとリソースで。Kyberの通信機能をプロジェクトに組み込みます。')}</p>
<div class="hero-buttons"><a class="button primary" href="{guide('getting-started', lang)}">{t('Read the quick start', '使い始める')}</a><a class="button secondary" href="{REPO}/releases/latest">{t('Download addon', 'アドオンをダウンロード')}</a></div>
</div>
<div class="hero-visual" aria-label="Godot and Kyber integration diagram">
<div class="identity"><img src="{asset('icon.svg')}" width="108" height="108" alt="godot-kyber"><div><strong>godot-kyber</strong><span>KyberSession</span></div></div>
<div class="flow"><div><span class="flow-icon">▧</span><strong>Viewport</strong><small>{t('Capture', 'キャプチャー')}</small></div><span class="flow-line">⟶</span><div><span class="flow-icon blue">⇄</span><strong>Kyber</strong><small>KyProto / QUIC</small></div><span class="flow-line">⟶</span><div><span class="flow-icon">▷</span><strong>VideoStream</strong><small>{t('Play', '再生')}</small></div></div>
<div class="code-card"><div class="code-title"><span>receiver.gd</span><span class="code-tag">GDScript</span></div><pre><code translate="no"><span class="code-purple">var</span> stream := <span class="code-blue">KyberVideoStream</span>.new()
stream.set_session(session)
<span class="code-green">$VideoStreamPlayer</span>.stream = stream
<span class="code-green">$VideoStreamPlayer</span>.play()</code></pre><p class="code-context">{t("This example uses an already-connected KyberSession.", "この例では、接続済みのKyberSessionを使用します。")}</p></div></div></section>
<figure class="demo-video" aria-labelledby="demo-video-caption">
<div class="demo-video-placeholder" role="img" aria-label="{t('Video placeholder: Windows and macOS app recordings joined along a diagonal edge into one window.', '動画用プレースホルダー。WindowsとmacOSのアプリ録画を斜めの境界で分割し、1つのウインドウに合成します。')}">
<div class="demo-window-title" aria-hidden="true"><span>godot-kyber</span><span>{t('Video streaming demo', '映像送受信デモ')}</span></div>
<div class="demo-split" aria-hidden="true">
<div class="demo-half demo-windows"><span class="demo-platform">Windows</span><span class="demo-role">{t('Sender', '送信')}</span></div>
<div class="demo-half demo-macos"><span class="demo-platform">macOS</span><span class="demo-role">{t('Receiver', '受信')}</span></div>
<svg class="demo-divider" viewBox="0 0 100 100" preserveAspectRatio="none"><path d="M 58 0 L 42 100" /></svg>
<div class="demo-placeholder-label">{t('Demo video placeholder', 'デモ動画のプレースホルダー')}</div>
</div></div>
<figcaption id="demo-video-caption"><strong>{t('Video and audio streaming demo', '映像・音声の送受信デモ')}</strong></figcaption>
</figure>
<section class="home-section"><div class="section-heading"><div><h2>{t('Integrate with Godot nodes', 'Godotの標準ノードと連携')}</h2></div><a class="text-link" href="{guide('overview', lang)}">{t('Explore the features', '対応機能を見る')}</a></div><div class="feature-grid">{features}</div></section>
<section class="home-section platform-section"><div><h2>{t('Supported platforms', '対応プラットフォーム')}</h2><p>{t('The SDK is embedded in the addon. No separate SDK install or library path is needed.', 'SDKはアドオンに組み込み済み。別途インストールしたり、ライブラリのパスを指定したりする必要はありません。')}</p><p class="muted">{t('Linux needs GStreamer and codec plugins. Mobile export needs platform tools and signing. Web is not supported.', 'LinuxにはGStreamerとコーデックプラグインが必要です。モバイルのエクスポートには専用ツールと署名設定が必要です。Webは対象外です。')}</p><a class="text-link" href="{guide('development', lang)}">{t('Platform setup guides', 'プラットフォーム別の設定')}</a></div><div class="platform-table"><table><thead><tr><th>{t('Platform', '環境')}</th><th>CPU</th><th>{t('Media backend', 'メディア機能')}</th></tr></thead><tbody>{rows}</tbody></table></div></section>
<section class="start-banner"><div><h2>{t('Try it with two app instances.', 'アプリを2つ起動して、試してみる。')}</h2><p>{t('The demo includes file streaming, Viewport capture, and multiplayer RPC.', '動画ファイルの送受信、Viewportキャプチャー、マルチプレイヤーRPCのデモを同梱しています。')}</p></div><a class="button primary" href="{guide('getting-started', lang)}">{t('Open the quick start', '導入ガイドへ')}</a></section>
<section class="license-note"><span>ⓘ</span><p>{t('The source code in this repository is MIT-licensed. Public binaries include the Kyber SDK and are distributed under AGPL-3.0-or-later. An applicable Kyber SAS commercial agreement may provide an alternative for covered Kyber components.', 'このリポジトリのソースコードはMITライセンスです。公開バイナリはKyber SDKを含み、AGPL-3.0-or-laterで配布します。対象のKyberコードには、条件を満たすKyber SASの商用契約を適用できる場合があります。')} <a href="{guide('license', lang)}">{t('Understand the licenses', 'ライセンスの詳細')}</a></p></section>'''

    paths = {}
    pages = []
    for lang in ('en', 'ja'):
        for slug, en, ja, stem in GUIDES:
            if stem is None:
                path = 'content/' + slug + ('_ja' if lang == 'ja' else '') + '.md'
                text = (ROOT / path).read_text(encoding='utf-8-sig').replace('KyberViewportSender', 'KyberFileVideo.start_viewport_sender()')
            else:
                path = 'BINARY_LICENSE.md' if stem == 'BINARY_LICENSE' else 'docs/' + stem + ('_ja' if lang == 'ja' else '') + '.md'
                override = ROOT / 'content' / pathlib.PurePosixPath(path).name
                text = override.read_text(encoding='utf-8-sig') if override.is_file() else source(path).decode('utf-8-sig')
            paths.setdefault(path, guide(slug, lang))
            pages.append((lang, slug, path, text))
        paths['docs/README' + ('_ja' if lang == 'ja' else '') + '.md'] = guide('getting-started', lang)
        paths['README' + ('_ja' if lang == 'ja' else '') + '.md'] = url(lang=lang)

    search_index = {'en': [], 'ja': []}
    for lang, slug, path, text in pages:
        # Hide repository-level language/index links; the site provides its own.
        text = re.sub(r'^\[(Documentation|ドキュメント|Project overview|プロジェクト概要|ドキュメント一覧).*$', '', text, flags=re.M)
        md = markdown.Markdown(extensions=['extra', 'toc', 'sane_lists'], extension_configs={'toc': {'slugify': slugify_unicode, 'toc_depth': '2-3'}})
        rendered = md.convert(text)
        def screenshot(match):
            caption = html.escape(match[1])
            pending = 'スクリーンショット掲載予定' if lang == 'ja' else 'Screenshot placeholder'
            return f'<figure class="screenshot-placeholder"><div>{pending}</div><figcaption>{caption}</figcaption></figure>'
        rendered = re.sub(r'<!-- screenshot: (.*?) -->', screenshot, rendered)
        def capture(match):
            name, caption = match[1], html.escape(match[2])
            image_url = asset('screenshots/' + name + '.png')
            image_path = ROOT / 'assets/screenshots' / (name + '.png')
            with image_path.open('rb') as image_file:
                header = image_file.read(24)
            if header[:8] != b'\x89PNG\r\n\x1a\n' or header[12:16] != b'IHDR':
                raise ValueError(f'Invalid PNG header: {image_path}')
            width, height = struct.unpack('>II', header[16:24])
            return f'<figure class="doc-screenshot"><a href="{image_url}"><img src="{image_url}" width="{width}" height="{height}" style="display:block;width:100%;height:auto;aspect-ratio:{width}/{height};object-fit:contain" loading="lazy" alt="{caption}"></a><figcaption>{caption}</figcaption></figure>'
        rendered = re.sub(r'<!-- capture: ([a-z0-9-]+) \| (.*?) -->', capture, rendered)
        def rewrite(match):
            target = html.unescape(match[1])
            parsed = urlsplit(target)
            if parsed.scheme or target.startswith(('#', '/')):
                return match[0]
            resolved = posixpath.normpath(posixpath.join(posixpath.dirname(path), unquote(parsed.path)))
            if path.startswith('content/') and resolved.startswith('../'):
                resolved = resolved[3:]
            destination = paths.get(resolved)
            if resolved == 'BINARY_LICENSE.md':
                destination = guide('license', lang)
            elif resolved == 'docs/sdk.md':
                destination = guide('sdk', lang)
            if destination is None:
                destination = REPO + '/blob/' + ref + '/' + quote(resolved)
            return 'href="' + html.escape(destination + ('#' + parsed.fragment if parsed.fragment else '')) + '"'
        rendered = re.sub(r'href="([^"]*)"', rewrite, rendered)
        intro = html.escape(guide_info[slug][lang])
        rendered = re.sub(r'(</h1>)', lambda m: m[1] + f'<p class="guide-summary">{intro}</p>', rendered, count=1)
        related = ''.join(f'<a href="{guide(item, lang)}">{label(item, lang)}</a>' for item in guide_info[slug]['related'])
        rendered += f'<nav class="related-guides" aria-label="{"関連ガイド" if lang == "ja" else "Related guides"}"><h2>{"関連ガイド" if lang == "ja" else "Related guides"}</h2>{related}</nav>'
        title = label(slug, lang)
        banner = ''
        source_url = REPO + '/blob/' + ref + '/' + path if not path.startswith('content/') else REPO + '/blob/gh-pages/' + path
        override = ROOT / 'content' / pathlib.PurePosixPath(path).name
        if override.is_file():
            source_url = REPO + '/blob/gh-pages/content/' + override.name
        body = f'<div class="doc-breadcrumb">DOCS <span>/</span> {title}</div><article class="prose">{banner}{rendered}</article><div class="doc-bottom"><a href="{source_url}">{"ソースを見る" if lang == "ja" else "View page source"} ↗</a><a href="{REPO}/issues">{"問題を報告" if lang == "ja" else "Report an issue"} ↗</a></div>'
        relative = ('ja/' if lang == 'ja' else '') + 'guide/' + slug + '/index.html'
        save(relative, shell(lang, title, body, slug, md.toc))
        plain = html.unescape(re.sub('<[^>]+>', ' ', rendered))
        search_index[lang].append({'title': title, 'url': guide(slug, lang), 'text': re.sub(r'\s+', ' ', plain)})

    for lang in ('en', 'ja'):
        title = 'ユーザーガイド' if lang == 'ja' else 'User guide'
        description = '導入から機能別の使い方、ビルドと配布まで。目的に合うガイドを選んでください。' if lang == 'ja' else 'Find a guide for installation, feature integration, or building and distributing the addon.'
        entries = ''.join(f'<li><a href="{guide(slug, lang)}">{label(slug, lang)}</a><p>{html.escape(guide_info[slug][lang])}</p></li>' for slug, *_ in GUIDES)
        body = f'<article class="prose"><h1>{title}</h1><p>{description}</p><ul class="guide-directory">{entries}</ul></article>'
        directory = shell(lang, title, body, 'index')
        directory = directory.replace(guide('index', 'ja' if lang == 'en' else 'en'), url('guide/', 'ja' if lang == 'en' else 'en'))
        save(('ja/' if lang == 'ja' else '') + 'guide/index.html', directory)
        landing = home(lang)
        if demo_video.is_file():
            video = f'<video class="demo-video-player" controls loop playsinline preload="metadata" aria-label="{"映像・音声の送受信デモ" if lang == "ja" else "Video and audio streaming demo"}"><source src="{asset("demo.webm")}" type="video/webm"><a href="{asset("demo.webm")}">{"動画をダウンロード" if lang == "ja" else "Download the video"}</a></video>'
            landing = re.sub(r'<div class="demo-video-placeholder".*?</div></div>\s*(?=<figcaption)', video, landing, flags=re.S)
        save(('ja/' if lang == 'ja' else '') + 'index.html', shell(lang, 'Godot × Kyber', landing))
        credits = f'''<article class="prose credits"><h1>{'クレジット' if lang == 'ja' else 'Credits'}</h1><p>Original site design and code: KOGA Mitsuhiro, MIT.</p><p>Godot Engine icon: © 2017 Andrea Calabró, <a href="{base}LICENSES/CC-BY-4.0.txt">CC BY 4.0</a>. The icon is scaled and overlaid on the Kyber icon. Kyber mark: Kyber SAS, <a href="{base}LICENSES/LicenseRef-Kyber-Logo.txt">rights notice</a>. This is an unofficial integration, not an endorsement by either project.</p><p>Built with Python-Markdown (BSD). Documentation and addon artwork are taken from <a href="{REPO}/tree/{ref}">addon revision {ref[:7]}</a>.</p><a href="{url(lang=lang)}">← {'ホームへ' if lang == 'ja' else 'Back to home'}</a></article>'''
        save(('ja/' if lang == 'ja' else '') + 'credits/index.html', shell(lang, 'Credits', credits))
    save('assets/search.json', json.dumps(search_index, ensure_ascii=False))
    save('.nojekyll', '')
    save('404.html', shell('en', 'Page not found', f'<article class="prose credits"><h1>Page not found</h1><p>This page may have moved.</p><a class="button primary" href="{url()}">Back to home</a></article>'))
    save('build-info.json', json.dumps({'source_commit': ref, 'base_path': base, 'pages': len(pages)}, indent=2))
    print(f'Built {len(pages)} guide pages plus landing pages in {out} from {ref[:7]}')


if __name__ == '__main__':
    main()
