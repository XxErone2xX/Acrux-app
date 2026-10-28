# -*- coding: utf-8 -*-
"""
RapidOCR 일꾼 프로세스 — 앱(app.py)이 글자를 읽어야 할 때만 띄우고, 한동안 안 쓰면 스스로 꺼짐
  · OCR 라이브러리(onnxruntime · OpenCV · 모델)는 이 프로세스에만 올라감 → 앱은 가볍게 유지
    (전체 화면을 한 번 읽으면 메모리가 수백 MB 로 늘고 줄지 않음 → 일꾼째로 끄면 전부 돌려받음)
  · CPU 코어를 다 쓰지 않게 스레드 수를 줄이고 우선순위를 낮춤 (사양 낮은 PC 에서 게임이 버벅이지 않게)

주고받는 방식 (표준 입출력 · 이진)
  요청: [4바이트 길이][JSON {"op": "text" | "count" | "boxes", "w": 너비, "h": 높이}][BGRA 픽셀 w*h*4 바이트]
  응답: [4바이트 길이][JSON {"result": ...} 또는 {"error": "..."}]
  시작하면 먼저 {"ready": true} (또는 {"error": ...}) 를 보냄
"""
import json
import os
import re
import struct
import sys
import threading
import time

IDLE_EXIT = 45.0          # 이 시간 동안 요청이 없으면 꺼짐 (다음에 필요하면 앱이 다시 띄움)
MEM_EXIT_MB = 260         # 한 번 읽고 나서 메모리가 이만큼 넘으면 대답한 뒤 꺼짐 (큰 화면을 읽고 늘어난 메모리를 돌려받음)
_last = [time.time()]
_busy = [False]


def _threads():
    n = os.cpu_count() or 2
    return max(1, min(4, n // 2))


def _load_engine():
    # rapidocr 가 만드는 onnxruntime 세션 설정에 스레드 수 제한을 미리 넣어 둠 (버전에 상관없이)
    import onnxruntime as ort
    orig = ort.SessionOptions
    nt = _threads()

    def opts():
        o = orig()
        o.intra_op_num_threads = nt
        o.inter_op_num_threads = 1
        try:
            o.enable_cpu_mem_arena = False      # 읽을 때마다 늘어난 메모리를 붙잡아 두지 않게
        except Exception:
            pass
        return o
    ort.SessionOptions = opts
    from rapidocr_onnxruntime import RapidOCR
    return RapidOCR()


# ---------------------------------------------------------------- 읽기 (예전 macro.py 의 RapidOCR 부분)
def read_text(eng, data, w, h):
    import numpy as np
    img = np.frombuffer(data, dtype=np.uint8).reshape(h, w, 4)[:, :, :3].copy()   # BGRA → BGR
    if h < 48:                               # 작은 글씨는 키워서 읽기
        import cv2
        k = 48.0 / max(1, h)
        img = cv2.resize(img, (int(w * k), 48), interpolation=cv2.INTER_CUBIC)
    # use_cls=False — 게임 글자는 안 뒤집혀 있는데, 방향 판별이 짧은 글자(Open)를 거꾸로 보고 "uado" 로 읽음
    res, _ = eng(img, use_cls=False)
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


def read_count(eng, data, w, h):
    """아이템 칸 오른쪽 아래의 작은 개수 글자(x23)만 따로 크게 읽기
    (흰 글자만 남기고 3배 확대 → 안 되면 2배 확대 원본)"""
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
        res, _ = eng(im, use_cls=False)
        for r in res or []:
            m = re.search(r"[x×X]\s*([0-9][0-9,\.]*)", r[1])
            if m:
                return "x" + m.group(1)
    return None


def read_boxes(eng, data, w, h):
    """글자 덩어리 → [[글자, x1, y1, x2, y2]] (받은 이미지 픽셀 기준)"""
    import numpy as np
    img = np.frombuffer(data, dtype=np.uint8).reshape(h, w, 4)[:, :, :3].copy()
    res, _ = eng(img, use_cls=False)
    return [[str(t), float(min(p[0] for p in b)), float(min(p[1] for p in b)),
             float(max(p[0] for p in b)), float(max(p[1] for p in b))] for b, t, _s in (res or [])]


OPS = {"text": read_text, "count": read_count, "boxes": read_boxes}


# ---------------------------------------------------------------- 주고받기
def _read_exact(f, n):
    buf = b""
    while len(buf) < n:
        chunk = f.read(n - len(buf))
        if not chunk:
            return None
        buf += chunk
    return buf


def _send(out, obj):
    body = json.dumps(obj, ensure_ascii=False).encode("utf-8")
    out.write(struct.pack("<I", len(body)) + body)
    out.flush()


def _mem_mb():
    try:
        if sys.platform == "win32":
            import ctypes
            from ctypes import wintypes

            class PMC(ctypes.Structure):
                _fields_ = [("cb", wintypes.DWORD), ("PageFaultCount", wintypes.DWORD),
                            ("PeakWorkingSetSize", ctypes.c_size_t), ("WorkingSetSize", ctypes.c_size_t),
                            ("QuotaPeakPagedPoolUsage", ctypes.c_size_t), ("QuotaPagedPoolUsage", ctypes.c_size_t),
                            ("QuotaPeakNonPagedPoolUsage", ctypes.c_size_t), ("QuotaNonPagedPoolUsage", ctypes.c_size_t),
                            ("PagefileUsage", ctypes.c_size_t), ("PeakPagefileUsage", ctypes.c_size_t)]
            c = PMC()
            c.cb = ctypes.sizeof(c)
            psapi = ctypes.WinDLL("psapi")
            if psapi.GetProcessMemoryInfo(ctypes.windll.kernel32.GetCurrentProcess(), ctypes.byref(c), c.cb):
                return c.WorkingSetSize / 1048576
            return 0
        for ln in open("/proc/self/status"):
            if ln.startswith("VmRSS"):
                return int(ln.split()[1]) / 1024
    except Exception:
        pass
    return 0


def _idle_watch():
    while True:
        time.sleep(2.0)
        if not _busy[0] and time.time() - _last[0] > IDLE_EXIT:
            os._exit(0)


def main():
    inp = sys.stdin.buffer
    out = os.fdopen(os.dup(sys.stdout.fileno()), "wb")
    try:                                       # 라이브러리가 화면에 찍는 글자가 주고받는 통로에 섞이지 않게
        os.dup2(sys.stderr.fileno(), sys.stdout.fileno())
    except Exception:
        pass
    sys.stdout = sys.stderr
    try:
        eng = _load_engine()
    except Exception as e:
        _send(out, {"error": str(e) or type(e).__name__})
        return
    _send(out, {"ready": True, "threads": _threads()})
    threading.Thread(target=_idle_watch, daemon=True).start()
    while True:
        raw = _read_exact(inp, 4)
        if raw is None:
            return                             # 앱이 꺼짐
        head = json.loads(_read_exact(inp, struct.unpack("<I", raw)[0]) or b"{}")
        w, h = int(head.get("w", 0)), int(head.get("h", 0))
        data = _read_exact(inp, w * h * 4) if w > 0 and h > 0 else b""
        if data is None:
            return
        _busy[0], _last[0] = True, time.time()
        try:
            fn = OPS.get(head.get("op"))
            if not fn:
                raise ValueError(f"모르는 요청: {head.get('op')}")
            _send(out, {"result": fn(eng, data, w, h)})
        except Exception as e:
            _send(out, {"error": str(e) or type(e).__name__})
        finally:
            _busy[0], _last[0] = False, time.time()
        if _mem_mb() > MEM_EXIT_MB:
            return                             # 큰 화면을 읽고 메모리가 늘었음 → 꺼져서 돌려줌 (다음 요청 때 다시 뜸)


if __name__ == "__main__":
    main()
