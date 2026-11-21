# -*- coding: utf-8 -*-
"""
서버 체크 (디스코드 봇 /서버 체크) — 지금 들어가 있는 서버가 내 브섭인지
1. 왼쪽 메뉴 버튼(줄 세 개)을 한 번 눌러 Menu 창을 엶
2. Menu 창 아래에 Private Server Commands 버튼이 있으면 내 브섭 (브섭 주인에게만 보이는 버튼)
3. Menu 창을 닫음 (X · 안 닫히면 메뉴 버튼을 한 번 더)
"""
import sell

MENU_POS = [round(33 / 1920, 4), round(673 / 1080, 4)]   # 기본 메뉴 버튼 위치 (1920×1080 전체 화면 기준) · 공통 위치에서 지정하면 그 위치
# Menu 창 버튼 글자 (sell._norm 뒤: a → o · i → l) → 1080 높이 전체 화면에서 글자 가운데 y
MENU_KEYS = {"seoson": 441, "ochlev": 494, "flshlng": 539, "settlng": 586, "robux": 633, "develop": 679}
TITLE_Y = 389                 # Menu 제목 가운데 y (같은 기준)
CLOSE_DX = 163                # 창 가운데 → X 의 x 거리 (같은 기준)


def _keys(b):
    n = sell._norm(b[0])
    return [k for k in MENU_KEYS if k in n]


def is_psc(text):
    """Private Server Commands 버튼 글자 (게임 글꼴이라 'Privote Server Commonds' 처럼 읽혀도)"""
    n = sell._norm(text)
    return "prlv" in n and ("serv" in n or "comm" in n)


def measure(boxes, rect):
    """OCR 덩어리 [(글자, x, y, w, h) 창 비율] → {"menu": Menu 창이 보임, "own": 내 브섭, "close": X 위치 (창 비율) 또는 None}"""
    items = [b for b in boxes if _keys(b)]
    found = {k for b in items for k in _keys(b)}
    if len(found) < 2:
        return {"menu": False, "own": False, "close": None}
    W, H = rect[2], rect[3]
    bottom = max(b[2] for b in items)
    ys = sorted(((b[2] * H, MENU_KEYS[_keys(b)[0]]) for b in items), key=lambda v: v[1])
    (y1, r1), (y2, r2) = ys[0], ys[-1]
    title = next((b for b in boxes if sell._norm(b[0]) in ("menu", "menux") and b[2] * H < y1), None)
    # Menu 창은 화면 가운데 (버튼 글자 덩어리는 왼쪽 아이콘이 같이 읽혀 가운데가 밀리기도 해서 제목 · 화면 가운데를 씀)
    cx = title[1] if title else 0.5
    # 버튼은 Menu 창 바로 아래 · 가운데 (월드의 글자 · 게시판에 걸리지 않게)
    own = any(is_psc(b[0]) and b[2] > bottom and abs(b[1] - cx) * W < 0.25 * H for b in boxes)
    # 크기: 읽힌 버튼 중 가장 위 · 아래 사이 거리 (UI 크기가 창 크기와 다를 때) · X = 창 가운데 + 163 · 제목 높이
    k = (y2 - y1) / (r2 - r1) if r2 > r1 and y2 > y1 else H / 1080
    ty = title[2] * H if title else y1 - (r1 - TITLE_Y) * k
    x, y = cx * W + CLOSE_DX * k, ty - k
    return {"menu": True, "own": own, "close": [round(min(1.0, max(0.0, x / W)), 4), round(min(1.0, max(0.0, y / H)), 4)]}


def check(ocr, click, wait, rect, menu_pos=None, log=None):
    """메뉴를 열어 내 브섭인지 확인하고 닫음 → True | False | None (메뉴 창을 못 찾음)
    ocr() → 창 전체 OCR 덩어리 · click([x, y] 창 비율) · wait(초) · rect() → 로블록스 창 (x, y, w, h)"""
    pos = menu_pos or MENU_POS
    res = None
    for attempt in range(2):            # 처음부터 메뉴가 열려 있었으면 첫 클릭에 닫힘 → 한 번 더
        click(pos)
        wait(0.9)
        res = measure(ocr(), rect())
        if not res["menu"]:
            wait(0.6)                   # 렉으로 늦게 뜸
            res = measure(ocr(), rect())
        if res["menu"]:
            break
    if not res["menu"]:
        return None
    own = res["own"]
    if not own:                         # 버튼이 늦게 그려졌을 수도 있으니 한 번 더 봄
        wait(0.5)
        again = measure(ocr(), rect())
        own = again["own"]
        res = again if again["menu"] else res
    # 닫기: X → 그래도 열려 있으면 메뉴 버튼 한 번 더
    for target in ([res["close"]] if res["close"] else []) + [pos]:
        click(target)
        wait(0.6)
        if not measure(ocr(), rect())["menu"]:
            break
    else:
        if log:
            log("서버 체크 — Menu 창이 안 닫힘 (직접 닫아 주세요)", "y")
    return own
