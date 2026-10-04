# -*- coding: utf-8 -*-
"""
프로세스 찾기 · 강제 종료 (윈도우 API 직접 사용)

예전에는 taskkill / tasklist 명령을 썼는데, 이 명령들은 내부적으로 WMI 를 거쳐서
PC 상태에 따라 가끔 몇 초씩 멈추거나 응답이 없어서 → 시간 초과로 '강제 종료'가 통째로 안 되는 경우가 있었음.
여기서는 윈도우 API(CreateToolhelp32Snapshot / TerminateProcess)를 바로 불러서 빠르고 확실하게 처리하고,
그래도 안 되는 경우에만 taskkill 로 한 번 더 시도함.
"""
import os
import subprocess
import time

IS_WIN = os.name == "nt"
NO_WINDOW = getattr(subprocess, "CREATE_NO_WINDOW", 0)
_W = {}


def _win():
    if _W:
        return _W
    import ctypes
    from ctypes import wintypes
    k32 = ctypes.WinDLL("kernel32", use_last_error=True)

    class PROCESSENTRY32W(ctypes.Structure):
        _fields_ = [("dwSize", wintypes.DWORD), ("cntUsage", wintypes.DWORD),
                    ("th32ProcessID", wintypes.DWORD), ("th32DefaultHeapID", ctypes.c_size_t),
                    ("th32ModuleID", wintypes.DWORD), ("cntThreads", wintypes.DWORD),
                    ("th32ParentProcessID", wintypes.DWORD), ("pcPriClassBase", wintypes.LONG),
                    ("dwFlags", wintypes.DWORD), ("szExeFile", wintypes.WCHAR * 260)]

    k32.CreateToolhelp32Snapshot.argtypes = (wintypes.DWORD, wintypes.DWORD)
    k32.CreateToolhelp32Snapshot.restype = wintypes.HANDLE
    k32.Process32FirstW.argtypes = (wintypes.HANDLE, ctypes.POINTER(PROCESSENTRY32W))
    k32.Process32FirstW.restype = wintypes.BOOL
    k32.Process32NextW.argtypes = (wintypes.HANDLE, ctypes.POINTER(PROCESSENTRY32W))
    k32.Process32NextW.restype = wintypes.BOOL
    k32.OpenProcess.argtypes = (wintypes.DWORD, wintypes.BOOL, wintypes.DWORD)
    k32.OpenProcess.restype = wintypes.HANDLE
    k32.TerminateProcess.argtypes = (wintypes.HANDLE, wintypes.UINT)
    k32.TerminateProcess.restype = wintypes.BOOL
    k32.WaitForSingleObject.argtypes = (wintypes.HANDLE, wintypes.DWORD)
    k32.WaitForSingleObject.restype = wintypes.DWORD
    k32.CloseHandle.argtypes = (wintypes.HANDLE,)
    _W.update(ctypes=ctypes, k32=k32, PE=PROCESSENTRY32W)
    return _W


def snapshot():
    """실행 중인 프로세스 전부 [(PID, 실행 파일 이름)]"""
    if not IS_WIN:
        return []
    w = _win()
    ct, k32 = w["ctypes"], w["k32"]
    h = k32.CreateToolhelp32Snapshot(0x2, 0)              # TH32CS_SNAPPROCESS
    if not h or h == ct.c_void_p(-1).value:
        raise OSError(ct.get_last_error(), "CreateToolhelp32Snapshot 실패")
    out = []
    try:
        pe = w["PE"]()
        pe.dwSize = ct.sizeof(pe)
        ok = k32.Process32FirstW(h, ct.byref(pe))
        while ok:
            out.append((int(pe.th32ProcessID), pe.szExeFile))
            ok = k32.Process32NextW(h, ct.byref(pe))
    finally:
        k32.CloseHandle(h)
    return out


def _norm(name):
    n = str(name or "").strip().strip('"').lower()
    return n if n.endswith(".exe") else n + ".exe"


def pids(*names):
    """그 이름(들)으로 실행 중인 프로세스 PID 목록 (대소문자 무시)"""
    want = {_norm(n) for n in names if str(n or "").strip()}
    try:
        return [pid for pid, exe in snapshot() if exe.lower() in want and pid]
    except Exception:
        return _pids_tasklist(want)


def running(name):
    return bool(pids(name))


def kill(names, wait=3.0):
    """이름이 같은 프로세스를 전부 강제 종료 → {"killed": [...], "denied": [...], "failed": [...]}
    killed: 실제로 종료한 프로그램 이름 / denied: 권한이 없어 못 끔 (관리자 권한으로 실행된 프로그램 등)"""
    names = [_norm(n) for n in names if str(n or "").strip()]
    res = {"killed": [], "denied": [], "failed": []}
    if not IS_WIN or not names:
        return res
    try:
        procs = [(pid, exe) for pid, exe in snapshot() if exe.lower() in set(names) and pid and pid != os.getpid()]
    except Exception:
        return _kill_taskkill(names, res)
    w = _win()
    ct, k32 = w["ctypes"], w["k32"]
    handles = []
    for pid, exe in procs:
        h = k32.OpenProcess(0x0001 | 0x00100000, False, pid)   # PROCESS_TERMINATE | SYNCHRONIZE
        if not h:
            err = ct.get_last_error()
            if err == 87:                       # 이미 꺼짐 (그 사이에 종료됨)
                continue
            (res["denied"] if err == 5 else res["failed"]).append(exe)
            continue
        if k32.TerminateProcess(h, 1):
            handles.append((h, exe))
        else:
            err = ct.get_last_error()
            k32.CloseHandle(h)
            (res["denied"] if err == 5 else res["failed"]).append(exe)
    end = time.time() + max(0.0, wait)
    for h, exe in handles:                       # 실제로 꺼질 때까지 잠깐 기다림
        left = max(0, int((end - time.time()) * 1000))
        k32.WaitForSingleObject(h, left)
        k32.CloseHandle(h)
        res["killed"].append(exe)
    if res["failed"]:                            # 알 수 없는 이유로 실패한 것만 taskkill 로 한 번 더
        retry = sorted(set(res["failed"]))
        res["failed"] = []
        _kill_taskkill(retry, res)
    for k in res:
        res[k] = sorted(set(res[k]), key=str.lower)
    return res


# ---------------------------------------------------------------- 예비: 명령어 방식
def _pids_tasklist(want):
    import csv
    try:
        out = subprocess.run(["tasklist", "/FO", "CSV", "/NH"], capture_output=True, text=True, errors="ignore",
                             timeout=15, creationflags=NO_WINDOW).stdout
    except Exception:
        return []
    return [int(r[1]) for r in csv.reader(out.splitlines())
            if len(r) >= 2 and r[0].lower() in want and r[1].isdigit()]


def _kill_taskkill(names, res):
    args = ["taskkill", "/F"]
    for n in names:
        args += ["/IM", n]
    try:
        r = subprocess.run(args, capture_output=True, text=True, errors="ignore", timeout=20,
                           creationflags=NO_WINDOW)
        out = (r.stdout or "") + (r.stderr or "")
        if r.returncode == 0:
            res["killed"] += names
        elif "denied" in out.lower() or "거부" in out:
            res["denied"] += names
    except Exception:
        res["failed"] += names
    return res
