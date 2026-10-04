# -*- coding: utf-8 -*-
"""
복귀 후 동작 — 플레이어가 직접 만드는 매크로 (순서대로 실행)
동작 종류: 키 입력 / 키 조합(Ctrl+L 등) / 마우스 클릭 / 대기 / 글자 입력 / 스크롤 / 프로그램 실행 / 로블록스 앞으로
- 키·글자·스크롤 입력은 컴퓨터에 그대로 보냄 (지금 맨 앞에 있는 창으로 들어감 · 필요하면 '창 맨 앞으로' 동작을 먼저)
- 마우스 클릭 위치만 로블록스 창 기준 (위치를 로블록스 화면에서 지정하므로)
- 시간 값은 화면에선 초, 저장은 ms
- F7 또는 [정지]로 언제든 멈춤
"""
import os
import shlex
import subprocess
import threading
import time
from pathlib import Path

import macro

STEP_TYPES = {
    "key": "키 입력", "combo": "키 조합", "click": "마우스 클릭", "wait": "대기",
    "text": "글자 입력", "scroll": "스크롤", "run": "프로그램 실행", "focus": "창 맨 앞으로",
    "kill": "프로그램 강제 종료",
}
MODIFIERS = ("ctrl", "shift", "alt", "win")


def parse_combo(text):
    """'Ctrl + Shift + L' → ['ctrl', 'shift', 'l'] (모르는 키가 있으면 ValueError)"""
    keys = [k.strip().lower() for k in str(text or "").replace("-", "+").split("+") if k.strip()]
    alias = {"control": "ctrl", "windows": "win", "return": "enter", "escape": "esc", "spacebar": "space"}
    keys = [alias.get(k, k) for k in keys]
    bad = [k for k in keys if k not in macro.VK]
    if bad or not keys:
        raise ValueError(f"모르는 키: {', '.join(bad) or '(비어 있음)'}")
    return keys


class Stopped(Exception):
    pass


def _taskkill(names):
    """watcher_core.taskkill 과 같은 방식 (윈도우 API 로 직접 종료 · 로블록스면 '갑자기 꺼짐' 알림 안 띄움)"""
    import watcher_core
    return watcher_core.taskkill(names)


class StepRunner:
    def __init__(self, get_steps, log, stop_key="f7", label="복귀 후 동작"):
        self.get_steps = get_steps      # () -> 동작 목록
        self.label = label
        self.log = log
        self.stop_key = stop_key
        self.stop_ev = threading.Event()
        self.thread = None
        self.lock = threading.Lock()
        self.state = {"running": False, "step": -1, "msg": "대기"}

    def running(self):
        return bool(self.thread and self.thread.is_alive())

    def snapshot(self):
        with self.lock:
            return dict(self.state, running=self.running())

    def _set(self, **kw):
        with self.lock:
            self.state.update(kw)

    def stop(self):
        self.stop_ev.set()

    def start(self, reason="", delay=0.0):
        if self.running():
            self.stop()
            self.thread.join(3)
        steps = [s for s in (self.get_steps() or []) if s.get("enabled", True)]
        if not steps:
            return False
        self.stop_ev = threading.Event()
        self.thread = threading.Thread(target=self._run, args=(steps, reason, delay, self.stop_ev), daemon=True)
        self.thread.start()
        return True

    # ---- 도우미
    def _check(self, stop):
        if stop.is_set() or macro.key_down_now(self.stop_key):
            stop.set()
            raise Stopped()

    def _wait(self, sec, stop):
        end = time.time() + max(0.0, float(sec))
        while True:
            self._check(stop)
            left = end - time.time()
            if left <= 0:
                return
            time.sleep(min(0.05, left))

    @staticmethod
    def _roblox_rect():
        """클릭 위치 계산용 로블록스 창 영역 (창을 앞으로 가져오지는 않음)"""
        hwnd = macro.roblox_window_cached()
        if not hwnd:
            raise RuntimeError("로블록스 창을 찾을 수 없음")
        return macro.client_rect(hwnd)

    # ---- 실행
    def run_now(self, reason="", timeout=60.0):
        """바로 실행하고 끝날 때까지 기다림 (게임 접속 전 동작용)"""
        if not self.start(reason):
            return False
        self.thread.join(timeout)
        if self.thread.is_alive():
            self.stop()
            self.thread.join(2)
        return True

    def _run(self, steps, reason, delay, stop):
        self.log(f"{self.label} 시작{' (' + reason + ')' if reason else ''} — {len(steps)}개 · 정지: F7", "c")
        try:
            with macro.fast_timing():
                if delay:
                    self._set(msg=f"{delay:g}초 뒤 시작")
                    self._wait(delay, stop)
                for i, s in enumerate(steps):
                    self._check(stop)
                    self._set(step=i, msg=f"{i + 1}. {STEP_TYPES.get(s.get('type'), s.get('type'))}")
                    self._do(s, stop)
            self.log(f"{self.label} 완료", "g")
        except Stopped:
            self.log(f"{self.label} 정지", "d")
        except Exception as e:
            self.log(f"{self.label} 오류: {e}", "r")
        finally:
            self._set(step=-1, msg="대기")

    def _do(self, s, stop):
        t = s.get("type")
        gap = max(0, int(s.get("gap", 100))) / 1000
        if t == "wait":
            self._wait(max(0, int(s.get("ms", 1000))) / 1000, stop)
        elif t == "focus":
            # 맨 앞으로 가져올 창: 로블록스 / 다른 창 (제목에 들어간 글자 · 프로그램 이름)
            if s.get("target", "roblox") == "roblox":
                hwnd, name = macro.roblox_window_cached(), "로블록스"
            else:
                name = s.get("title") or s.get("exe") or "?"
                hwnd = None
                end = time.time() + max(0, int(s.get("wait_ms", 3000))) / 1000   # 창이 늦게 뜨면 잠깐 기다림
                while True:
                    hwnd = macro.find_window(s.get("title", ""), s.get("exe", ""))
                    if hwnd or time.time() >= end:
                        break
                    self._wait(0.2, stop)
            if not hwnd:
                raise RuntimeError(f"'{name}' 창을 찾을 수 없음")
            macro.focus(hwnd, wait=0.2)
        elif t == "key":
            for _ in range(max(1, int(s.get("count", 1)))):
                self._check(stop)
                macro.key_tap(s.get("key", "e"), int(s.get("hold", 40)))
                self._wait(gap, stop)
        elif t == "combo":
            keys = parse_combo(s.get("keys"))
            for _ in range(max(1, int(s.get("count", 1)))):
                self._check(stop)
                macro.key_combo(keys, int(s.get("hold", 40)))
                self._wait(gap, stop)
        elif t == "click":
            if s.get("x") is None:
                raise RuntimeError("클릭 위치가 지정되지 않음")
            rect = self._roblox_rect()
            x, y = macro.to_screen(s["x"], s["y"], rect)
            for _ in range(max(1, int(s.get("count", 1)))):
                self._check(stop)
                macro.move_to(x + 3, y + 3)
                time.sleep(0.03)
                macro.click(x, y, s.get("button", "left"))
                self._wait(gap, stop)
        elif t == "text":
            macro.paste_text(str(s.get("text", "")))
            if s.get("enter"):
                macro.key_tap("enter")
        elif t == "scroll":
            macro.scroll(int(s.get("amount", -3)))
            self._wait(gap, stop)
        elif t == "kill":
            names = [n.strip().strip('"') for n in str(s.get("exe") or "").replace(";", ",").split(",") if n.strip()]
            if not names:
                raise RuntimeError("종료할 프로그램 이름이 없음")
            names = [n if n.lower().endswith(".exe") else n + ".exe" for n in names]
            ok = _taskkill(names)
            self.log(f"프로그램 강제 종료: {', '.join(names)}" + ("" if ok else " (실행 중 아님)"), "d")
            self._wait(gap, stop)
        elif t == "run":
            self._launch(s)
            self._wait(max(0, int(s.get("wait_ms", 1000))) / 1000, stop)
        else:
            raise RuntimeError(f"모르는 동작: {t}")

    def _launch(self, s):
        path = str(s.get("path") or "").strip().strip('"')
        if not path:
            raise RuntimeError("실행할 프로그램이 지정되지 않음")
        args = str(s.get("args") or "").strip()
        p = Path(path)
        cwd = str(p.parent) if p.parent.exists() else None
        if not args and hasattr(os, "startfile"):
            os.startfile(path)                      # .exe / .lnk / .bat / 문서 / URL 전부 가능
        else:
            subprocess.Popen([path] + shlex.split(args, posix=False), cwd=cwd,
                             creationflags=getattr(subprocess, "DETACHED_PROCESS", 0))
        self.log(f"프로그램 실행: {p.name}{' ' + args if args else ''}", "d")
