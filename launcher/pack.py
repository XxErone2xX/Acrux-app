# -*- coding: utf-8 -*-
"""릴리스 파일 만들기 (GitHub Actions 에서 실행): app.zip · runtime.zip · release.json"""
import hashlib
import json
import os
import sys
import zipfile

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
SRC = os.path.join(ROOT, "src")
OUT = os.path.join(ROOT, "out")
SKIP_DIRS = {"__pycache__"}
SKIP_FILES = {"config.json", "config_t.json", "app.log"}
SKIP_EXT = {".pyc", ".bat", ".log"}


def zip_dir(src, dest):
    with zipfile.ZipFile(dest, "w", zipfile.ZIP_DEFLATED, compresslevel=6) as z:
        for base, dirs, files in os.walk(src):
            dirs[:] = [d for d in dirs if d not in SKIP_DIRS]
            for f in files:
                if f in SKIP_FILES or os.path.splitext(f)[1].lower() in SKIP_EXT:
                    continue
                p = os.path.join(base, f)
                z.write(p, os.path.relpath(p, src))


def sha(p):
    h = hashlib.sha256()
    with open(p, "rb") as f:
        for b in iter(lambda: f.read(1 << 20), b""):
            h.update(b)
    return h.hexdigest()


def main():
    os.makedirs(OUT, exist_ok=True)
    sys.path.insert(0, SRC)
    sys.path.insert(0, os.path.join(ROOT, "launcher"))
    import version
    from launcher import LAUNCHER_VERSION
    zip_dir(SRC, os.path.join(OUT, "app.zip"))
    zip_dir(os.path.join(ROOT, "dist", "AcruxHost"), os.path.join(OUT, "runtime.zip"))
    exe = os.path.join(ROOT, "dist", "Acrux.exe")
    files = {n: os.path.join(OUT, n) for n in ("app.zip", "runtime.zip")}
    files["Acrux.exe"] = exe
    info = {"version": version.VERSION, "runtime": version.RUNTIME, "launcher": LAUNCHER_VERSION,
            "sha256": {n: sha(p) for n, p in files.items()}}
    with open(os.path.join(OUT, "release.json"), "w", encoding="utf-8") as f:
        json.dump(info, f, indent=1)
    print(json.dumps(info, indent=1))


if __name__ == "__main__":
    main()
