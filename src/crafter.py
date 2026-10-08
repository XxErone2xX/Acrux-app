# -*- coding: utf-8 -*-
"""
포션 자동 제작 — 오른쪽 알림에 "Auto Crafted" (Your ○○○ was automatically crafted!) 가 뜨면
  그 포션 이름을 읽고 → 포션 제작 장소(Stella)로 이동 → F → 검색창에 이름 입력 → 첫 결과 → Open Recipe
  → Add Ingredients 창에서 Add Everything → Craft → Add Everything (다음 자동 제작용으로 재료를 다시 채움) → 그 자리에서 리셋
  화면은 전부 글자(OCR)로 찾음: 상점 제목 'Stella's Workshop' · 탭 'Item' 'Lantern' 으로 위치 · 크기를 잼
  (1920x1080 전체 화면 스크린샷 기준 거리)
"""
import difflib
import re
import time

import macro
import popping
import sell

# 게임 글꼴은 OCR 이 a → o 로 읽어서 ("Heovenly") 이름을 그대로 검색하면 안 나옴 → 아는 낱말로 고침
WORDS = ("Potion", "Fortune", "Haste", "Heavenly", "Godlike", "Godly", "Warp", "Zeus", "Poseidon", "Hades", "Jewelry",
         "Zombie", "Rage", "Diver", "Bound", "Oblivion", "Transcendent", "Lucky", "Speed", "Mixed", "Super", "Tidal",
         "Shifter", "Popping", "Stella", "Candle", "Celestial", "Exotic", "Powered", "Quartz", "Strange", "Random",
         "Sack", "Divine", "Aurora", "Of", "The", "I", "II", "III", "IV", "V", "X")
_CRAFTED = re.compile(r"y[o0]ur\s*([^\n]+?)\s*w[ao]s\s*[ao]ut[ao]m[ao]t[il1][ck][ao][il1]{1,2}y\s*cr[ao]fted", re.I)
ROMAN = ("I", "II", "III", "IV", "V")

# Stella's Workshop (1080p · 제목 'Stella's Workshop' 가운데 기준 px) — 크기는 Item ↔ Lantern 탭 가로 거리(239px)로
SHOP_REF_TABS = 239
SEARCH_OFF = (7, 83)                 # 검색창
SHOP_CLOSE_OFF = (217, -2)           # 오른쪽 위 X
LIST_OFF = (-225, 110, 225, 735)     # 검색 결과 목록 (카드 제목들)
# Add Ingredients 창 (제목 가운데 기준) — 크기는 제목 ↔ Add Everything 세로 거리(295px)로
ADD_REF_GAP = 295
CRAFT_FROM_ADD = 240                 # Add Everything → Craft 가로 거리
ADD_CLOSE_OFF = (226, -2)            # Add Ingredients 오른쪽 위 X


def crafted_name(text):
    """알림 OCR 글자 → 자동 제작된 포션 이름 (예: 'Fortune Potion I') 또는 None"""
    t = "\n".join(" ".join(ln.split()) for ln in str(text or "").splitlines())    # 줄은 그대로 (위의 'Potion Auto Added' 알림과 섞이지 않게)
    m = _CRAFTED.search(t)
    if not m:
        return None
    name = m.group(1).strip(" .!")
    if " " not in name:                                  # 띄어쓰기를 못 읽음 → 대문자 앞에서 끊기
        name = re.sub(r"(?<=[a-z])(?=[A-Z])", " ", name)
    return fix_name(name) or None


def fix_name(name):
    """OCR 이름의 낱말을 아는 낱말로 (a ↔ o · i ↔ l 이 헷갈려도 같은 낱말이면) · 로마 숫자 l → I"""
    out = []
    for w in name.split():
        n = sell._norm(w)
        if n and all(ch == "l" for ch in n) and len(n) <= 3:     # 'l' · 'Il' → 로마 숫자
            out.append("I" * len(n))
            continue
        hit = next((k for k in WORDS if sell._norm(k) == n), None)
        out.append(hit or w)
    return " ".join(out)


def safe_query(name):
    """OCR 이 헷갈리는 글자(a o i l)가 없는 가장 긴 조각 — 고친 이름으로 검색이 안 될 때 씀"""
    parts = re.split(r"[aoilAOIL]", name)
    best = max(parts, key=len).strip()
    return best if len(best) >= 3 else name


def split_roman(name):
    """'Fortune Potion II' → ('Fortune Potion', 2) · 숫자가 없으면 (name, 0)"""
    w = name.split()
    if len(w) > 1 and w[-1] in ROMAN:
        return " ".join(w[:-1]), ROMAN.index(w[-1]) + 1
    return name, 0


def same_name(a, b):
    na, nb = sell._norm(a), sell._norm(b)
    return bool(na) and (na == nb or (abs(len(na) - len(nb)) <= 1
                                      and difflib.SequenceMatcher(None, na, nb).ratio() >= 0.9))


def pick_card(boxes, name):
    """검색 결과 글자 덩어리 → 누를 카드 제목 덩어리
    로마 숫자(I · II · III)는 OCR 이 l · i · m 처럼 읽어서 구별이 안 됨 → 같은 이름의 카드 중 위에서 n 번째 (목록은 I, II, III 순서)"""
    base, n = split_roman(name)
    if not n:
        return next((b for b in boxes if same_name(b[0], name)), None)
    nb = sell._norm(base)
    cards = sorted((b for b in boxes if sell._norm(b[0]).startswith(nb) and len(sell._norm(b[0])) <= len(nb) + 4),
                   key=lambda b: b[2])
    return cards[n - 1] if len(cards) >= n else None


def _box(boxes, key):
    """key 가 들어간 글자 덩어리 (덩어리 전체의 가운데 = 버튼 가운데)"""
    return next((b for b in boxes if key in sell._norm(b[0])), None)


def _r(rect, x, y):
    return [round(min(1.0, max(0.0, x / rect[2])), 4), round(min(1.0, max(0.0, y / rect[3])), 4)]


def shop_layout(boxes, rect):
    """Stella's Workshop 창 글자들 → {search_pos, shop_close_pos, list_region} (창 비율) 또는 None"""
    title = next((b for b in boxes if "workshop" in sell._norm(b[0])), None)       # 제목 전체의 가운데
    if not title:
        return None
    W, H = rect[2], rect[3]
    item = next((b for b in boxes if sell._norm(b[0]) == "ltem"), None)
    lant = next((b for b in boxes if sell._norm(b[0]).startswith("lontern")), None)
    k = abs(lant[1] - item[1]) * W / SHOP_REF_TABS if item and lant else H / 1080
    tx, ty = title[1] * W, title[2] * H
    return {"search_pos": _r(rect, tx + SEARCH_OFF[0] * k, ty + SEARCH_OFF[1] * k),
            "shop_close_pos": _r(rect, tx + SHOP_CLOSE_OFF[0] * k, ty + SHOP_CLOSE_OFF[1] * k),
            "list_region": _r(rect, tx + LIST_OFF[0] * k, ty + LIST_OFF[1] * k) + _r(rect, tx + LIST_OFF[2] * k, ty + LIST_OFF[3] * k)}


def add_layout(boxes, rect):
    """Add Ingredients 창 글자들 → {add_all_pos, craft_pos, add_close_pos} 또는 None"""
    title = _box(boxes, "ngredlents")                  # 'Add Ingredients' · 'Add Everything' 도 앞 글자는 안 봄
    add = _box(boxes, "everythlng")
    if not (title and add):
        return None
    W, H = rect[2], rect[3]
    ax, ay, hx, hy = add[1] * W, add[2] * H, title[1] * W, title[2] * H
    k = (ay - hy) / ADD_REF_GAP if ay > hy else H / 1080
    return {"add_all_pos": _r(rect, ax, ay), "craft_pos": _r(rect, ax + CRAFT_FROM_ADD * k, ay),
            "add_close_pos": _r(rect, hx + ADD_CLOSE_OFF[0] * k, hy + ADD_CLOSE_OFF[1] * k)}


def open_recipe(boxes):
    """Open Recipe 버튼 — 작은 창에선 첫 글자를 잘못 읽기도 해서 ('Dpen Recipe') 첫 글자는 안 봄"""
    b = _box(boxes, "penreclpe")
    return [round(b[1], 4), round(b[2], 4)] if b else None


def autocal(ocr, click, wait, rect, status, wait_f=120.0):
    """포션 제작 위치 자동 보정 — 플레이어가 Stella 앞에서 F 를 직접 누르면 (대화창 없이 바로 제작 창)
    제작 창 → 목록 첫 칸 → Open Recipe → Add Ingredients 창까지 글자로 재고 닫음 (Add Everything · Craft 는 안 누름)
    → 찾은 위치 {키: 값}"""
    status("Stella 앞에서 F 를 눌러 주세요")
    end = time.time() + wait_f
    while True:
        shop = shop_layout(ocr(None), rect())
        if shop:
            break
        if time.time() > end:
            raise RuntimeError("제작 창이 안 보임 — Stella 앞에서 F 를 눌러 주세요")
        wait(0.4)
    status("제작 창 찾음 — 이제 만지지 마세요")
    found = dict(shop)
    wait(0.5)
    cards = sorted((b for b in ocr(found["list_region"]) if len(sell._norm(b[0])) >= 6), key=lambda b: b[2])
    if not cards:
        raise RuntimeError("제작 목록이 비어 있음")
    click([cards[0][1], cards[0][2]])                    # 목록 첫 칸 (Open Recipe 가 보이게)
    wait(0.6)
    orc = open_recipe(ocr(None))
    if not orc:
        raise RuntimeError("Open Recipe 버튼을 못 찾음")
    found["open_recipe_pos"] = orc
    status("Open Recipe 확인 중 (재료는 안 넣음)")
    click(orc)
    add, end = None, time.time() + 4
    while not add and time.time() < end:
        wait(0.4)
        add = add_layout(ocr(None), rect())
    if not add:
        raise RuntimeError("Add Ingredients 창을 못 찾음")
    found.update(add)
    status("창 닫는 중")
    click(add["add_close_pos"])
    wait(0.4)
    click(found["shop_close_pos"])
    wait(0.3)
    return found


# 매크로 기준 위치 설정 → 포션 자동 제작 (제작 창 버튼 · 자동 보정으로 채움)
POS_KEYS = (("search_pos", "검색창"), ("shop_close_pos", "제작 창 X"), ("open_recipe_pos", "Open Recipe 버튼"),
            ("add_all_pos", "Add Everything 버튼"), ("craft_pos", "Craft 버튼"), ("add_close_pos", "Add Ingredients 창 X"))


class Crafter(popping.Popper):
    LABEL = "포션 자동 제작"

    def __init__(self, get_cfg, log, borrow, place_index, on_done=None):
        super().__init__(lambda: {}, log)
        self.get_mcfg = get_cfg
        self.borrow = borrow                 # 이동기 빌려 쓰기 (with borrow(stop) as mv)
        self.place_index = place_index       # () → 포션 제작 장소 번호 또는 None
        self.on_done = on_done
        self.crafted = 0
        self.name = None
        self.test = False                    # 제작 테스트로 돌리는 중 (매크로 · 기능이 꺼져 있어도 끝까지)

    def snapshot(self):
        s = super().snapshot()
        s["crafted"] = self.crafted
        s["name"] = self.name if self.running() else None
        return s

    def start_job(self, name, test=False):
        if self.running():
            return False
        import threading
        self.stop_ev = threading.Event()
        self.name = name
        self.test = test
        self.thread = threading.Thread(target=self._job, args=(name, self.stop_ev), daemon=True)
        self.thread.start()
        return True

    def _job(self, name, stop):
        ok = False
        try:
            ok = self._run(name, stop)
        except popping.Stopped:
            self.log(f"{self.LABEL} 정지", "d")
        except Exception as e:
            self.log(f"{self.LABEL} 오류: {e}", "r")
        finally:
            self._set(msg="대기")
            if self.on_done:
                try:
                    self.on_done(name, ok)
                except Exception:
                    pass

    # ---- 화면 읽기
    def _shop(self, stop, timeout):
        """제작 창 위치들 (화면 전체 글자로 · 못 읽으면 None) — 보정을 안 했을 때만 씀 (느림)"""
        end = time.time() + timeout
        while True:
            lay = shop_layout(macro.ocr_boxes(None), self._rect(stop))
            if lay or time.time() > end:
                return lay
            self._wait(0.4, stop)

    def _list_open(self, stop, region, timeout):
        """목록 칸에 카드 글자가 보이면 제작 창이 열린 것 (목록 칸만 읽어서 빠름)"""
        end = time.time() + timeout
        while True:
            if any(len(sell._norm(b[0])) >= 4 for b in macro.ocr_boxes(region)):
                return True
            if time.time() > end:
                return False
            self._wait(0.25, stop)

    def _pick_result(self, stop, shop, name, query):
        """검색창에 query 입력 → Enter → 결과 카드 중 그 포션을 누름 → True"""
        self._click(shop["search_pos"], stop)
        self._wait(0.15, stop)
        macro.key_combo(["ctrl", "a"])
        self._wait(0.05, stop)
        macro.paste_text(query)
        self._wait(0.1, stop)
        macro.key_tap("enter")
        end = time.time() + 2.0
        while True:
            self._wait(0.3, stop)
            b = pick_card(macro.ocr_boxes(shop["list_region"]), name)
            if b or time.time() > end:
                break
        if b:
            self._click([b[1], b[2]], stop)
        return bool(b)

    # ---- 전체 순서
    def _run(self, name, stop):
        c = self.get_mcfg() or {}
        i = self.place_index()
        if i is None:
            self.log(f"{self.LABEL} 안 함 — 이동 탭에서 '포션 제작 장소' 를 지정해 주세요", "n")
            return False
        self.log(f"{self.LABEL} — {name}", "c")
        # 1. 포션 제작 장소로
        self._set(msg="포션 제작 장소로 가는 중")
        with self.borrow(stop) as mv:
            mv.plan(mv.start_time(i) + mv.place_time(i))
            mv.go_start(i)                   # 반대쪽 기준 장소 → 퀘스트 보드 → E · Exit · D → 나머지 지점
            mv.walk(i)
        try:
            with macro.fast_timing():
                return self._craft(name, c, stop)
        finally:
            self._set(msg="리셋")
            macro.respawn()

    def _craft(self, name, c, stop):
        saved = {k: c.get(k) for k in ("search_pos", "list_region", "shop_close_pos", "open_recipe_pos",
                                       "add_all_pos", "craft_pos", "add_close_pos")}
        cal = all(saved[k] for k in ("search_pos", "list_region", "shop_close_pos"))
        # 2. F → Stella's Workshop (대화창 없이 바로 제작 창) — 보정했으면 목록 칸만 읽어서 열렸는지 봄 (빠름)
        self._set(msg="F (Stella's Workshop)")
        macro.key_tap("f")
        shop = None
        if cal:
            if self._list_open(stop, saved["list_region"], float(c.get("f_wait", 2.5))):
                shop = saved
            else:
                macro.key_tap("f")                           # 한 번 더 (멀거나 씹힘)
                if self._list_open(stop, saved["list_region"], 2.5):
                    shop = saved
        if not shop:                                         # 보정 안 함 · 목록이 안 보임 → 화면 전체 글자로
            shop = self._shop(stop, 1.0 if cal else float(c.get("f_wait", 2.5)))
            if not shop and not cal:
                macro.key_tap("f")
                shop = self._shop(stop, 3.0)
        if not shop:
            self.log(f"{self.LABEL} — 제작 창(Stella's Workshop)이 안 열림 (포션 제작 장소 확인)", "y")
            return False
        # 3. 검색 → Enter → 결과 누르기 (번호가 있으면 번호 없이 검색 · I II III 순서로 고름)
        self._set(msg=f"{name} 검색")
        base, _n = split_roman(name)
        found = self._pick_result(stop, shop, name, base)
        if not found and safe_query(base) != base:
            found = self._pick_result(stop, shop, name, safe_query(base))
        if not found:
            self.log(f"{self.LABEL} — 목록에서 '{name}' 을 못 찾음", "y")
            self._click(shop["shop_close_pos"], stop)
            return False
        # 4. Open Recipe — 보정한 위치가 있으면 바로 누름 (없으면 화면 전체 글자로 찾음)
        self._wait(0.35, stop)
        orc = saved["open_recipe_pos"] or open_recipe(macro.ocr_boxes(None))
        if not orc:
            self.log(f"{self.LABEL} — Open Recipe 버튼을 못 찾음", "y")
            return False
        self._set(msg="Open Recipe")
        self._click(orc, stop)
        # 5. Add Ingredients 창 → Add Everything → Craft → Add Everything
        if all(saved[k] for k in ("add_all_pos", "craft_pos", "add_close_pos")):
            add = saved
            self._wait(0.5, stop)
        else:
            add, end = None, time.time() + 3
            while not add and time.time() < end:
                self._wait(0.3, stop)
                add = add_layout(macro.ocr_boxes(None), self._rect(stop))
        if not add:
            self.log(f"{self.LABEL} — Add Ingredients 창을 못 찾음", "y")
            return False
        self._set(msg="Add Everything → Craft → Add Everything")
        self._click(add["add_all_pos"], stop)
        self._wait(0.25, stop)
        self._click(add["craft_pos"], stop)
        self._wait(0.25, stop)
        self._click(add["add_all_pos"], stop)
        self._wait(0.4, stop)
        # 6. 창은 안 닫고 그 자리에서 리셋 (_run 의 finally)
        self.crafted += 1
        self.log(f"{self.LABEL} 완료 — {name} 재료 다시 채움", "g")
        return True
