# -*- coding: utf-8 -*-
"""
현재 상태 창 — 매크로가 켜져 있는 동안 로블록스 화면 오른쪽 가운데 (스크립트 매크로의 상태 UI 와 같은 모양)
  · 폭 560 · 오른쪽에서 20 · 세로 가운데 · 배경 (18, 18, 22) 을 왼쪽은 투명 → 오른쪽은 85% 로 (가로 그라디언트) · 각진 모서리
  · 줄은 전부 오른쪽 정렬 "라벨 값" (라벨 회색 #9AA0AC · 값 흰색 · 현재 행동은 하늘색 #7FD1FF · 크게)
  · 글자 크기: 기본 24 · 현재 행동 30 · 묶음 22 · 켜져 있는 기능 16 · 묶음 사이 흰색 10% 구분선
  · 켜질 때 · 꺼질 때 0.3초 동안 서서히
  · 클릭이 통과되고 포커스를 안 가져감 · 화면 캡처에는 안 찍힘 (매크로가 화면을 읽을 때 방해 안 되게)
윈도우: 글자 모양을 GDI 로 그려서 numpy 로 합친 뒤 UpdateLayeredWindow (픽셀마다 투명도) · 실패하면 macro.show_status_simple
앱이 파일에 {"on": true, "rows": [[종류, 라벨, 값, 색], …]} 을 씀 — 종류는 STYLE 참고 · on 이 false 면 서서히 사라지고 끝남
"""
import ctypes
import json
import os
import sys
import time

import numpy as np

W_BASE = 560
PAD_X, PAD_Y, GAP = 20, 16, 4
BG = (18, 18, 22)
BG_ALPHA = 0.85
LABEL, VALUE, ACCENT = "#9AA0AC", "#FFFFFF", "#7FD1FF"
# 종류 → (글자 크기, 위쪽 여백 더, 값 색)
STYLE = {"line": (24, 0, VALUE), "act": (30, 0, ACCENT), "head": (22, 6, VALUE), "sub": (22, 0, VALUE),
         "shead": (16, 6, VALUE), "small": (16, 0, VALUE), "line6": (24, 6, VALUE)}
FADE = 0.3


def _rgb(h):
    h = str(h or "").lstrip("#")
    try:
        return int(h[0:2], 16), int(h[2:4], 16), int(h[4:6], 16)
    except (ValueError, IndexError):
        return 255, 255, 255


class Renderer:
    """줄들 → 픽셀마다 투명도가 있는 그림 (BGRA · 미리 곱한 알파) · mask(글자, 크기px) → 0~255 글자 모양 (높이 × 폭)"""

    def __init__(self, mask):
        self.mask = mask
        self.cache = {}

    def _m(self, text, px):
        k = (text, px)
        if k not in self.cache:
            if len(self.cache) > 400:
                self.cache.clear()
            self.cache[k] = self.mask(text, px)
        return self.cache[k]

    def _wrap(self, segs, px, width):
        """[(글자, 색)] → 폭에 맞게 나눈 줄들 (낱말 단위 · 오른쪽 정렬이라 앞에서부터 채움)"""
        words = []
        for text, col in segs:
            parts = str(text).split(" ")
            for i, p in enumerate(parts):
                if p:
                    words.append((p, col))
        lines, cur, cur_w = [], [], 0
        space = self._m(" ", px).shape[1] if self._m(" ", px).size else px // 4
        for w, col in words:
            ww = self._m(w, px).shape[1]
            add = ww + (space if cur else 0)
            if cur and cur_w + add > width:
                lines.append(cur)
                cur, cur_w = [], 0
                add = ww
            cur.append((w, col))
            cur_w += add
        if cur:
            lines.append(cur)
        return lines or [[]], space

    def render(self, rows, scale=1.0):
        s = max(0.5, min(2.0, float(scale)))
        W = int(W_BASE * s)
        inner = W - 2 * int(PAD_X * s)
        items = []                               # ("div", 높이) · ("text", 위 여백, 높이, [(마스크, 색, x)])
        for r in rows:
            kind = r[0]
            if kind == "div":
                items.append(("div", max(1, int(round(s)))))
                continue
            size, extra, vcol = STYLE.get(kind, STYLE["line"])
            px = max(8, int(round(size * s)))
            label, value = str(r[1] or ""), str(r[2] if len(r) > 2 and r[2] is not None else "")
            col = r[3] if len(r) > 3 and r[3] else vcol
            segs = ([(label, LABEL)] if label else []) + ([(value, col)] if value else [])
            lines, space = self._wrap(segs, px, inner)
            for n, line in enumerate(lines):
                parts, x = [], 0
                h = max([self._m(w, px).shape[0] for w, _ in line] or [px])
                for i, (w, c) in enumerate(line):
                    m = self._m(w, px)
                    parts.append((m, c, x))
                    x += m.shape[1] + (space if i < len(line) - 1 else 0)
                items.append(("text", int(extra * s) if n == 0 else 0, h, parts, x))
        gap = int(GAP * s)
        H = 2 * int(PAD_Y * s)
        for i, it in enumerate(items):
            H += (it[1] if it[0] == "div" else it[1] + it[2]) + (gap if i else 0)
        H = max(H, 2 * int(PAD_Y * s) + 1)
        # 배경: 왼쪽 투명 → 오른쪽 85% (스크립트의 UIGradient)
        a = (np.linspace(0.0, 1.0, W) * BG_ALPHA)[None, :].repeat(H, 0)
        rgb = np.zeros((H, W, 3), np.float32)
        rgb[:] = BG
        y = int(PAD_Y * s)
        x0, x1 = int(PAD_X * s), W - int(PAD_X * s)
        for i, it in enumerate(items):
            if i:
                y += gap
            if it[0] == "div":
                hh = it[1]
                sl = (slice(y, y + hh), slice(x0, x1))
                k = 0.1
                rgb[sl] = rgb[sl] * (a[sl][..., None] * (1 - k)) / np.maximum(a[sl] * (1 - k) + k, 1e-6)[..., None] \
                    + 255 * k / np.maximum(a[sl] * (1 - k) + k, 1e-6)[..., None]
                a[sl] = a[sl] * (1 - k) + k
                y += hh
                continue
            _, extra, hh, parts, tw = it
            y += extra
            left = x1 - tw                              # 오른쪽 정렬
            for m, c, x in parts:
                mh, mw = m.shape
                yy, xx = y + (hh - mh), left + x
                if mw == 0 or xx >= W or yy >= H:
                    continue
                cov = m[: H - yy, : W - xx].astype(np.float32) / 255.0
                sl = (slice(yy, yy + cov.shape[0]), slice(xx, xx + cov.shape[1]))
                col = np.array(_rgb(c), np.float32)
                old_a = a[sl]
                new_a = cov + old_a * (1 - cov)
                rgb[sl] = (col * cov[..., None] + rgb[sl] * (old_a * (1 - cov))[..., None]) / np.maximum(new_a, 1e-6)[..., None]
                a[sl] = new_a
            y += hh
        out = np.zeros((H, W, 4), np.uint8)
        pa = a[..., None]
        out[..., :3] = np.clip(rgb[..., ::-1] * pa, 0, 255).astype(np.uint8)     # BGR · 미리 곱한 알파
        out[..., 3] = np.clip(a * 255, 0, 255).astype(np.uint8)
        return out


# ---------------------------------------------------------------- 윈도우 (GDI · 겹친 창)
def _win():
    from ctypes import wintypes as W
    u, g = ctypes.WinDLL("user32", use_last_error=True), ctypes.WinDLL("gdi32")
    vp = ctypes.c_void_p
    u.GetDC.argtypes, u.GetDC.restype = [vp], vp
    u.ReleaseDC.argtypes = [vp, vp]
    g.CreateCompatibleDC.argtypes, g.CreateCompatibleDC.restype = [vp], vp
    g.DeleteDC.argtypes = [vp]
    g.SelectObject.argtypes, g.SelectObject.restype = [vp, vp], vp
    g.DeleteObject.argtypes = [vp]
    g.CreateFontW.argtypes = [ctypes.c_int] * 5 + [W.DWORD] * 8 + [W.LPCWSTR]
    g.CreateFontW.restype = vp
    g.CreateDIBSection.argtypes = [vp, vp, W.UINT, ctypes.POINTER(vp), vp, W.DWORD]
    g.CreateDIBSection.restype = vp
    g.SetTextColor.argtypes, g.SetBkMode.argtypes = [vp, W.DWORD], [vp, ctypes.c_int]
    g.TextOutW.argtypes = [vp, ctypes.c_int, ctypes.c_int, W.LPCWSTR, ctypes.c_int]
    g.GetTextExtentPoint32W.argtypes = [vp, W.LPCWSTR, ctypes.c_int, ctypes.POINTER(W.SIZE)]
    u.UpdateLayeredWindow.argtypes = [vp, vp, ctypes.POINTER(W.POINT), ctypes.POINTER(W.SIZE), vp,
                                      ctypes.POINTER(W.POINT), W.DWORD, vp, W.DWORD]
    u.GetWindowLongW.argtypes, u.GetWindowLongW.restype = [vp, ctypes.c_int], ctypes.c_long
    u.SetWindowLongW.argtypes = [vp, ctypes.c_int, ctypes.c_long]
    u.SetWindowDisplayAffinity.argtypes = [vp, W.DWORD]
    u.GetAncestor.argtypes, u.GetAncestor.restype = [vp, W.UINT], vp
    u.SetWindowPos.argtypes = [vp, vp, ctypes.c_int, ctypes.c_int, ctypes.c_int, ctypes.c_int, W.UINT]
    return u, g, W


class _BMI(ctypes.Structure):
    _fields_ = [("biSize", ctypes.c_uint32), ("biWidth", ctypes.c_int32), ("biHeight", ctypes.c_int32),
                ("biPlanes", ctypes.c_uint16), ("biBitCount", ctypes.c_uint16), ("biCompression", ctypes.c_uint32),
                ("biSizeImage", ctypes.c_uint32), ("biXPelsPerMeter", ctypes.c_int32), ("biYPelsPerMeter", ctypes.c_int32),
                ("biClrUsed", ctypes.c_uint32), ("biClrImportant", ctypes.c_uint32), ("pad", ctypes.c_uint32 * 4)]


class _Blend(ctypes.Structure):
    _fields_ = [("BlendOp", ctypes.c_ubyte), ("BlendFlags", ctypes.c_ubyte), ("SourceConstantAlpha", ctypes.c_ubyte),
                ("AlphaFormat", ctypes.c_ubyte)]


def _bmi(w, h):
    b = _BMI()
    b.biSize, b.biWidth, b.biHeight, b.biPlanes, b.biBitCount = 40, w, -h, 1, 32
    return b


def gdi_mask_factory(font="Malgun Gothic"):
    u, g, W = _win()
    dc = g.CreateCompatibleDC(None)
    fonts = {}

    def mask(text, px):
        if px not in fonts:
            # 높이 -px = 글자 크기 px · ANTIALIASED_QUALITY (회색 테두리 · ClearType 아님)
            fonts[px] = g.CreateFontW(-px, 0, 0, 0, 400, 0, 0, 0, 1, 0, 0, 4, 0, font)
        old = g.SelectObject(dc, fonts[px])
        sz = W.SIZE()
        g.GetTextExtentPoint32W(dc, text, len(text), ctypes.byref(sz))
        w, h = max(1, sz.cx), max(1, sz.cy)
        bits = ctypes.c_void_p()
        bmi = _bmi(w, h)
        bmp = g.CreateDIBSection(dc, ctypes.byref(bmi), 0, ctypes.byref(bits), None, 0)
        ob = g.SelectObject(dc, bmp)
        ctypes.memset(bits, 0, w * h * 4)
        g.SetBkMode(dc, 1)
        g.SetTextColor(dc, 0xFFFFFF)
        g.TextOutW(dc, 0, 0, text, len(text))
        arr = np.frombuffer((ctypes.c_ubyte * (w * h * 4)).from_address(bits.value), np.uint8).reshape(h, w, 4)
        m = arr[..., 1].copy()
        g.SelectObject(dc, ob)
        g.DeleteObject(bmp)
        g.SelectObject(dc, old)
        return m if text.strip() else np.zeros((h, w), np.uint8)
    return mask


def run(path, parent=None, lang="ko", font="Malgun Gothic", rect_fn=None, alive_fn=None):
    """윈도우에서 상태 창 띄우기 (tk 창을 겹친 창으로 바꿔서 그림만 UpdateLayeredWindow 로 넣음)"""
    import tkinter as tk
    u, g, W = _win()
    labels = _labels(lang)
    rnd = Renderer(gdi_mask_factory(font))
    root = tk.Tk()
    root.overrideredirect(True)
    root.attributes("-topmost", True)
    root.geometry("1x1+-32000+-32000")
    import macro
    # 겹친 창 · 클릭 통과 · 포커스 안 가져감 · 작업 표시줄에 안 보임 · 맨 위 · 화면 캡처에서 뺌 — 스타일을 먼저 바꾸고 띄움 (로블록스 포커스를 안 뺏게)
    hwnd = macro.quiet_window(root, extra=0x8, capture=False)
    hwnd = hwnd or u.GetAncestor(root.winfo_id(), 2) or root.winfo_id()
    screen = u.GetDC(None)
    mem = g.CreateCompatibleDC(screen)
    st = {"mtime": None, "rows": None, "img": None, "bmp": None, "bits": None, "size": (0, 0), "on": True,
          "alpha": 0.0, "t": time.time(), "n": 0, "pos": None, "scale": 1.0}

    def upload(img):
        h, w = img.shape[:2]
        if st["size"] != (w, h):
            if st["bmp"]:
                g.SelectObject(mem, st["old"])
                g.DeleteObject(st["bmp"])
            bits = ctypes.c_void_p()
            bmi = _bmi(w, h)
            st["bmp"] = g.CreateDIBSection(mem, ctypes.byref(bmi), 0, ctypes.byref(bits), None, 0)
            st["old"] = g.SelectObject(mem, st["bmp"])
            st["bits"], st["size"] = bits, (w, h)
        ctypes.memmove(st["bits"], img.ctypes.data, w * h * 4)

    def show():
        w, h = st["size"]
        if not w:
            return
        rect = rect_fn() if rect_fn else None
        if rect and rect[2] > 200:
            x, y = rect[0] + rect[2] - w - int(20 * st["scale"]), rect[1] + (rect[3] - h) // 2
        else:
            x, y = root.winfo_screenwidth() - w - 20, (root.winfo_screenheight() - h) // 2
        bl = _Blend(0, 0, int(255 * max(0.0, min(1.0, st["alpha"]))), 1)
        u.UpdateLayeredWindow(hwnd, screen, ctypes.byref(W.POINT(x, y)), ctypes.byref(W.SIZE(w, h)), mem,
                              ctypes.byref(W.POINT(0, 0)), 0, ctypes.byref(bl), 2)
        if st["n"] % 40 == 0:                               # 다른 창이 맨 위로 올라와도 다시 위로
            u.SetWindowPos(hwnd, ctypes.c_void_p(-1), 0, 0, 0, 0, 0x1 | 0x2 | 0x10)

    def tick():
        st["n"] += 1
        try:
            m = os.path.getmtime(path)
            if m != st["mtime"]:
                st["mtime"] = m
                with open(path, encoding="utf-8") as f:
                    d = json.load(f)
                st["on"] = bool(d.get("on", True))
                if "rows" not in d:
                    raise ValueError
                rows = [[r[0], labels.get(r[1], r[1]), _value(r[2], lang)] + list(r[3:4])
                        for r in (d.get("rows") or []) if isinstance(r, list) and len(r) >= 3]
                rect = rect_fn() if rect_fn else None
                scale = max(0.6, min(1.5, rect[3] / 1080)) if rect and rect[3] > 200 else 1.0
                if rows != st["rows"] or scale != st["scale"]:
                    st["rows"], st["scale"] = rows, scale
                    upload(rnd.render(rows, scale))
        except (OSError, ValueError):
            pass
        dt, st["t"] = time.time() - st["t"], time.time()
        st["alpha"] += (dt / FADE) * (1 if st["on"] else -1)
        st["alpha"] = max(0.0, min(1.0, st["alpha"]))
        show()
        if (not st["on"] and st["alpha"] <= 0) or (parent and st["n"] % 40 == 0 and alive_fn and not alive_fn(parent)):
            root.destroy()
            return
        root.after(33 if 0 < st["alpha"] < 1 else 250, tick)

    tick()
    root.mainloop()


# ---------------------------------------------------------------- 글자 (라벨 · 정해진 값)
LABELS = {
    "feat": ("현재 작동중인 기능:", "Running:", "動作中の機能:"),
    "act": ("현재 행동:", "Current action:", "現在の行動:"),
    "queue": ("대기열인 기능:", "Queued:", "待機中の機能:"),
    "cool": ("아이템 사용 쿨타임", "Item cooldowns", "アイテム使用クールタイム"),
    "strange": ("Strange Controller:", "Strange Controller:", "Strange Controller:"),
    "random": ("Biome Randomizer:", "Biome Randomizer:", "Biome Randomizer:"),
    "biome": ("인식중인 바이옴:", "Detected biome:", "認識中のバイオーム:"),
    "on": ("켜져 있는 기능:", "Enabled features:", "オンの機能:"),
    "g_web": ("[웹후크 기능]", "[Webhook]", "[Webhook]"),
    "g_always": ("[상시 기능]", "[Always]", "[常時]"),
    "g_single": ("[단일성 기능]", "[One-shot]", "[単発]"),
    "g_urgent": ("[긴급 기능]", "[Urgent]", "[緊急]"),
    "time": ("작동 시간:", "Uptime:", "稼働時間:"),
}
VALUES = {
    "자동 낚시": ("Auto fishing", "自動釣り"), "레어 바이옴 자동 팝핑": ("Rare biome popping", "レアバイオーム自動ポッピング"),
    "블러드 레인 자동 생성": ("Auto Blood Rain", "ブラッドレイン自動生成"), "아이템 사용": ("Item use", "アイテム使用"),
    "상인 자동 구매": ("Merchant auto buy", "商人の自動購入"), "포션 자동 제작": ("Auto potion crafting", "ポーション自動作成"),
    "메모리 매치": ("Memory match", "メモリーマッチ"), "바이옴 매크로": ("Biome macro", "バイオームマクロ"),
    "이동": ("Moving", "移動"), "없음": ("None", "なし"), "대기": ("Idle", "待機"), "작동 중": ("Running", "動作中"),
    "이동 중": ("Moving", "移動中"), "켜진 기능 없음": ("No features on", "オンの機能なし"),
    "다른 서버 접속 중이라 대기": ("Waiting (in another server)", "他のサーバーに接続中のため待機"),
    "준비됨": ("Ready", "準備完了"), "꺼짐": ("Off", "オフ"), "확인 중": ("Checking", "確認中"),
}


def _labels(lang):
    i = {"en": 1, "ja": 2}.get(lang, 0)
    return {k: v[i] for k, v in LABELS.items()}


def _value(v, lang):
    v = str(v if v is not None else "")
    if lang not in ("en", "ja"):
        return v
    i = 0 if lang == "en" else 1
    import re
    v = re.sub(r"^(\d+)분 (\d+)초$", (lambda m: f"{m[1]}m {m[2]}s") if lang == "en" else (lambda m: f"{m[1]}分{m[2]}秒"), v)
    return ", ".join(" · ".join(VALUES[p][i] if p in VALUES else p for p in part.split(" · ")) for part in v.split(", "))


if __name__ == "__main__" and "--preview" in sys.argv:
    # 미리 보기 (개발용): python overlay.py --preview rows.json out.png 글꼴.ttf
    from PIL import Image, ImageDraw, ImageFont
    _rows, _out, _font = sys.argv[sys.argv.index("--preview") + 1:][:3]

    def _pil_mask(text, px):
        f = ImageFont.truetype(_font, px)
        asc, desc = f.getmetrics()
        w = max(1, int(f.getlength(text)))
        im = Image.new("L", (w, asc + desc), 0)
        ImageDraw.Draw(im).text((0, 0), text, fill=255, font=f)
        return np.array(im)
    with open(_rows, encoding="utf-8") as fh:
        _d = json.load(fh)
    lab = _labels("ko")
    img = Renderer(_pil_mask).render([[r[0], lab.get(r[1], r[1]), r[2]] + list(r[3:4]) for r in _d["rows"]], 1.0)
    rgb = img[..., 2::-1].astype(np.float32)
    bg = np.zeros_like(rgb)
    bg[:] = (70, 110, 60)                                 # 게임 화면 대신 초록 바탕
    a = img[..., 3:4].astype(np.float32) / 255
    Image.fromarray(np.clip(rgb + bg * (1 - a), 0, 255).astype(np.uint8)).save(_out)
