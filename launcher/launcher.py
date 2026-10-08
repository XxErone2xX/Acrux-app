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

LAUNCHER_VERSION = 9            # 실행기 코드를 바꿨을 때만 +1
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
iVBORw0KGgoAAAANSUhEUgAAAbgAAACWCAMAAACbxW1zAAAB4FBMVEWDhZl4fKRzdZ5taqZnaoZpan5iZHtnYJZZXX5dXnNYWW5T
VnVPUXBMTmhJTG5HSWZDRmdBQ2A9QF47PFg2N1I0Nj4zNE0xMUoxMEcvLkYuLkYuLUUuLUQtLUQtLUMtLEMtLEIsLEMsLEEsK0Es
K0ArK0ErKz4rKkArKj8qKj4qKT8qKT4pKT8qKj0qKT0pKT0qKjopKTspKTgoKTYpKD4pKD0oKD0pKDwoKDwoJzwoKDsoJzsnJzso
KDooKDgoJzknJzonJzknJzcnJjonJjkmJjkkJj4mJjgmJTglJTgmJjclJTcoKDYnJzYmJzUnJzMmJzImJjYmJjQlJjQmJjMlJjAl
Ji0mJTYlJTYlJTQlJTEkJTElJS8kJS4kJS0kJSwkJSslJDclJDYlJDUkJDUlJDQkJDMkIzMjIzMkJDEjJDEkIzIjIzEjIzAiIjAk
JC4jJC4jIy4iIy4iIi4hIi0hIS0jJCwjJCsjIywiIywiIysiIishIishISsgISsjJCoiIyoiIykiIiohIiohIikhIighIichISog
ISkgISggIScgISYgICofICgfICcfICYfICUfHygeHyYeHyUeHiYdHiUeHyQeHyMdHiQdHiMdHiIcHSMcHSIcHSEbHCEbHCA7HT2T
AAAiMElEQVR42u2di58b1XXHZe1odt4PzWRmpNVjtdKyDy92oSFQILFxiU0cHEJiyiOvliRQJwaH8CgphJCQBNJCWmhIk6Zp/tWe
c+69o5nRSBpJI61k52fw4vXu5vPRN79zzj3n3FHlM3/VRqryGbNSqXhBGKG8atUJwjAIAh/l+XXP8+qu61i2YRi6rlQq1e3qmYqk
SJKscOmGIcPPkLa3lW2pKukufpNX9+hH+PCzQviJIf5IW66BND+MGmPUZNohtYQ6Q+2C9pIaoO5K6G7U36R0b6y/JT0u9ERaXx3q
qVH9Q56+ndR3VicAB695peaHQK7RCI1KRa/7gpwXY7OAmq4oEoLbrlaIHEgmfLpeZeCkbUmqAjiGjpGj/x+E9LvvmQiu5gZhaeAG
WXB331HgKh64AtCFPqJR6n5AfvPqgA3BATcAJAO4MwjuTKXCwBE8xZDwR1QJXLWqO/At9bogF5C45zwVwcljLddMgmuVBe7eceCe
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
gg==
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
