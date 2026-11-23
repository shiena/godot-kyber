// SPDX-FileCopyrightText: 2026 KOGA Mitsuhiro
// SPDX-License-Identifier: MIT
const root = document.documentElement;
const ja = root.dataset.language === 'ja';
try { const theme = localStorage.getItem('godot-kyber-theme'); if (theme) root.dataset.theme = theme; } catch {}
document.querySelector('.theme-toggle').addEventListener('click', () => {
  const theme = root.dataset.theme === 'dark' ? 'light' : 'dark';
  root.dataset.theme = theme;
  try { localStorage.setItem('godot-kyber-theme', theme); } catch {}
});
const menu = document.querySelector('.menu-toggle');
if (menu) menu.setAttribute('aria-label', ja ? 'ドキュメントメニュー' : 'Documentation menu');
menu?.addEventListener('click', () => {
  const open = document.querySelector('#sidebar').classList.toggle('open');
  menu.setAttribute('aria-expanded', String(open));
});
document.addEventListener('keydown', event => {
  if (event.key === 'Escape' && menu) {
    document.querySelector('#sidebar').classList.remove('open');
    menu.setAttribute('aria-expanded', 'false');
  }
});
for (const block of document.querySelectorAll('.prose pre')) {
  const code = block.querySelector('code');
  if (!code) continue;
  const button = document.createElement('button');
  button.className = 'copy-code';
  button.setAttribute('aria-live', 'polite');
  button.textContent = ja ? 'コピー' : 'Copy';
  button.addEventListener('click', async () => {
    try {
      await navigator.clipboard.writeText(code.textContent);
      button.textContent = ja ? 'コピーしました' : 'Copied';
    } catch { button.textContent = ja ? '選択してコピーしてください' : 'Select text to copy'; }
    setTimeout(() => { button.textContent = ja ? 'コピー' : 'Copy'; }, 1800);
  });
  block.append(button);
}
const dialog = document.querySelector('#search-dialog');
const input = document.querySelector('#search-input');
const results = document.querySelector('#search-results');
const status = document.querySelector('.search-status');
let entries;
let loading;
async function loadSearch() {
  if (entries) return;
  loading ??= fetch(root.dataset.base + 'assets/search.json').then(response => {
    if (!response.ok) throw Error('Search index unavailable');
    return response.json();
  }).then(data => { entries = data[root.dataset.language]; }).catch(error => { loading = undefined; throw error; });
  await loading;
}
async function renderResults() {
  const query = input.value.trim().toLocaleLowerCase();
  results.replaceChildren();
  if (!query) { status.textContent = ja ? 'キーワードを入力してください。' : 'Type a keyword to search the guides.'; return; }
  try { await loadSearch(); } catch {
    status.textContent = ja ? '検索データを読み込めません。もう一度お試しください。' : 'Could not load search. Please try again.'; return;
  }
  if (query !== input.value.trim().toLocaleLowerCase()) return;
  const terms = query.split(/\s+/);
  const matches = entries.filter(entry => terms.every(term => (entry.title + ' ' + entry.text).toLocaleLowerCase().includes(term))).slice(0, 8);
  status.textContent = matches.length ? (ja ? `${matches.length}件を表示` : `${matches.length} results`) : (ja ? '一致するページがありません。' : 'No matching pages.');
  for (const entry of matches) {
    const link = document.createElement('a'); link.href = entry.url;
    const title = document.createElement('strong'); title.textContent = entry.title;
    const snippet = document.createElement('small');
    const index = Math.max(0, entry.text.toLocaleLowerCase().indexOf(terms[0]) - 35);
    snippet.textContent = (index ? '…' : '') + entry.text.slice(index, index + 160) + '…';
    link.append(title, snippet); results.append(link);
  }
}
function openSearch() { if (!dialog.open) dialog.showModal(); input.focus({ preventScroll: true }); renderResults(); }
document.querySelector('.search-open').addEventListener('click', openSearch);
input.addEventListener('input', renderResults);
dialog.addEventListener('click', event => { if (event.target === dialog && (event.clientX < dialog.getBoundingClientRect().left || event.clientX > dialog.getBoundingClientRect().right || event.clientY < dialog.getBoundingClientRect().top || event.clientY > dialog.getBoundingClientRect().bottom)) dialog.close(); });
document.addEventListener('keydown', event => {
  if (event.key === '/' && !event.ctrlKey && !event.metaKey && !['INPUT', 'TEXTAREA'].includes(document.activeElement.tagName) && !document.activeElement.isContentEditable) { event.preventDefault(); openSearch(); }
});
