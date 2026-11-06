# -*- coding: utf-8 -*-
"""
Acrux 창(Edge 앱 창) 아이콘을 Acrux 아이콘으로 바꿈

Edge 앱 창은 작업 표시줄에서 Edge 와 같은 묶음으로 잡혀서 Edge/예전 아이콘이 보일 수 있음.
→ 창에 Acrux 전용 앱 ID(AppUserModelID)를 붙여서 따로 묶이게 하고, 창 아이콘(큰/작은)을 icon.ico 로 지정.
Edge 가 페이지를 다시 그리면서 아이콘을 되돌릴 수 있어서, 창이 떠 있는 동안 가끔 다시 확인함.
실패해도 프로그램 동작에는 영향 없음 (아이콘만 그대로).
"""
import os
import threading
import time

APP_ID = "Acrux.Macro"
TITLE_KEY = "Acrux snipe"          # 창 제목 (index.html 의 <title>) 에 들어 있는 글자


def _api():
    import ctypes
    from ctypes import wintypes
    u32 = ctypes.WinDLL("user32", use_last_error=True)
    sh = ctypes.WinDLL("shell32")
    ole = ctypes.WinDLL("ole32")
    u32.LoadImageW.argtypes = (wintypes.HINSTANCE, wintypes.LPCWSTR, wintypes.UINT, ctypes.c_int, ctypes.c_int, wintypes.UINT)
    u32.LoadImageW.restype = wintypes.HANDLE
    u32.SendMessageW.argtypes = (wintypes.HWND, wintypes.UINT, wintypes.WPARAM, wintypes.LPARAM)
    u32.SendMessageW.restype = ctypes.c_ssize_t
    u32.GetClassNameW.argtypes = (wintypes.HWND, wintypes.LPWSTR, ctypes.c_int)
    u32.GetWindowTextW.argtypes = (wintypes.HWND, wintypes.LPWSTR, ctypes.c_int)
    u32.IsWindowVisible.argtypes = (wintypes.HWND,)
    u32.IsWindow.argtypes = (wintypes.HWND,)
    u32.GetSystemMetrics.argtypes = (ctypes.c_int,)
    sh.SHGetPropertyStoreForWindow.argtypes = (wintypes.HWND, ctypes.c_void_p, ctypes.POINTER(ctypes.c_void_p))
    sh.SHGetPropertyStoreForWindow.restype = ctypes.c_long
    return ctypes, wintypes, u32, sh, ole


def _guid(ctypes, s):
    import uuid

    class GUID(ctypes.Structure):
        _fields_ = [("d1", ctypes.c_uint32), ("d2", ctypes.c_uint16), ("d3", ctypes.c_uint16), ("d4", ctypes.c_ubyte * 8)]
    u = uuid.UUID(s)
    g = GUID(u.time_low, u.time_mid, u.time_hi_version)
    for i, b in enumerate(u.bytes[8:]):
        g.d4[i] = b
    return g, GUID


def find_windows():
    """제목에 Acrux 가 들어간 Edge 앱 창 목록"""
    ctypes, wintypes, u32, _, _ = _api()
    found = []
    proc = ctypes.WINFUNCTYPE(wintypes.BOOL, wintypes.HWND, wintypes.LPARAM)

    def cb(h, _):
        try:
            if not u32.IsWindowVisible(h):
                return True
            cls = ctypes.create_unicode_buffer(64)
            u32.GetClassNameW(h, cls, 64)
            if not cls.value.startswith("Chrome_WidgetWin"):
                return True
            t = ctypes.create_unicode_buffer(256)
            u32.GetWindowTextW(h, t, 256)
            if TITLE_KEY in t.value:
                found.append(h)
        except Exception:
            pass
        return True
    u32.EnumWindows(proc(cb), 0)
    return found


def set_app_id(hwnd):
    """창에 Acrux 전용 앱 ID → 작업 표시줄에서 Edge 와 따로 묶이고, 창 아이콘이 그대로 보임"""
    ctypes, wintypes, u32, sh, ole = _api()
    iid, _ = _guid(ctypes, "886D8EEB-8CF2-4446-8D02-CDBA1DBDCF99")          # IID_IPropertyStore
    fmtid, GUID = _guid(ctypes, "9F4C2855-9F79-4B39-A8D0-E1D42DE1D5F3")    # PKEY_AppUserModel_*

    class PROPERTYKEY(ctypes.Structure):
        _fields_ = [("fmtid", GUID), ("pid", ctypes.c_uint32)]

    class PROPVARIANT(ctypes.Structure):
        _fields_ = [("vt", ctypes.c_ushort), ("r1", ctypes.c_ushort), ("r2", ctypes.c_ushort), ("r3", ctypes.c_ushort),
                    ("val", ctypes.c_void_p), ("pad", ctypes.c_void_p)]

    store = ctypes.c_void_p()
    if sh.SHGetPropertyStoreForWindow(hwnd, ctypes.byref(iid), ctypes.byref(store)) != 0 or not store:
        return False
    vtbl = ctypes.cast(store, ctypes.POINTER(ctypes.POINTER(ctypes.c_void_p)))[0]
    HR = ctypes.c_long
    set_value = ctypes.WINFUNCTYPE(HR, ctypes.c_void_p, ctypes.POINTER(PROPERTYKEY), ctypes.POINTER(PROPVARIANT))(vtbl[6])
    commit = ctypes.WINFUNCTYPE(HR, ctypes.c_void_p)(vtbl[7])
    release = ctypes.WINFUNCTYPE(ctypes.c_ulong, ctypes.c_void_p)(vtbl[2])
    try:
        buf = ctypes.create_unicode_buffer(APP_ID)
        pv = PROPVARIANT(31, 0, 0, 0, ctypes.cast(buf, ctypes.c_void_p).value, None)    # VT_LPWSTR
        key = PROPERTYKEY(fmtid, 5)                                                       # PKEY_AppUserModel_ID
        ok = set_value(store, ctypes.byref(key), ctypes.byref(pv)) == 0
        commit(store)
        return ok
    finally:
        release(store)


_ICONS = {}


def set_icon(hwnd, ico_path):
    ctypes, wintypes, u32, _, _ = _api()
    if not _ICONS:
        for which, (mx, my) in ((1, (11, 12)), (0, (49, 50))):     # ICON_BIG: SM_CXICON / ICON_SMALL: SM_CXSMICON
            w, h = u32.GetSystemMetrics(mx), u32.GetSystemMetrics(my)
            _ICONS[which] = u32.LoadImageW(None, str(ico_path), 1, w, h, 0x10)   # IMAGE_ICON, LR_LOADFROMFILE
    for which, hicon in _ICONS.items():
        if hicon and u32.SendMessageW(hwnd, 0x7F, which, 0) != hicon:          # WM_GETICON: 이미 같으면 그대로
            u32.SendMessageW(hwnd, 0x80, which, hicon)                         # WM_SETICON


def fit_window(hwnd, w, h):
    """창을 w x h 비율로 (화면 작업 영역에 맞게 줄여서) 가운데에 놓음"""
    ctypes, wintypes, u32, _, _ = _api()

    class MONITORINFO(ctypes.Structure):
        _fields_ = [("cbSize", wintypes.DWORD), ("rcMonitor", wintypes.RECT), ("rcWork", wintypes.RECT), ("dwFlags", wintypes.DWORD)]
    u32.MonitorFromWindow.argtypes = (wintypes.HWND, wintypes.DWORD)
    u32.MonitorFromWindow.restype = wintypes.HANDLE
    u32.GetMonitorInfoW.argtypes = (wintypes.HANDLE, ctypes.POINTER(MONITORINFO))
    u32.SetWindowPos.argtypes = (wintypes.HWND, wintypes.HWND, ctypes.c_int, ctypes.c_int, ctypes.c_int, ctypes.c_int, wintypes.UINT)
    u32.IsZoomed.argtypes = (wintypes.HWND,)
    if u32.IsZoomed(hwnd):
        return
    mi = MONITORINFO()
    mi.cbSize = ctypes.sizeof(mi)
    if not u32.GetMonitorInfoW(u32.MonitorFromWindow(hwnd, 2), ctypes.byref(mi)):     # MONITOR_DEFAULTTONEAREST
        return
    wa = mi.rcWork
    aw, ah = wa.right - wa.left, wa.bottom - wa.top
    k = min(1.0, (aw - 40) / w, (ah - 40) / h)          # 화면이 작으면 비율 그대로 줄임
    nw, nh = int(w * k), int(h * k)
    x, y = wa.left + (aw - nw) // 2, wa.top + (ah - nh) // 2
    u32.SetWindowPos(hwnd, None, x, y, nw, nh, 0x0004 | 0x0010)                   # SWP_NOZORDER | SWP_NOACTIVATE


def keep(ico_path, alive=lambda: True, fit=None):
    """창이 떠 있는 동안 아이콘 · 앱 ID 를 유지 (처음 1분은 1초마다, 그 뒤 5초마다)"""
    if os.name != "nt" or not os.path.isfile(ico_path):
        return

    def run():
        try:
            ole = _api()[4]
            ole.CoInitialize(None)
        except Exception:
            pass
        done_id = set()
        start = time.time()
        while alive():
            try:
                for h in find_windows():
                    if h not in done_id:
                        try:
                            set_app_id(h)
                        except Exception:
                            pass
                        if fit:
                            try:
                                fit_window(h, *fit)
                            except Exception:
                                pass
                        done_id.add(h)
                    set_icon(h, ico_path)
            except Exception:
                pass
            time.sleep(1 if time.time() - start < 60 else 5)
    threading.Thread(target=run, daemon=True).start()
