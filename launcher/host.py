# -*- coding: utf-8 -*-
"""
AcruxHost — 런타임 (파이썬 + 라이브러리) 실행 파일
런처가 받아둔 앱 코드(%LOCALAPPDATA%\\AcruxMacro\\app)를 이 런타임으로 실행함.
앱 코드는 업데이트로 자주 바뀌고, 이 런타임은 라이브러리가 바뀔 때만 다시 받음 (version.py 의 RUNTIME).

  AcruxHost.exe                 → app.py 실행
  AcruxHost.exe 파일.py 인자…   → 그 파일 실행 (앱이 위치 지정 창 등을 따로 띄울 때)
"""
import os
import runpy
import sys

# ---- PyInstaller 가 런타임에 같이 넣도록 앱이 쓰는 모듈을 전부 적어둠 (앱 코드는 따로라 분석이 안 됨)
# 실제로 불러오지는 않음 — 예전엔 여기서 전부 불러와서 앱 · 위치 지정 창 · OCR 등 모든 프로세스가
# 쓰지도 않는 OCR · 이미지 라이브러리를 메모리에 올렸음 (앱이 필요할 때 직접 불러옴)
# (조건은 실행 중에만 알 수 있는 값이어야 함 — `if False:` 는 파이썬이 지워 버려서 PyInstaller 가 못 봄)
if os.environ.get("ACRUX_HOST_ANALYZE") == "1":
    import asyncio, base64, collections, ctypes, ctypes.wintypes, datetime, difflib, hashlib, html, http.client  # noqa
    import http.server, json, pathlib, queue, random, re, secrets, shlex, shutil, socket, ssl, string  # noqa
    import struct, subprocess, tempfile, threading, time, traceback, unicodedata, urllib.parse, urllib.request  # noqa
    import uuid, webbrowser, zipfile, logging, math, itertools, functools, io, glob, copy, csv, platform  # noqa
    import gc, zlib, importlib, importlib.util  # noqa
    import tkinter, tkinter.filedialog, tkinter.messagebox, tkinter.ttk  # noqa
    import winreg, winsound  # noqa
    import websocket  # noqa
    import mss, mss.tools  # noqa
    import numpy  # noqa
    import cv2  # noqa
    import rapidocr_onnxruntime  # noqa
    import zstandard  # noqa  (디스코드 통신 압축 풀기)
    import winrt.windows.foundation, winrt.windows.foundation.collections, winrt.windows.globalization  # noqa
    import winrt.windows.graphics.imaging, winrt.windows.media.ocr, winrt.windows.storage.streams  # noqa


def main():
    root = os.environ.get("ACRUX_ROOT") or os.path.join(os.environ.get("LOCALAPPDATA", "."), "AcruxMacro")
    app_dir = os.environ.get("ACRUX_APP") or os.path.join(root, "app")
    os.environ.setdefault("ACRUX_APP", app_dir)
    os.environ.setdefault("ACRUX_DATA", os.path.join(root, "data"))
    # 창 없는 exe 라 print 할 곳이 없음
    if sys.stdout is None:
        sys.stdout = open(os.devnull, "w", encoding="utf-8")
    if sys.stderr is None:
        sys.stderr = open(os.devnull, "w", encoding="utf-8")
    # 앱 코드는 일반 .py 로 실행되는 것처럼 (경로를 앱 폴더 기준으로 잡도록)
    if hasattr(sys, "frozen"):
        del sys.frozen
    if len(sys.argv) > 1 and sys.argv[1].lower().endswith(".py"):
        script = os.path.abspath(sys.argv[1])
        sys.argv = sys.argv[1:]
    else:
        script = os.path.join(app_dir, "app.py")
        sys.argv = [script] + sys.argv[1:]
    sys.path.insert(0, os.path.dirname(script))
    os.chdir(os.path.dirname(script))
    runpy.run_path(script, run_name="__main__")


if __name__ == "__main__":
    main()
