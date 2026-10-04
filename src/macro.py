# -*- coding: utf-8 -*-
"""
로블록스 조작 도구 (키·마우스 입력, 창 찾기, 화면 캡처, OCR)
- 키 입력 / 마우스: 윈도우 SendInput (로블록스가 받도록 하드웨어 스캔코드 방식)
- 좌표: 로블록스 창 안쪽(클라이언트 영역) 기준 비율(0~1) → 창 크기·위치가 바뀌어도 그대로 동작
- 화면 캡처: mss
- OCR: 윈도우 내장 OCR (Windows.Media.Ocr, winrt 패키지)
윈도우 전용 기능은 다른 OS 에서 불러와도 import 는 되도록 감싸둠 (테스트용)
"""
import ctypes
import os
import sys
import threading
import time
from pathlib import Path

IS_WIN = sys.platform == "win32"

if IS_WIN:
    from ctypes import wintypes
    user32 = ctypes.WinDLL("user32", use_last_error=True)
    try:
        ctypes.windll.shcore.SetProcessDpiAwareness(2)   # 화면 배율과 상관없이 실제 픽셀 좌표 사용
    except Exception:
        try:
            user32.SetProcessDPIAware()
        except Exception:
            pass


def _log(msg, color=""):
    try:
        import watcher_core as core
        core.log(msg, color)
    except Exception:
        print(msg)


# ---------------------------------------------------------------- SendInput
if IS_WIN:
    ULONG_PTR = ctypes.c_size_t

    class MOUSEINPUT(ctypes.Structure):
        _fields_ = [("dx", wintypes.LONG), ("dy", wintypes.LONG), ("mouseData", wintypes.DWORD),
                    ("dwFlags", wintypes.DWORD), ("time", wintypes.DWORD), ("dwExtraInfo", ULONG_PTR)]

    class KEYBDINPUT(ctypes.Structure):
        _fields_ = [("wVk", wintypes.WORD), ("wScan", wintypes.WORD), ("dwFlags", wintypes.DWORD),
                    ("time", wintypes.DWORD), ("dwExtraInfo", ULONG_PTR)]

    class HARDWAREINPUT(ctypes.Structure):
        _fields_ = [("uMsg", wintypes.DWORD), ("wParamL", wintypes.WORD), ("wParamH", wintypes.WORD)]

    class _U(ctypes.Union):
        _fields_ = [("mi", MOUSEINPUT), ("ki", KEYBDINPUT), ("hi", HARDWAREINPUT)]

    class INPUT(ctypes.Structure):
        _fields_ = [("type", wintypes.DWORD), ("u", _U)]

    user32.SendInput.argtypes = (wintypes.UINT, ctypes.POINTER(INPUT), ctypes.c_int)

INPUT_MOUSE, INPUT_KEYBOARD = 0, 1
KEYEVENTF_EXTENDEDKEY, KEYEVENTF_KEYUP, KEYEVENTF_SCANCODE = 0x1, 0x2, 0x8
MOUSEEVENTF_MOVE, MOUSEEVENTF_ABSOLUTE, MOUSEEVENTF_VIRTUALDESK = 0x1, 0x8000, 0x4000
MOUSEEVENTF_LEFTDOWN, MOUSEEVENTF_LEFTUP = 0x2, 0x4
MOUSEEVENTF_RIGHTDOWN, MOUSEEVENTF_RIGHTUP = 0x8, 0x10
MOUSEEVENTF_WHEEL = 0x800

# 키 이름 → 가상 키 코드
VK = {"enter": 0x0D, "esc": 0x1B, "tab": 0x09, "space": 0x20, "backspace": 0x08,
      "shift": 0x10, "ctrl": 0x11, "alt": 0x12, "win": 0x5B, "capslock": 0x14,
      "left": 0x25, "up": 0x26, "right": 0x27, "down": 0x28,
      "home": 0x24, "end": 0x23, "pageup": 0x21, "pagedown": 0x22, "insert": 0x2D, "delete": 0x2E,
      "/": 0xBF, ".": 0xBE, ",": 0xBC, "-": 0xBD, "=": 0xBB, ";": 0xBA, "'": 0xDE,
      "[": 0xDB, "]": 0xDD, "`": 0xC0, "\\": 0xDC}
for _c in "abcdefghijklmnopqrstuvwxyz":
    VK[_c] = ord(_c.upper())
for _d in "0123456789":
    VK[_d] = ord(_d)
for _n in range(1, 13):
    VK[f"f{_n}"] = 0x6F + _n
EXTENDED = {"left", "up", "right", "down", "home", "end", "pageup", "pagedown", "insert", "delete", "win"}
KEY_NAMES = sorted(VK, key=lambda k: (len(k) > 1, k))


def _send(*inputs):
    if not IS_WIN:
        return
    arr = (INPUT * len(inputs))(*inputs)
    user32.SendInput(len(inputs), arr, ctypes.sizeof(INPUT))


def _key_input(name, up):
    name = name.lower()
    vk = VK[name]
    scan = user32.MapVirtualKeyW(vk, 0)
    flags = KEYEVENTF_SCANCODE | (KEYEVENTF_KEYUP if up else 0) | (KEYEVENTF_EXTENDEDKEY if name in EXTENDED else 0)
    return INPUT(type=INPUT_KEYBOARD, u=_U(ki=KEYBDINPUT(0, scan, flags, 0, 0)))


def key_tap(name, hold_ms=40):
    if not IS_WIN:
        return
    _send(_key_input(name, False))
    time.sleep(max(0, hold_ms) / 1000)
    _send(_key_input(name, True))


def key_combo(names, hold_ms=40):
    """여러 키 동시 입력 (Ctrl+L 등): 앞 키들을 차례로 누른 채 마지막 키 → 거꾸로 뗌"""
    if not IS_WIN:
        return
    names = [n.lower() for n in names]
    for n in names:
        _send(_key_input(n, False))
        time.sleep(0.015)
    time.sleep(max(0, hold_ms) / 1000)
    for n in reversed(names):
        _send(_key_input(n, True))
        time.sleep(0.015)


def _mouse(flags, dx=0, dy=0, data=0):
    return INPUT(type=INPUT_MOUSE, u=_U(mi=MOUSEINPUT(dx, dy, data, flags, 0, 0)))


def move_to(x, y):
    """화면 절대 좌표(픽셀)로 이동. 로블록스가 위치를 인식하도록 1픽셀 흔들어줌"""
    if not IS_WIN:
        return
    vx, vy = user32.GetSystemMetrics(76), user32.GetSystemMetrics(77)
    vw, vh = user32.GetSystemMetrics(78), user32.GetSystemMetrics(79)
    nx = int((x - vx) * 65535 / max(1, vw - 1))
    ny = int((y - vy) * 65535 / max(1, vh - 1))
    f = MOUSEEVENTF_MOVE | MOUSEEVENTF_ABSOLUTE | MOUSEEVENTF_VIRTUALDESK
    _send(_mouse(f, nx, ny))
    time.sleep(0.01)
    _send(_mouse(MOUSEEVENTF_MOVE, 1, 0))
    _send(_mouse(MOUSEEVENTF_MOVE, -1, 0))


# 사람처럼 클릭 (스나이핑 안정성 설정): on = 부드럽게 이동 + 누르는 시간 랜덤 / jitter = 클릭 위치 랜덤 범위(px)
HUMAN = {"on": False, "jitter": 0}


def _abs_move(x, y):
    vx, vy = user32.GetSystemMetrics(76), user32.GetSystemMetrics(77)
    vw, vh = user32.GetSystemMetrics(78), user32.GetSystemMetrics(79)
    f = MOUSEEVENTF_MOVE | MOUSEEVENTF_ABSOLUTE | MOUSEEVENTF_VIRTUALDESK
    _send(_mouse(f, int((x - vx) * 65535 / max(1, vw - 1)), int((y - vy) * 65535 / max(1, vh - 1))))


def human_move(x, y):
    """지금 위치에서 목표까지 살짝 휘어진 길로 0.04~0.14초 동안 부드럽게 이동 (사람 손처럼 시작·끝은 느리게)"""
    if not IS_WIN:
        return
    import math
    import random
    sx, sy = cursor_pos()
    dist = math.hypot(x - sx, y - sy)
    if dist >= 3:
        steps = int(min(14, max(4, dist / 60)))
        dur = min(0.14, 0.04 + dist / 6000)
        bend = random.uniform(-0.08, 0.08) * dist
        mx, my = (sx + x) / 2 - (y - sy) / max(dist, 1) * bend, (sy + y) / 2 + (x - sx) / max(dist, 1) * bend
        for i in range(1, steps):
            t = i / steps
            e = t * t * (3 - 2 * t)
            bx = (1 - e) ** 2 * sx + 2 * (1 - e) * e * mx + e * e * x
            by = (1 - e) ** 2 * sy + 2 * (1 - e) * e * my + e * e * y
            _abs_move(bx, by)
            time.sleep(dur / steps)
    move_to(x, y)


def click(x, y, button="left", hold_ms=30):
    j = int(HUMAN.get("jitter") or 0)
    if j > 0:
        import random
        x, y = x + random.randint(-j, j), y + random.randint(-j, j)
    if HUMAN.get("on"):
        import random
        human_move(x, y)
        hold_ms = random.randint(max(20, hold_ms - 10), hold_ms + 40)
    else:
        move_to(x, y)
    time.sleep(0.03)
    down, up = (MOUSEEVENTF_RIGHTDOWN, MOUSEEVENTF_RIGHTUP) if button == "right" else \
        (MOUSEEVENTF_LEFTDOWN, MOUSEEVENTF_LEFTUP)
    _send(_mouse(down))
    time.sleep(max(0, hold_ms) / 1000)
    _send(_mouse(up))


def scroll(amount):
    _send(_mouse(MOUSEEVENTF_WHEEL, data=ctypes.c_uint32(int(amount) * 120).value))


def type_text(text, gap_ms=20):
    """영문/숫자/기호 위주. 한글 등은 유니코드 방식으로 보냄"""
    if not IS_WIN:
        return
    for ch in text:
        k = ch.lower()
        if k in VK and (len(k) == 1):
            if ch.isupper():
                _send(_key_input("shift", False))
            key_tap(k, 15)
            if ch.isupper():
                _send(_key_input("shift", True))
        else:
            code = ord(ch)
            _send(INPUT(type=INPUT_KEYBOARD, u=_U(ki=KEYBDINPUT(0, code, 0x4, 0, 0))),
                  INPUT(type=INPUT_KEYBOARD, u=_U(ki=KEYBDINPUT(0, code, 0x4 | KEYEVENTF_KEYUP, 0, 0))))
        time.sleep(gap_ms / 1000)


CF_UNICODETEXT, GMEM_MOVEABLE = 13, 0x0002
if IS_WIN:
    _k32 = ctypes.WinDLL("kernel32", use_last_error=True)
    _k32.GlobalAlloc.argtypes = (wintypes.UINT, ctypes.c_size_t)
    _k32.GlobalAlloc.restype = ctypes.c_void_p
    _k32.GlobalLock.argtypes = (ctypes.c_void_p,)
    _k32.GlobalLock.restype = ctypes.c_void_p
    _k32.GlobalUnlock.argtypes = (ctypes.c_void_p,)
    _k32.GlobalFree.argtypes = (ctypes.c_void_p,)
    _k32.GlobalFree.restype = ctypes.c_void_p
    user32.OpenClipboard.argtypes = (wintypes.HWND,)
    user32.GetClipboardData.argtypes = (wintypes.UINT,)
    user32.GetClipboardData.restype = ctypes.c_void_p
    user32.SetClipboardData.argtypes = (wintypes.UINT, ctypes.c_void_p)
    user32.SetClipboardData.restype = ctypes.c_void_p


def _open_clipboard():
    for _ in range(20):                       # 다른 프로그램이 잡고 있으면 잠깐 기다렸다 다시
        if user32.OpenClipboard(None):
            return True
        time.sleep(0.01)
    return False


def get_clipboard():
    """지금 클립보드의 글자 (글자가 아니거나 못 읽으면 None)"""
    if not IS_WIN or not _open_clipboard():
        return None
    try:
        h = user32.GetClipboardData(CF_UNICODETEXT)
        if not h:
            return None
        p = _k32.GlobalLock(h)
        if not p:
            return None
        try:
            return ctypes.wstring_at(p)
        finally:
            _k32.GlobalUnlock(h)
    finally:
        user32.CloseClipboard()


def set_clipboard(text):
    if not IS_WIN:
        return False
    data = ctypes.create_unicode_buffer(str(text))
    size = ctypes.sizeof(data)
    h = _k32.GlobalAlloc(GMEM_MOVEABLE, size)
    if not h:
        return False
    p = _k32.GlobalLock(h)
    if not p:
        _k32.GlobalFree(h)
        return False
    ctypes.memmove(p, data, size)
    _k32.GlobalUnlock(h)
    if not _open_clipboard():
        _k32.GlobalFree(h)
        return False
    try:
        user32.EmptyClipboard()
        if not user32.SetClipboardData(CF_UNICODETEXT, h):
            _k32.GlobalFree(h)
            return False
        return True                           # 성공하면 메모리는 윈도우가 관리
    finally:
        user32.CloseClipboard()


def paste_text(text):
    """입력칸에 글자를 한 번에 넣기: 클립보드에 넣고 Ctrl+V (FishSol 방식)
    한 글자씩 치는 것보다 빠르고, 글자가 빠지거나 순서가 꼬이지 않음.
    원래 클립보드 글자는 붙여넣은 뒤 되돌려 둠. 클립보드를 못 쓰면 한 글자씩 입력으로 대신함"""
    if not IS_WIN:
        return
    text = str(text)
    old = get_clipboard()
    if not set_clipboard(text):
        type_text(text)
        return
    time.sleep(0.05)
    key_combo(["ctrl", "v"], 30)
    time.sleep(0.35)                          # 게임이 붙여넣기를 처리할 시간 (이후 엔터)
    if old is not None and old != text:
        set_clipboard(old)


def key_down_now(name):
    """지금 그 키가 눌려 있는지 (정지 단축키, 위치 지정용)"""
    if not IS_WIN:
        return False
    return bool(user32.GetAsyncKeyState(VK[name.lower()]) & 0x8000)


def cursor_pos():
    if not IS_WIN:
        return 0, 0
    pt = wintypes.POINT()
    user32.GetCursorPos(ctypes.byref(pt))
    return pt.x, pt.y


# ---------------------------------------------------------------- 로블록스 창
def roblox_window():
    """로블록스 게임 창 핸들 (없으면 None)
    제목이 'Roblox' 인 '보이는' 창 중에서 로블록스 클라이언트 창(WINDOWSCLIENT)을 우선, 가장 큰 것"""
    if not IS_WIN:
        return None
    found = []
    proc = ctypes.WINFUNCTYPE(wintypes.BOOL, wintypes.HWND, wintypes.LPARAM)

    def cb(h, _):
        try:
            if not user32.IsWindowVisible(h):
                return True
            buf = ctypes.create_unicode_buffer(64)
            user32.GetWindowTextW(h, buf, 64)
            if buf.value != "Roblox":
                return True
            cls = ctypes.create_unicode_buffer(64)
            user32.GetClassNameW(h, cls, 64)
            rc = wintypes.RECT()
            user32.GetClientRect(h, ctypes.byref(rc))
            area = (rc.right - rc.left) * (rc.bottom - rc.top)
            found.append((cls.value == "WINDOWSCLIENT", area, h))
        except Exception:
            pass
        return True
    try:
        user32.EnumWindows(proc(cb), 0)
    except Exception:
        pass
    if found:
        found.sort(reverse=True)
        return found[0][2]
    hwnd = user32.FindWindowW(None, "Roblox")
    return hwnd or None


def _exe_of(hwnd):
    """창을 만든 프로그램 이름 (예: Discord.exe)"""
    try:
        pid = wintypes.DWORD()
        user32.GetWindowThreadProcessId(hwnd, ctypes.byref(pid))
        k32 = ctypes.windll.kernel32
        h = k32.OpenProcess(0x1000, False, pid.value)          # PROCESS_QUERY_LIMITED_INFORMATION
        if not h:
            return ""
        try:
            buf = ctypes.create_unicode_buffer(520)
            n = wintypes.DWORD(520)
            if k32.QueryFullProcessImageNameW(h, 0, buf, ctypes.byref(n)):
                return os.path.basename(buf.value)
        finally:
            k32.CloseHandle(h)
    except Exception:
        pass
    return ""


def list_windows():
    """화면에 보이는 창 목록 [{title, exe}] (제목 있는 것만)"""
    if not IS_WIN:
        return []
    out = []
    proc = ctypes.WINFUNCTYPE(wintypes.BOOL, wintypes.HWND, wintypes.LPARAM)

    def cb(h, _):
        try:
            if user32.IsWindowVisible(h) and not user32.GetWindow(h, 4):     # 4 = GW_OWNER (팝업 제외)
                n = user32.GetWindowTextLengthW(h)
                if n:
                    buf = ctypes.create_unicode_buffer(n + 1)
                    user32.GetWindowTextW(h, buf, n + 1)
                    out.append({"hwnd": h, "title": buf.value, "exe": _exe_of(h)})
        except Exception:
            pass
        return True
    try:
        user32.EnumWindows(proc(cb), 0)
    except Exception:
        pass
    return out


def find_window(title="", exe=""):
    """제목에 title 이 들어가거나 / 프로그램 이름이 exe 인 보이는 창 (대소문자 무시)"""
    t, e = (title or "").strip().lower(), (exe or "").strip().lower()
    if not t and not e:
        return None
    for w in list_windows():
        if (not e or w["exe"].lower() == e) and (not t or t in w["title"].lower()):
            return w["hwnd"]
    return None


_WIN_CACHE = [0.0, None]    # (확인 시각, 핸들)


def roblox_window_cached(max_age=1.0):
    """자주 부르는 곳용: 1초 안에 찾은 창이 아직 살아 있으면 그대로 사용 (창 전체 검색을 줄임)"""
    if not IS_WIN:
        return None
    t, h = _WIN_CACHE
    if h and time.time() - t < max_age and user32.IsWindow(h) and user32.IsWindowVisible(h):
        return h
    h = roblox_window()
    _WIN_CACHE[0], _WIN_CACHE[1] = time.time(), h
    return h


class fast_timing:
    """with fast_timing(): 동안만 윈도우 타이머를 1ms 단위로 + 이 프로그램 우선순위를 '높음'으로
    (컴퓨터가 바쁠 때 클릭 간격이 밀리는 것을 줄임 · 끝나면 원래대로)"""

    def __enter__(self):
        self.ok_timer = self.ok_prio = False
        if not IS_WIN:
            return self
        try:
            self.ok_timer = ctypes.windll.winmm.timeBeginPeriod(1) == 0
        except Exception:
            pass
        try:
            k32 = ctypes.windll.kernel32
            proc = k32.GetCurrentProcess()
            self.old_prio = k32.GetPriorityClass(proc)
            self.ok_prio = bool(k32.SetPriorityClass(proc, 0x80))          # HIGH_PRIORITY_CLASS
            k32.SetThreadPriority(k32.GetCurrentThread(), 2)                # THREAD_PRIORITY_HIGHEST
        except Exception:
            pass
        return self

    def __exit__(self, *exc):
        if not IS_WIN:
            return False
        try:
            k32 = ctypes.windll.kernel32
            k32.SetThreadPriority(k32.GetCurrentThread(), 0)
            if self.ok_prio:
                k32.SetPriorityClass(k32.GetCurrentProcess(), self.old_prio or 0x20)   # 원래대로 (기본 NORMAL)
        except Exception:
            pass
        try:
            if self.ok_timer:
                ctypes.windll.winmm.timeEndPeriod(1)
        except Exception:
            pass
        return False


def client_rect(hwnd):
    """창 안쪽 영역의 화면 좌표 (left, top, width, height)"""
    if not IS_WIN or not hwnd:
        return None
    rc = wintypes.RECT()
    if not user32.GetClientRect(hwnd, ctypes.byref(rc)):
        return None
    pt = wintypes.POINT(0, 0)
    user32.ClientToScreen(hwnd, ctypes.byref(pt))
    return pt.x, pt.y, rc.right - rc.left, rc.bottom - rc.top


def is_foreground(hwnd):
    return IS_WIN and bool(hwnd) and user32.GetForegroundWindow() == hwnd


def focus(hwnd, wait=0.0):
    """로블록스 창을 맨 앞으로. 이미 앞에 있으면 아무것도 안 함. wait: 앞으로 가져왔을 때 화면이 바뀔 시간"""
    if not IS_WIN or not hwnd:
        return
    if is_foreground(hwnd) and not user32.IsIconic(hwnd):
        return
    try:
        if user32.IsIconic(hwnd):
            user32.ShowWindow(hwnd, 9)        # 최소화 상태면 복원
        # 다른 프로그램이 앞에 있어도 가져올 수 있도록 alt 를 한 번 눌렀다 뗌
        _send(_key_input("alt", False), _key_input("alt", True))
        user32.BringWindowToTop(hwnd)
        user32.SetForegroundWindow(hwnd)
        for _ in range(25):                   # 실제로 앞으로 올 때까지 최대 0.5초
            if is_foreground(hwnd):
                break
            time.sleep(0.02)
        if wait:
            time.sleep(wait)
    except Exception:
        pass


def to_screen(rx, ry, rect):
    left, top, w, h = rect
    return int(left + rx * w), int(top + ry * h)


def to_ratio(x, y, rect):
    left, top, w, h = rect
    return round((x - left) / max(1, w), 4), round((y - top) / max(1, h), 4)


def pick_point(key="f8", timeout=30.0, stop=None):
    """마우스를 원하는 곳에 두고 key 를 누르면 그 위치를 로블록스 창 기준 비율로 반환"""
    end = time.time() + timeout
    while key_down_now(key):          # 이미 눌려 있으면 뗄 때까지
        time.sleep(0.02)
    while time.time() < end:
        if stop and stop.is_set():
            return None
        if key_down_now(key):
            x, y = cursor_pos()
            hwnd = roblox_window()
            rect = client_rect(hwnd)
            while key_down_now(key):
                time.sleep(0.02)
            if not rect:
                return {"error": "로블록스 창을 찾을 수 없음"}
            rx, ry = to_ratio(x, y, rect)
            return {"x": rx, "y": ry}
        time.sleep(0.02)
    return {"error": "시간 초과"}


# ---------------------------------------------------------------- 드래그로 영역 지정
# 위치 지정 창 · 파일 선택 창 안내 글자 (앱이 ACRUX_LANG 로 화면 언어를 넘겨줌)
_LANG = os.environ.get("ACRUX_LANG", "ko")
_UI_FONT = {"ko": "Malgun Gothic", "en": "Segoe UI", "ja": "Yu Gothic UI"}
_GUIDE = {
    "ko": {"point": "클릭할 위치를 한 번 클릭   ·   Esc 취소", "region": "드래그로 OCR 영역 선택   ·   Esc 취소",
           "file": "실행할 프로그램 선택", "prog": "프로그램", "all": "모든 파일"},
    "en": {"point": "Click the position once   ·   Esc to cancel", "region": "Drag to select the OCR area   ·   Esc to cancel",
           "file": "Choose a program to run", "prog": "Programs", "all": "All files"},
    "ja": {"point": "クリックする位置を1回クリック   ·   Escでキャンセル", "region": "ドラッグでOCR範囲を選択   ·   Escでキャンセル",
           "file": "実行するプログラムを選択", "prog": "プログラム", "all": "すべてのファイル"},
}


def pick_region_overlay(mode="region"):
    """로블록스 화면 위에 반투명 창을 띄우고 선택 → 창 기준 비율
    mode="region": 드래그로 영역 → {"region": [x1, y1, x2, y2]}
    mode="point":  클릭 1번으로 위치 → {"x": rx, "y": ry}
    (tkinter 를 쓰므로 별도 프로세스에서 실행: python macro.py --pick-region / --pick-point)"""
    import base64
    import tkinter as tk
    import mss
    import mss.tools

    hwnd = roblox_window()
    rect = client_rect(hwnd)
    if not rect or rect[2] < 10 or rect[3] < 10:
        return {"error": "로블록스 창을 찾을 수 없음"}
    focus(hwnd, wait=0.35)                # 로블록스가 앞으로 나온 뒤 캡처
    left, top, w, h = rect
    with mss.mss() as s:
        shot = s.grab({"left": left, "top": top, "width": w, "height": h})
        png = mss.tools.to_png(shot.rgb, shot.size)

    result = {"error": "취소됨"}
    root = tk.Tk()
    root.overrideredirect(True)
    root.attributes("-topmost", True)
    root.geometry(f"{w}x{h}+{left}+{top}")
    cv = tk.Canvas(root, width=w, height=h, highlightthickness=0, bd=0, cursor="crosshair", bg="black")
    cv.pack()
    img = tk.PhotoImage(data=base64.b64encode(png))
    cv.create_image(0, 0, image=img, anchor="nw")
    cv.create_rectangle(0, 0, w, h, fill="black", stipple="gray50", outline="", tags="dim")
    bar = cv.create_rectangle(0, 0, w, 34, fill="#1e1f22", outline="")
    guide = _GUIDE.get(_LANG, _GUIDE["ko"])["point" if mode == "point" else "region"]
    cv.create_text(w // 2, 17, text=guide, fill="#ffffff", font=(_UI_FONT.get(_LANG, "Malgun Gothic"), 11, "bold"))
    state = {"x0": 0, "y0": 0}

    if mode == "point":
        def motion(e):                       # 마우스를 따라다니는 십자선
            cv.delete("cross")
            cv.create_line(e.x, 34, e.x, h, fill="#e67e22", dash=(4, 3), tags="cross")
            cv.create_line(0, e.y, w, e.y, fill="#e67e22", dash=(4, 3), tags="cross")
            cv.create_oval(e.x - 6, e.y - 6, e.x + 6, e.y + 6, outline="#e67e22", width=2, tags="cross")

        def click(e):
            if e.y < 34:
                return                       # 안내 바 위는 무시
            result.clear()
            result["x"] = round(max(0, min(w, e.x)) / w, 4)
            result["y"] = round(max(0, min(h, e.y)) / h, 4)
            cv.delete("cross")
            cv.create_oval(e.x - 8, e.y - 8, e.x + 8, e.y + 8, fill="#e67e22", outline="#ffffff", width=2)
            root.after(250, root.destroy)    # 찍힌 위치를 잠깐 보여주고 닫음

        cv.bind("<Motion>", motion)
        cv.bind("<ButtonPress-1>", click)
        root.bind("<Escape>", lambda e: root.destroy())
        root.after(120000, root.destroy)
        root.focus_force()
        root.mainloop()
        return result

    def press(e):
        state["x0"], state["y0"] = e.x, e.y
        cv.delete("sel")

    def drag(e):
        cv.delete("sel")
        x0, y0 = state["x0"], state["y0"]
        cv.create_rectangle(x0, y0, e.x, e.y, outline="#e67e22", width=2, tags="sel")
        cv.create_text(min(x0, e.x) + 4, min(y0, e.y) - 10, anchor="w", fill="#e67e22", tags="sel",
                       text=f"{abs(e.x - x0)} × {abs(e.y - y0)}", font=("Consolas", 10, "bold"))

    def release(e):
        x0, y0 = state["x0"], state["y0"]
        if abs(e.x - x0) < 4 or abs(e.y - y0) < 4:
            return                           # 너무 작으면 다시 드래그
        x1, x2 = sorted((max(0, min(w, x0)), max(0, min(w, e.x))))
        y1, y2 = sorted((max(0, min(h, y0)), max(0, min(h, e.y))))
        result.clear()
        result["region"] = [round(x1 / w, 4), round(y1 / h, 4), round(x2 / w, 4), round(y2 / h, 4)]
        root.destroy()

    cv.bind("<ButtonPress-1>", press)
    cv.bind("<B1-Motion>", drag)
    cv.bind("<ButtonRelease-1>", release)
    root.bind("<Escape>", lambda e: root.destroy())
    root.after(120000, root.destroy)         # 2분 지나면 자동 취소
    root.focus_force()
    root.mainloop()
    return result


# ---------------------------------------------------------------- 화면 캡처 / OCR
SHOT_DIR = None


def grab(region):
    """region = (left, top, width, height) 화면 좌표 → (bgra bytes, width, height)"""
    import mss
    with mss.mss() as s:
        img = s.grab({"left": int(region[0]), "top": int(region[1]),
                      "width": max(1, int(region[2])), "height": max(1, int(region[3]))})
        return bytes(img.bgra), img.width, img.height


def save_screenshot(folder, rect=None):
    import mss
    import mss.tools
    Path(folder).mkdir(parents=True, exist_ok=True)
    path = Path(folder) / time.strftime("shot_%Y%m%d_%H%M%S.png")
    with mss.mss() as s:
        mon = {"left": rect[0], "top": rect[1], "width": rect[2], "height": rect[3]} if rect else s.monitors[1]
        img = s.grab(mon)
        mss.tools.to_png(img.rgb, img.size, output=str(path))
    return str(path)


def _upscale_bgra(data, w, h, k):
    """작은 글씨 인식률을 위해 k배 확대 (최근접)"""
    if k <= 1:
        return data, w, h
    src = memoryview(data)
    rows = []
    for y in range(h):
        row = src[y * w * 4:(y + 1) * w * 4]
        big = b"".join(bytes(row[x * 4:x * 4 + 4]) * k for x in range(w))
        rows.append(big * k)
    return b"".join(rows), w * k, h * k


def _downscale_bgra(data, w, h, k):
    """너무 큰 영역은 1/k 로 축소 (윈도우 OCR 최대 크기 제한)"""
    src = memoryview(data)
    nw, nh = max(1, w // k), max(1, h // k)
    rows = []
    for y in range(nh):
        sy = y * k * w * 4
        rows.append(b"".join(bytes(src[sy + x * k * 4: sy + x * k * 4 + 4]) for x in range(nw)))
    return b"".join(rows), nw, nh


OCR_TIMEOUT = 12.0


def _ocr_worker(data, w, h):
    """윈도우 내장 OCR. asyncio 대신 작업 상태를 직접 확인 (스레드에서 멈추는 문제 방지)"""
    try:
        from winrt.windows.foundation import AsyncStatus
    except Exception:
        AsyncStatus = None
    from winrt.windows.media.ocr import OcrEngine
    from winrt.windows.graphics.imaging import SoftwareBitmap, BitmapPixelFormat
    from winrt.windows.storage.streams import DataWriter

    if w * h < 200 * 60:                     # 작은 영역은 확대해서 읽기
        data, w, h = _upscale_bgra(data, w, h, 2)
    try:
        limit = int(OcrEngine.max_image_dimension)
    except Exception:
        limit = 2600
    k = 1
    while max(w, h) // k > limit:
        k += 1
    if k > 1:
        data, w, h = _downscale_bgra(data, w, h, k)

    engine = OcrEngine.try_create_from_user_profile_languages()
    if engine is None:
        raise RuntimeError("윈도우 OCR 언어 없음 (윈도우 설정 → 언어에서 한국어/영어 언어팩 설치 필요)")
    dw = DataWriter()
    dw.write_bytes(data)
    bmp = SoftwareBitmap.create_copy_from_buffer(dw.detach_buffer(), BitmapPixelFormat.BGRA8, w, h)
    op = engine.recognize_async(bmp)

    if hasattr(op, "status") and hasattr(op, "get_results"):
        end = time.time() + OCR_TIMEOUT
        while int(op.status) == 0:           # 0 = Started (진행 중)
            if time.time() > end:
                try:
                    op.cancel()
                except Exception:
                    pass
                raise TimeoutError("OCR 시간 초과")
            time.sleep(0.02)
        st = int(op.status)
        if st != 1:                          # 1 = Completed
            code = getattr(op, "error_code", None)
            raise RuntimeError(f"OCR 실패 (상태 {st}{', ' + str(code) if code is not None else ''})")
        return op.get_results().text

    # 예전 winrt 버전: await 방식
    import asyncio

    async def wait_op():
        return await op
    return asyncio.run(asyncio.wait_for(wait_op(), OCR_TIMEOUT)).text


# ---- RapidOCR (PaddleOCR 모델을 onnxruntime 으로 실행) — 윈도우 OCR 보다 훨씬 정확, 게임 글자(테두리·배경색)에 강함
_RAPID = {"engine": None, "failed": None}
_RAPID_LOCK = threading.Lock()


def rapid_engine():
    """설치돼 있으면 RapidOCR 엔진 (처음 한 번만 불러옴), 없으면 None"""
    with _RAPID_LOCK:
        if _RAPID["engine"] is None and _RAPID["failed"] is None:
            try:
                from rapidocr_onnxruntime import RapidOCR
                _RAPID["engine"] = RapidOCR()
            except Exception as e:
                _RAPID["failed"] = str(e) or type(e).__name__
        return _RAPID["engine"]


def ocr_engine_name():
    return "RapidOCR" if rapid_engine() else "윈도우 OCR"


def _rapid_worker(eng, data, w, h):
    import numpy as np
    img = np.frombuffer(data, dtype=np.uint8).reshape(h, w, 4)[:, :, :3].copy()   # BGRA → BGR
    if h < 48:                               # 작은 글씨는 키워서 읽기
        import cv2
        k = 48.0 / max(1, h)
        img = cv2.resize(img, (int(w * k), 48), interpolation=cv2.INTER_CUBIC)
    res, _ = eng(img)
    if not res:
        return ""
    # 글자 덩어리를 줄(위→아래) · 칸(왼→오) 순서로 이어 붙임
    items = []
    for box, text, _score in res:
        ys = [p[1] for p in box]
        xs = [p[0] for p in box]
        items.append(((min(ys) + max(ys)) / 2, min(xs), max(ys) - min(ys), text))
    items.sort()
    lines, cur, cy, ch = [], [], None, 0
    for y, x, hh, text in items:
        if cy is not None and abs(y - cy) > max(ch, hh) * 0.6:
            lines.append(cur)
            cur = []
        cur.append((x, text))
        cy, ch = y, hh
    if cur:
        lines.append(cur)
    return "\n".join(" ".join(t for _, t in sorted(line)) for line in lines)


def _rapid_count(eng, data, w, h):
    """아이템 칸 오른쪽 아래의 작은 개수 글자(x23)만 따로 크게 읽기
    (흰 글자만 남기고 3배 확대 → 안 되면 2배 확대 원본)"""
    import re
    import numpy as np
    import cv2
    img = np.frombuffer(data, dtype=np.uint8).reshape(h, w, 4)[:, :, :3]
    crop = np.ascontiguousarray(img[int(h * 0.6):h, int(w * 0.4):w])
    if crop.size == 0:
        return None
    gray = cv2.cvtColor(crop, cv2.COLOR_BGR2GRAY)
    _, th = cv2.threshold(gray, 170, 255, cv2.THRESH_BINARY_INV)
    tries = [cv2.cvtColor(cv2.resize(th, None, fx=3, fy=3, interpolation=cv2.INTER_CUBIC), cv2.COLOR_GRAY2BGR),
             cv2.resize(crop, None, fx=2, fy=2, interpolation=cv2.INTER_CUBIC)]
    for im in tries:
        res, _ = eng(im)
        for r in res or []:
            m = re.search(r"[x×X]\s*([0-9][0-9,\.]*)", r[1])
            if m:
                return "x" + m.group(1)
    return None


def ocr_item_bgra(data, w, h):
    """아이템 칸 읽기: 이름 + 개수. 전체를 읽고, 개수를 못 찾으면 오른쪽 아래만 따로 다시 읽음"""
    import re
    text = ocr_bgra(data, w, h)
    eng = rapid_engine()
    if eng is not None and not re.search(r"[x×X]\s*[0-9]", text or ""):
        try:
            cnt = _rapid_count(eng, data, w, h)
        except Exception:
            cnt = None
        if cnt:
            # 전체 읽기에서 섞여 들어온 깨진 개수 조각은 빼고 붙임
            lines = [ln for ln in (text or "").splitlines() if not re.fullmatch(r"\s*[x×X]\S*\s*", ln)]
            text = "\n".join(lines + [cnt])
    return text


def ocr_bgra(data, w, h):
    """별도 스레드에서 OCR 실행. 어떤 경우에도 OCR_TIMEOUT 안에 결과 또는 오류를 돌려줌
    RapidOCR 가 설치돼 있으면 그걸로, 아니면 윈도우 내장 OCR"""
    box = {}

    def run():
        try:
            eng = rapid_engine()
            if eng is not None:
                box["text"] = _rapid_worker(eng, data, w, h)
                return
            box["text"] = _ocr_worker(data, w, h)
        except BaseException as e:
            box["err"] = e
    t = threading.Thread(target=run, daemon=True)
    t.start()
    t.join(OCR_TIMEOUT + 3)
    if t.is_alive():
        raise TimeoutError("OCR 응답 없음 (시간 초과)")
    if "err" in box:
        raise box["err"]
    return box.get("text", "")


def ocr_region(region_ratio, item=False):
    """로블록스 창 기준 비율 영역 [x1, y1, x2, y2] 을 읽음 (먼저 로블록스 창을 맨 앞으로)
    item=True: 아이템 칸(이름 + 오른쪽 아래 개수)으로 읽기"""
    hwnd = roblox_window()
    focus(hwnd, wait=0.25)
    rect = client_rect(hwnd)
    if not rect:
        raise RuntimeError("로블록스 창을 찾을 수 없음")
    x1, y1 = to_screen(min(region_ratio[0], region_ratio[2]), min(region_ratio[1], region_ratio[3]), rect)
    x2, y2 = to_screen(max(region_ratio[0], region_ratio[2]), max(region_ratio[1], region_ratio[3]), rect)
    data, w, h = grab((x1, y1, x2 - x1, y2 - y1))
    return ocr_item_bgra(data, w, h) if item else ocr_bgra(data, w, h)


def wait_for_roblox(timeout=90.0, stop=None):
    """로블록스 창이 뜰 때까지 기다림"""
    end = time.time() + timeout
    while time.time() < end:
        if stop and stop.is_set():
            return None
        h = roblox_window()
        if h:
            return h
        time.sleep(0.5)
    return None


def pick_region_to_file(path, mode="region"):
    """결과를 파일로 저장 (창 없는 실행/exe 에선 표준 출력이 없어서 파일로 주고받음)"""
    import json as _json
    try:
        res = pick_region_overlay(mode)
    except Exception as e:
        res = {"error": str(e)}
    Path(path).write_text(_json.dumps(res, ensure_ascii=False), encoding="utf-8")


def pick_file_to_file(path):
    """프로그램 고르기 창 (tkinter) — 고른 경로를 파일로 저장"""
    import json as _json
    res = {"error": "취소됨"}
    try:
        import tkinter as tk
        from tkinter import filedialog
        root = tk.Tk()
        root.withdraw()
        root.attributes("-topmost", True)
        g = _GUIDE.get(_LANG, _GUIDE["ko"])
        p = filedialog.askopenfilename(title=g["file"], parent=root,
                                       filetypes=[(g["prog"], "*.exe *.lnk *.bat *.cmd *.ahk *.py *.pyw"), (g["all"], "*.*")])
        root.destroy()
        if p:
            res = {"path": str(Path(p))}
    except Exception as e:
        res = {"error": str(e)}
    Path(path).write_text(_json.dumps(res, ensure_ascii=False), encoding="utf-8")


if __name__ == "__main__" and "--pick-file" in sys.argv:
    pick_file_to_file(sys.argv[sys.argv.index("--pick-file") + 1])
elif __name__ == "__main__" and "--pick-region" in sys.argv:
    pick_region_to_file(sys.argv[sys.argv.index("--pick-region") + 1])
elif __name__ == "__main__" and "--pick-point" in sys.argv:
    pick_region_to_file(sys.argv[sys.argv.index("--pick-point") + 1], "point")
