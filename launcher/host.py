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

# ---- PyInstaller 가 런타임에 같이 넣도록 앱이 쓰는 모듈을 전부 불러둠 (앱 코드는 따로라 분석이 안 됨)
import asyncio, base64, collections, ctypes, ctypes.wintypes, datetime, difflib, hashlib, html, http.client  # noqa
import http.server, json, pathlib, queue, random, re, secrets, shlex, shutil, socket, ssl, string  # noqa
import struct, subprocess, tempfile, threading, time, traceback, unicodedata, urllib.parse, urllib.request  # noqa
import uuid, webbrowser, zipfile, logging, math, itertools, functools, io, glob, copy, csv, platform  # noqa
import tkinter, tkinter.filedialog, tkinter.messagebox, tkinter.ttk  # noqa
if os.name == "nt":
    import winreg, winsound  # noqa
import websocket  # noqa
import mss, mss.tools  # noqa
import numpy  # noqa
import cv2  # noqa
import rapidocr_onnxruntime  # noqa
import zstandard  # noqa  (디스코드 통신 압축 풀기)
try:
    import winrt.windows.foundation, winrt.windows.foundation.collections, winrt.windows.globalization  # noqa
    import winrt.windows.graphics.imaging, winrt.windows.media.ocr, winrt.windows.storage.streams  # noqa
except Exception:
    pass


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
