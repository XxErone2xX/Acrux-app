# -*- coding: utf-8 -*-
"""PC 버전 번역 사전(src/web/i18n.js 의 E 목록) → 모바일 사전(android/app/src/main/assets/i18n/pc.tsv)
빌드할 때마다 (GitHub Actions) 다시 뽑아서, PC 에 번역을 추가하면 모바일에도 그대로 들어감
한 줄 = 한국어 \\t English \\t 日本語 (줄바꿈 · 탭은 \\n · \\t)"""
import os, re, sys

ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
SRC = os.path.join(ROOT, 'src', 'web', 'i18n.js')
OUT = os.path.join(ROOT, 'android', 'app', 'src', 'main', 'assets', 'i18n', 'pc.tsv')
LIT = r"""('(?:[^'\\]|\\.)*'|"(?:[^"\\]|\\.)*"|`(?:[^`\\$]|\\.)*`)"""
ROW = re.compile(r'^\s*\[\s*' + LIT + r'\s*,\s*' + LIT + r'\s*,\s*' + LIT + r'\s*\]\s*,?\s*(//.*)?$')


def unjs(s):
    s = s[1:-1]
    out, i = [], 0
    while i < len(s):
        c = s[i]
        if c == '\\' and i + 1 < len(s):
            n = s[i + 1]
            if n == 'n': out.append('\n'); i += 2; continue
            if n == 't': out.append('\t'); i += 2; continue
            if n == 'u' and re.match(r'[0-9a-fA-F]{4}', s[i + 2:i + 6]):
                out.append(chr(int(s[i + 2:i + 6], 16))); i += 6; continue
            out.append(n); i += 2; continue
        out.append(c); i += 1
    return ''.join(out)


def esc(s):
    return s.replace('\t', '\\t').replace('\n', '\\n')


def main():
    text = open(SRC, encoding='utf-8').read().split('\n')
    start = next(i for i, l in enumerate(text) if re.match(r'\s*const E = \[', l))
    end = next(i for i in range(start + 1, len(text)) if re.match(r'\s*\];', text[i]))
    rows = []
    for l in text[start + 1:end]:
        m = ROW.match(l)
        if not m:
            continue
        ko, en, ja = (unjs(g) for g in m.groups()[:3])
        if '<' in ko and '>' in ko:          # HTML 이 섞인 문장 (모바일엔 없음)
            continue
        rows.append('\t'.join(esc(x) for x in (ko, en, ja)))
    os.makedirs(os.path.dirname(OUT), exist_ok=True)
    with open(OUT, 'w', encoding='utf-8', newline='\n') as f:
        f.write('# PC 버전 src/web/i18n.js 에서 자동으로 뽑음 — 고치지 말고 PC 사전을 고칠 것 (android/tools/export_pc_i18n.py)\n')
        f.write('\n'.join(rows) + '\n')
    print(f'{len(rows)} entries → {os.path.relpath(OUT, ROOT)}')


if __name__ == '__main__':
    sys.exit(main())
