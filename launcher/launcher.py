# -*- coding: utf-8 -*-
"""
Acrux.exe — 실행기 (사용자에게 보이는 건 이 파일 하나)

켤 때마다:
  1. GitHub 최신 릴리스 확인 (rngenesis0-coder/Acrux-app)
  2. 실행기가 더 새 버전이면 실행기 자신을 교체하고 다시 실행
  3. 런타임(파이썬+라이브러리)이 바뀌었으면 runtime.zip 받기 (가끔)
  4. 앱 버전이 바뀌었으면 app.zip 받기 (몇 MB)
  5. %LOCALAPPDATA%\\AcruxMacro\\runtime\\AcruxHost.exe 로 앱 실행 후 실행기는 종료
인터넷이 안 되거나 GitHub 이 안 되면, 이미 설치된 버전으로 그냥 실행.

폴더 (%LOCALAPPDATA%\\AcruxMacro)
  app\\       앱 코드 (업데이트 때 통째로 교체)
  runtime\\   파이썬 + 라이브러리
  data\\      설정(config.json)·로그 — 업데이트해도 그대로
  ui\\        화면(Edge) 프로필
  installed.json  설치된 버전
"""
import hashlib
import json
import math
import os
import shutil
import subprocess
import sys
import tempfile
import threading
import time
import urllib.request
import zipfile

LAUNCHER_VERSION = 8            # 실행기 코드를 바꿨을 때만 +1
REPO = "rngenesis0-coder/Acrux-app"
API = f"https://api.github.com/repos/{REPO}/releases/latest"
UA = {"User-Agent": "AcruxLauncher", "Accept": "application/vnd.github+json"}

ROOT = os.path.join(os.environ.get("LOCALAPPDATA") or os.path.expanduser("~"), "AcruxMacro")
APP, RUNTIME, DATA = (os.path.join(ROOT, d) for d in ("app", "runtime", "data"))
STATE = os.path.join(ROOT, "installed.json")
HOST = os.path.join(RUNTIME, "AcruxHost.exe")
NO_WINDOW = getattr(subprocess, "CREATE_NO_WINDOW", 0)

TEXT = {
    "ko": {"check": "업데이트 확인 중…", "dl_app": "업데이트 받는 중 (V{v})", "dl_rt": "필요한 파일 받는 중 (잠시 기다려 주십시오...)",
           "dl_self": "실행기 업데이트 중", "start": "실행 중…", "offline": "업데이트 확인 실패 — 설치된 버전으로 실행",
           "fail": "처음 설치에 필요한 파일을 받지 못했습니다.\n인터넷 연결을 확인하고 다시 실행해주세요.\n\n{e}",
           "running": "Acrux macro 가 이미 실행 중입니다.", "title": "Acrux macro"},
    "en": {"check": "Checking for updates…", "dl_app": "Downloading update (V{v})", "dl_rt": "Downloading required files (Please wait...)",
           "dl_self": "Updating launcher", "start": "Starting…", "offline": "Update check failed — starting installed version",
           "fail": "Couldn't download the files needed for the first install.\nCheck your internet connection and try again.\n\n{e}",
           "running": "Acrux macro is already running.", "title": "Acrux macro"},
    "ja": {"check": "アップデートを確認中…", "dl_app": "アップデートをダウンロード中 (V{v})", "dl_rt": "必要なファイルをダウンロード中（しばらくお待ちください...）",
           "dl_self": "ランチャーを更新中", "start": "起動中…", "offline": "アップデート確認に失敗 — インストール済みのバージョンで起動",
           "fail": "初回インストールに必要なファイルをダウンロードできませんでした。\nインターネット接続を確認して再度起動してください。\n\n{e}",
           "running": "Acrux macro はすでに起動しています。", "title": "Acrux macro"},
}


def lang():
    try:
        with open(os.path.join(DATA, "config.json"), encoding="utf-8-sig") as f:
            l = json.load(f).get("lang") or ""
    except Exception:
        l = ""
    if l not in TEXT:
        import locale
        loc = (locale.getdefaultlocale()[0] or "").lower()
        l = "ko" if loc.startswith("ko") else "ja" if loc.startswith("ja") else "en"
    return l


T = TEXT[lang()]


def read_state():
    try:
        with open(STATE, encoding="utf-8") as f:
            return json.load(f)
    except Exception:
        return {}


def write_state(st):
    os.makedirs(ROOT, exist_ok=True)
    tmp = STATE + ".tmp"
    with open(tmp, "w", encoding="utf-8") as f:
        json.dump(st, f)
    os.replace(tmp, STATE)


def get_json(url):
    with urllib.request.urlopen(urllib.request.Request(url, headers=UA), timeout=10) as r:
        return json.loads(r.read().decode("utf-8"))


def download(url, dest, progress, sha256):
    """받은 파일은 release.json 의 sha256 과 반드시 같아야 함 (값이 없으면 받지 않음)"""
    if not sha256 or len(sha256) != 64:
        raise RuntimeError(f"checksum missing: {os.path.basename(dest)}")
    if not url.startswith("https://"):
        raise RuntimeError("insecure download url")
    req = urllib.request.Request(url, headers={"User-Agent": "AcruxLauncher"})
    h = hashlib.sha256()
    with urllib.request.urlopen(req, timeout=30) as r, open(dest, "wb") as f:
        total = int(r.headers.get("Content-Length") or 0)
        done = 0
        while True:
            chunk = r.read(256 * 1024)
            if not chunk:
                break
            f.write(chunk)
            h.update(chunk)
            done += len(chunk)
            progress(done / total if total else None)
    if h.hexdigest().lower() != sha256.lower():
        raise RuntimeError(f"checksum mismatch: {os.path.basename(dest)}")


def install_zip(url, target, progress, sha256):
    """zip 을 받아서 target 폴더를 통째로 교체 (받다가 실패하면 기존 폴더 그대로)"""
    os.makedirs(ROOT, exist_ok=True)
    fd, tmpzip = tempfile.mkstemp(suffix=".zip", dir=ROOT)
    os.close(fd)
    new, old = target + ".new", target + ".old"
    try:
        download(url, tmpzip, progress, sha256)
        shutil.rmtree(new, ignore_errors=True)
        with zipfile.ZipFile(tmpzip) as z:
            z.extractall(new)
        shutil.rmtree(old, ignore_errors=True)
        if os.path.exists(target):
            os.replace(target, old)
        os.replace(new, target)
        shutil.rmtree(old, ignore_errors=True)
    finally:
        try:
            os.remove(tmpzip)
        except OSError:
            pass
        shutil.rmtree(new, ignore_errors=True)


def host_running():
    try:
        out = subprocess.run(["tasklist", "/FI", "IMAGENAME eq AcruxHost.exe", "/NH"], capture_output=True,
                             text=True, errors="ignore", timeout=5, creationflags=NO_WINDOW).stdout
        return "AcruxHost.exe" in out
    except Exception:
        return False


def clean_env(**extra):
    """실행기(exe 하나짜리)가 자기 임시 폴더를 가리키는 환경변수를 남기는데, 그대로 물려주면
    런타임이 그 폴더(실행기가 꺼지면 지워짐)를 쓰려다 실패함 → 깨끗한 환경으로 실행"""
    env = {k: v for k, v in os.environ.items()
           if not k.upper().startswith(("_PYI", "_MEI")) and k.upper() not in ("TCL_LIBRARY", "TK_LIBRARY", "TCLLIBPATH")}
    env["PYINSTALLER_RESET_ENVIRONMENT"] = "1"
    env.update(extra)
    return env


def self_update(url, progress, sha256):
    """실행 중인 exe 는 덮어쓸 수 없어서: 이름을 .old 로 바꾸고 새 파일을 그 자리에 둠.
    다시 실행하지 않고 이번 실행은 그대로 이어감 (새 실행기는 다음에 켤 때부터) → 한 번만 실행하면 됨"""
    me = sys.executable
    tmp = me + ".new"
    download(url, tmp, progress, sha256)
    old = me + ".old"
    try:
        os.remove(old)
    except OSError:
        pass
    os.replace(me, old)
    os.replace(tmp, me)


def update(ui):
    st = read_state()
    installed = os.path.exists(HOST) and os.path.exists(os.path.join(APP, "app.py"))
    ui.set(T["check"], None)
    try:
        rel = get_json(API)
        assets = {a["name"]: a["browser_download_url"] for a in rel.get("assets", [])}
        info = get_json(assets["release.json"]) if "release.json" in assets else {}
        sums = info.get("sha256") or {}
        frozen = getattr(sys, "frozen", False)
        if frozen and int(info.get("launcher", 0)) > LAUNCHER_VERSION and "Acrux.exe" in assets:
            ui.set(T["dl_self"], 0)
            try:
                self_update(assets["Acrux.exe"], lambda p: ui.set(T["dl_self"], p), sums.get("Acrux.exe"))
            except Exception:
                pass                    # 실행기 교체가 안 돼도(권한 등) 앱 업데이트·실행은 계속
        if not os.path.exists(HOST) or st.get("runtime") != info.get("runtime"):
            ui.set(T["dl_rt"], 0)
            install_zip(assets["runtime.zip"], RUNTIME, lambda p: ui.set(T["dl_rt"], p), sums.get("runtime.zip"))
            st["runtime"] = info.get("runtime")
            write_state(st)
        version = info.get("version") or rel.get("tag_name", "").lstrip("v")
        if not os.path.exists(os.path.join(APP, "app.py")) or st.get("version") != version:
            msg = T["dl_app"].format(v=version)
            ui.set(msg, 0)
            install_zip(assets["app.zip"], APP, lambda p: ui.set(msg, p), sums.get("app.zip"))
            st["version"] = version
            write_state(st)
    except Exception as e:
        if not installed:
            raise
        ui.set(T["offline"], None)
        time.sleep(1.2)


def launch():
    os.makedirs(DATA, exist_ok=True)
    env = clean_env(ACRUX_ROOT=ROOT, ACRUX_APP=APP, ACRUX_DATA=DATA)
    subprocess.Popen([HOST], cwd=APP, env=env, close_fds=True,
                     creationflags=getattr(subprocess, "DETACHED_PROCESS", 0))


# 진행 창 배경 (엠블럼 포함, 440x150 PNG)
BG_PNG = """
iVBORw0KGgoAAAANSUhEUgAAAbgAAACWCAMAAACbxW1zAAAB4FBMVEWDhZl5fJhzdqV0a9Roa4lqa39jZXxqYaFaXn9cXXJUWHdS
VXJPUXNNTIlJTG5MTWJGSGdNRIFCRGA9QF45O1Y2N1I1NlA0Nj4zNE0xMUswMUgvLkYuLkYuLUUuLUQtLUQtLUMtLEMtLEIsLEMs
LEEsK0EsK0ArK0ErKz4rKkArKj8qKj4qKT8qKT4pKT8qKj0qKT0pKT0qKjspKTspKTgoKTYpKD4pKD0oKD0pKDwoKDwoJzwoKDso
JzsnJzsoKDooKDgoJzknJzonJzknJzcnJjonJjkmJjkkJj4mJjgmJTglJTgmJjcmJTclJTcoKDYnJzYmJzUnJzMmJzImJjYmJjQl
JjQmJjMlJjElJi4mJTYlJTYlJTQlJTEkJTElJS8kJS4kJS0kJSwkJSslJDclJDYlJDUkJDUkJDQkIzMjIzMkJDEkIzIjIzEjIzAi
IjAkJC4jJC4jIy4iIy4iIi4hIi0hIS0jJCwjJCsjIywiIywiIysiIishIishISsjJCoiIyoiIykiIiohIiohIikhIighIichISog
ISkgISggIScgISYgICofICgfICcfICYfICUfHygeHyYeHyUeHiYdHiUeHyQeHyMdHiQdHiMdHiIcHSMcHSIcHSEbHCEbHCBz3404
AAAh3ElEQVR42u2di5vb5JXGrayl6FZJj26VJY0v47HNTCZpUmgpLNAmsDShUFJKm5RbL7u0sGlKSrksXejS0ha6C92FhW67bbf/
6p5zvk+2JMuybGs89oQTmECSGZ5HP973O7dP0/i7T2Mro/GZT2Mro/HZBoTot8IwjKJQbzRUx28Fge9BuI7rOLZtG5aha5oqy3Kj
cer06VONRlNuYsiSBL+o6gJ8jSb7NUG1HQc/z3XhK/h+0IKv1qKPvmuKGHYA/6niiFnsULST6E5iF2IvFUOKOybxOYrPT+KudHyB
4vEknsjGNydxncd35sT30/GD9cVnGbiG20JyYctrNhqC7PgBYnMZNtsyDB2pyXITwTVPCfBnmklIst7ELyE0CZ2gWrZjIzcg5yM5
PwiQG5DzXAXBSX4YlnKLs9yODNwT9YL7wfrBmSgMjMAiBjqXmwvULJCbrgI4Cbg0BACXIydLjQw4w7KZ6AgdYgvoq8NPvi2R5Frh
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
"""


class UI:
    """작은 진행 창 (업데이트가 없으면 잠깐 떴다가 사라짐)"""
    W, H = 440, 150
    C1, C2 = (108, 123, 255), (139, 92, 246)     # 앱과 같은 강조색 그라데이션
    SEGS = 56

    def __init__(self):
        import tkinter as tk
        self.tk = tk
        self.root = tk.Tk()
        self.root.title(T["title"])
        self.root.overrideredirect(True)
        self.root.configure(bg="#1b1c20")
        W, H = self.W, self.H
        x = (self.root.winfo_screenwidth() - W) // 2
        y = (self.root.winfo_screenheight() - H) // 2
        self.root.geometry(f"{W}x{H}+{x}+{y}")
        self.root.attributes("-topmost", True)
        try:
            self.root.attributes("-alpha", 0.0)
        except tk.TclError:
            pass
        cv = self.cv = tk.Canvas(self.root, width=W, height=H, highlightthickness=0, bd=0, bg="#1b1c20")
        cv.pack()
        try:
            self.bg = tk.PhotoImage(data=BG_PNG)
            cv.create_image(0, 0, image=self.bg, anchor="nw")
        except tk.TclError:
            self.bg = None
        title = cv.create_text(138, 36, text="Acrux macro", anchor="w", fill="#ffffff", font=("Segoe UI Semibold", 16))
        ver = read_state().get("version")
        if ver:
            x1 = cv.bbox(title)[2]
            cv.create_text(x1 + 8, 39, text=f"v{ver}", anchor="w", fill="#8e92a6", font=("Segoe UI", 9))
        self.msg = cv.create_text(24, H - 58, text="", anchor="w", fill="#c9ccd6", font=("Malgun Gothic", 10))
        self.pct = cv.create_text(W - 24, H - 58, text="", anchor="e", fill="#9aa0ff", font=("Segoe UI Semibold", 9))
        # 진행 바: 둥근 트랙 위에 그라데이션 조각들
        self.bx0, self.bx1, self.by = 24, W - 24, H - 32
        cv.create_line(self.bx0, self.by, self.bx1, self.by, width=5, capstyle="round", fill="#2e3038")
        self.segs = []
        step = (self.bx1 - self.bx0) / self.SEGS
        for i in range(self.SEGS):
            t = i / (self.SEGS - 1)
            c = "#%02x%02x%02x" % tuple(round(self.C1[k] + (self.C2[k] - self.C1[k]) * t) for k in range(3))
            a = self.bx0 + i * step
            self.segs.append((a, a + step, cv.create_line(a, self.by, a + step, self.by, width=5, capstyle="round", fill=c, state="hidden")))
        # 창 끌어서 옮기기
        cv.bind("<ButtonPress-1>", lambda e: setattr(self, "_drag", (e.x, e.y)))
        cv.bind("<B1-Motion>", self._move)
        self._t0 = time.monotonic()
        self._shown = (None, None)
        self.state = (None, None)
        self._fade()

    def _move(self, e):
        dx, dy = self._drag
        self.root.geometry(f"+{self.root.winfo_x() + e.x - dx}+{self.root.winfo_y() + e.y - dy}")

    def _fade(self, a=0.0):
        a = min(1.0, a + 0.12)
        try:
            self.root.attributes("-alpha", a)
        except self.tk.TclError:
            return
        if a < 1.0:
            self.root.after(16, self._fade, a)

    def _span(self, lo, hi):
        """lo~hi (픽셀) 구간만 채워서 보이기"""
        for a, b, item in self.segs:
            s, e = max(a, lo), min(b, hi)
            if e - s >= 0.5:
                self.cv.coords(item, s, self.by, e, self.by)
                self.cv.itemconfigure(item, state="normal")
            else:
                self.cv.itemconfigure(item, state="hidden")

    def set(self, text, frac):
        self.state = (text, frac)

    def pump(self):
        text, frac = self.state
        if text is not None and self._shown[0] != text:
            self.cv.itemconfigure(self.msg, text=text)
        span = self.bx1 - self.bx0
        if frac is None:
            # 왔다 갔다 하는 빛줄기
            p = ((time.monotonic() - self._t0) / 1.4) % 1.0
            p = 0.5 - 0.5 * math.cos(p * 2 * math.pi)
            w = span * 0.28
            c = self.bx0 - w + (span + w) * p
            self._span(max(self.bx0, c), min(self.bx1, c + w))
            if self._shown[1] is not None:
                self.cv.itemconfigure(self.pct, text="")
        else:
            frac = max(0.0, min(1.0, frac))
            self._span(self.bx0, self.bx0 + span * frac)
            self.cv.itemconfigure(self.pct, text=f"{int(frac * 100)}%")
        self._shown = (text, frac)


def main():
    # 예전 실행기 파일 정리 (실행기 업데이트 후 남은 것)
    if getattr(sys, "frozen", False):
        try:
            os.remove(sys.executable + ".old")
        except OSError:
            pass
    ui = UI()
    if host_running():
        from tkinter import messagebox
        ui.root.withdraw()
        messagebox.showinfo(T["title"], T["running"])
        return
    result = {}

    def work():
        try:
            update(ui)
            ui.set(T["start"], None)
            launch()
            result["ok"] = True
        except Exception as e:
            result["err"] = e

    th = threading.Thread(target=work, daemon=True)
    th.start()

    def tick():
        ui.pump()
        if th.is_alive():
            ui.root.after(25, tick)
        else:
            if "err" in result:
                from tkinter import messagebox
                ui.root.withdraw()
                messagebox.showerror(T["title"], T["fail"].format(e=result["err"]))
            ui.root.after(300 if "ok" in result else 0, ui.root.destroy)
    tick()
    ui.root.mainloop()


if __name__ == "__main__":
    main()
