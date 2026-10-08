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
import os
import shutil
import subprocess
import sys
import tempfile
import threading
import time
import urllib.request
import zipfile

LAUNCHER_VERSION = 7            # 실행기 코드를 바꿨을 때만 +1
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


class UI:
    """작은 진행 창 (업데이트가 없으면 잠깐 떴다가 사라짐)"""
    def __init__(self):
        import tkinter as tk
        from tkinter import ttk
        self.tk = tk
        self.root = tk.Tk()
        self.root.title(T["title"])
        self.root.overrideredirect(True)
        self.root.configure(bg="#1e1f22")
        w, h = 380, 112
        x = (self.root.winfo_screenwidth() - w) // 2
        y = (self.root.winfo_screenheight() - h) // 2
        self.root.geometry(f"{w}x{h}+{x}+{y}")
        self.root.attributes("-topmost", True)
        font = ("Malgun Gothic", 10)
        tk.Label(self.root, text="Acrux macro", fg="#ffffff", bg="#1e1f22", font=("Segoe UI", 14, "bold")).pack(anchor="w", padx=20, pady=(16, 2))
        self.msg = tk.Label(self.root, text="", fg="#b5bac1", bg="#1e1f22", font=font)
        self.msg.pack(anchor="w", padx=20)
        style = ttk.Style(self.root)
        style.theme_use("default")
        style.configure("A.Horizontal.TProgressbar", troughcolor="#2b2d31", background="#5865f2", thickness=6, borderwidth=0)
        self.bar = ttk.Progressbar(self.root, style="A.Horizontal.TProgressbar", length=340, mode="indeterminate")
        self.bar.pack(padx=20, pady=(10, 0))
        self.bar.start(12)
        self._det = False
        self.state = (None, None)

    def set(self, text, frac):
        self.state = (text, frac)

    def pump(self):
        text, frac = self.state
        if text is not None and self.msg.cget("text") != text:
            self.msg.config(text=text)
        if frac is None and self._det:
            self.bar.config(mode="indeterminate"); self.bar.start(12); self._det = False
        elif frac is not None:
            if not self._det:
                self.bar.stop(); self.bar.config(mode="determinate", maximum=1000); self._det = True
            self.bar["value"] = int(frac * 1000)


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
            ui.root.after(50, tick)
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
