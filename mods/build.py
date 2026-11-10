# -*- coding: utf-8 -*-
"""모드 파일 만들기: mods/<id>/ → out/mods/Acrux-<이름>-V<버전>.acxm (zip · mod.json 이 맨 위)
  python mods/build.py            모든 모드
  python mods/build.py sniping    그 모드만"""
import json
import os
import sys
import zipfile

ROOT = os.path.dirname(os.path.abspath(__file__))
OUT = os.path.join(os.path.dirname(ROOT), "out", "mods")
SKIP = {"__pycache__"}


def build(mid):
    src = os.path.join(ROOT, mid)
    info = json.load(open(os.path.join(src, "mod.json"), encoding="utf-8"))
    os.makedirs(OUT, exist_ok=True)
    dest = os.path.join(OUT, f"Acrux-{mid}-V{info.get('version', '0')}.acxm")
    with zipfile.ZipFile(dest, "w", zipfile.ZIP_DEFLATED, compresslevel=9) as z:
        for base, dirs, files in os.walk(src):
            dirs[:] = sorted(d for d in dirs if d not in SKIP)
            for f in sorted(files):
                if f.endswith(".pyc"):
                    continue
                p = os.path.join(base, f)
                z.write(p, os.path.relpath(p, src))
    print(dest)
    return dest


if __name__ == "__main__":
    ids = sys.argv[1:] or sorted(d for d in os.listdir(ROOT) if os.path.isfile(os.path.join(ROOT, d, "mod.json")))
    for i in ids:
        build(i)
