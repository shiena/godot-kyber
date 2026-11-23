# SPDX-FileCopyrightText: 2026 KOGA Mitsuhiro
# SPDX-License-Identifier: MIT
"""Check generated links, anchors, language routes, and search entries."""
import argparse
import json
import pathlib
from html.parser import HTMLParser
from urllib.parse import unquote, urlsplit

class Page(HTMLParser):
    def __init__(self, text):
        super().__init__()
        self.links = []
        self.ids = set()
        self.feed(text)

    def handle_starttag(self, tag, attrs):
        attrs = dict(attrs)
        if 'id' in attrs:
            self.ids.add(attrs['id'])
        if tag in ('a', 'link') and 'href' in attrs:
            self.links.append(attrs['href'])
        if tag in ('script', 'img') and 'src' in attrs:
            self.links.append(attrs['src'])


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output', type=pathlib.Path, default=pathlib.Path(__file__).resolve().parents[1] / 'dist')
    args = parser.parse_args()
    root = args.output.resolve()
    info = json.loads((root / 'build-info.json').read_text(encoding='utf-8'))
    base = info['base_path']
    pages = {p.resolve(): Page(p.read_text(encoding='utf-8')) for p in root.rglob('*.html')}
    errors = []
    for path, page in pages.items():
        for link in page.links:
            url = urlsplit(link)
            if url.scheme or url.netloc:
                continue
            if not url.path:
                destination = path
            elif url.path.startswith(base):
                destination = root / unquote(url.path[len(base):])
            else:
                errors.append(f'{path.relative_to(root)}: outside base path: {link}')
                continue
            if destination.is_dir():
                destination /= 'index.html'
            destination = destination.resolve()
            if not destination.is_file():
                errors.append(f'{path.relative_to(root)}: missing target: {link}')
            elif url.fragment and destination in pages and unquote(url.fragment) not in pages[destination].ids:
                errors.append(f'{path.relative_to(root)}: missing anchor: {link}')
    index = json.loads((root / 'assets/search.json').read_text(encoding='utf-8'))
    for language in ('en', 'ja'):
        assert len(index[language]) == 11
        for item in index[language]:
            assert item['title'] and item['text']
            assert (root / item['url'][len(base):] / 'index.html').is_file()
    if errors:
        raise SystemExit('\n'.join(errors))
    print(f'Checked {len(pages)} pages, all internal links and anchors, and 22 search entries ({base})')

if __name__ == '__main__':
    main()
