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

LAUNCHER_VERSION = 10           # 실행기 코드를 바꿨을 때만 +1
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
iVBORw0KGgoAAAANSUhEUgAAAbgAAACWCAMAAACbxW1zAAAB4FBMVEWEhpp7fZJ2eIxvc49rbYNmaodiZH9dYHtbXXRWWndSVXJQ
UWtMUHFKTGhGSWpERmRAQ2M+QFw7PVs4OVQ1NlE0NU0yMks0Nj4wMEkwMEYvLkYuLkYuLUUuLUQtLUQtLUMtLEMtLEIsLEMsLEEs
K0EsK0ArK0ErKz4rKkArKj8qKj4qKT8qKT4pKT8qKj0qKT0pKT0qKjopKTspKTgoKTYpKD4pKD0oKD0pKDwoKDwoJzwoKDsoJzsn
JzsoKDooKDgoJzknJzonJzknJzcnJjonJjkmJjkkJj4mJjgmJTglJTgmJjcmJTclJTcoKDYnJzYmJzUnJzMmJzImJjYmJjQlJjQm
JjMlJjAlJi0mJTYlJTYlJTQlJTEkJTElJS8kJS4kJS0kJSwkJSslJDclJDYlJDUkJDUkJDQkIzMjIzMkJDEjJDEkIzIjIzEjIzAi
IjAkJC4jJC4jIy4iIy4iIi4hIi0hIS0jJCwjJCsjIywiIywiIysiIishIishISsjJCoiIyoiIykiIiohIiohIikhIighIichISog
ISkgISggIScgISYgICofICgfICcfICYfICUfHygeHyYeHyUeHiYdHiUeHyQeHyMdHiQdHiMdHiIcHSMcHSIcHSEbHCEbHCBuc5HA
AAAjJ0lEQVR42u2di5sb5XXGpdFo7veLMpJWl9Vql12vjV1oCBRIbCixSQgOITHlkltLAnVicAiX0kJCQhJIC2mhkCZN0vyrPed8
30gzo5E0kma1ks0BdvHauzyPfrzvuXznG1U+91lsZVQ+Z1YqFb8RUXi1mhM2GmEYBhC+73sQruPYpmkYmqbJlUqtXoMPskQhy7Kq
qoYhVauVer0u1euCoDmu58E3+j7+jAB+VqMR4c8MfFsSIZSg0WxNiTaLHYpOHL1x7ELsJWMf445E3InxN6m4exR/S/F4HE+k4xvj
eGoy/iEvvpeM768vABxwqwgBkWtFRqWiugG8zMQtxmaZFmBTZVkicECuLtcxGDtNEyr4Gxg1QXVcROdxdPT/QQMDyOkITnQbUWng
9rPg7rytwFVjyfl1+IXsBiHKDcgBNgBngNwAkCTB79aAXLVSlerwmcGTDbHCwNUInAXf4roxOPhRYSNEzQE5V0FwchhFM7m109xW
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
Gf8PCa4THz4SiRoAAAAASUVORK5CYII=
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
