# -*- coding: utf-8 -*-
"""
Acrux 본체 공통 — 설정 (config.json) · 경로 · 로그 · 로블록스 링크 열기
모드가 쓰는 설정은 NORMALIZERS 로 각 모드가 정리함 (본체는 모르는 키를 지우지 않음)
"""
import json
import os
import re
import secrets
import subprocess
import sys
import threading
import time
from pathlib import Path

import procs

if getattr(sys, "frozen", False):          # exe 로 묶었을 때
    BASE = Path(sys.executable).resolve().parent
else:
    BASE = Path(__file__).resolve().parent
# 런처로 설치한 경우 설정은 앱 폴더 밖(ACRUX_DATA)에 둠 — 업데이트로 앱 폴더를 갈아도 설정 유지
DATA_BASE = Path(os.environ["ACRUX_DATA"]) if os.environ.get("ACRUX_DATA") else BASE
DATA_BASE.mkdir(parents=True, exist_ok=True)
CONFIG_PATH = DATA_BASE / "config.json"

DEFAULT_CONFIG = {
    "lang": "",               # 화면 언어: ko / en / ja (비어 있으면 처음 켤 때 윈도우 언어로 정함)
    "tutorials_done": [],     # 끝까지 완료한 튜토리얼 ID 목록
    "online_share": True,     # 사용자 수 집계에 참여 ('켜져 있음' 신호만 보냄 · 끄면 숫자만 봄)
    "install_id": "",         # 사용자 수 집계용 무작위 번호 (처음 켤 때 만듦 · 개인 정보 아님)
    "discord": {},            # 디스코드 연결 (매크로 봇): {"token": 앱 전용 열쇠, "name": 디스코드 이름} — 화면에서 못 바꿈
    "menu_tab": "macro",      # 메인 화면 설정 탭 (biome / macro / acrux · 모드 탭) — 마지막에 본 탭
    "win_size": "",           # 기본 창 크기를 맞춘 적 있는지 (맞춘 크기) — 한 번만 맞춤
    "play": {},               # 게임 입장: Play 버튼 자동 클릭 (복구 · 복귀) (아래 PLAY_DEFAULT)
    "pop": {},                # 팝핑 공통 설정: 딜레이 · 이름 일치율 (아래 POP_DEFAULT · 매크로 탭 팝핑 기능이 같이 씀)
    "ret": {},                # 복구 · 복귀: {"ps_link": 내 브섭 링크, "start_wait"} (모드가 쓰는 다른 키는 그대로 둠)
    "biome": {},              # 바이옴 매크로 설정 (아래 BIOME_DEFAULT)
    "ocr_engine": "auto",     # Acrux 설정 · OCR 감지 방식: auto / rapid / windows
    "macro_on": False,        # 메인 화면 '매크로' 버튼 — 꺼져 있으면 매크로 탭 기능이 전부 안 돎 (켤 때마다 꺼진 상태로 시작)
    "mfish": {},              # 매크로 탭 · 자동 낚시 (아래 MFISH_DEFAULT)
    "mitem": {},              # 매크로 탭 · 오토 아이템 사용 (아래 MITEM_DEFAULT)
    "mcraft": {},             # 매크로 탭 · 포션 자동 제작 (아래 MCRAFT_DEFAULT)
    "mmerch": {},             # 매크로 탭 · 상인 자동 구매 (아래 MMERCH_DEFAULT)
    "mmatch": {},             # 매크로 탭 · 오토 메모리 매치 (아래 MMATCH_DEFAULT)
    "mpop": {},               # 매크로 탭 · 레어 바이옴 자동 팝핑 (내 서버) (아래 MPOP_DEFAULT)
    "mblood": {},             # 매크로 탭 · 블러드 레인 자동 생성 (할로윈 이벤트) (아래 MBLOOD_DEFAULT)
    "base": {},               # 매크로 기준 위치 — 여러 기능이 같이 쓰는 위치 (아래 BASE_DEFAULT)
    "move": {},               # 이동 — 기준 장소로 가는 방법 · 장소별 경로 (아래 MOVE_DEFAULT)
    "stats": {},              # 통계 올타임: 기능마다 센 값 + since (처음 기록한 시각)
    "mods": {},               # 모드 · 플러그인: {id: {"enabled": bool}} (모드 설정은 각 모드가 자기 키로)
}
BIOME_DEFAULT = {
    "enabled": False,         # 작동 중일 때 바이옴 웹후크 전송
    "player": "",             # 로블록스 실제 닉네임 (디스플레이 이름 X)
    "ps_link": "",            # 비공개 서버 링크 (임베드에 표시)
    "webhooks": ["", ""],     # 디스코드 웹후크 주소 (최대 2개)
    "everyone": {"GLITCHED": True, "DREAMSPACE": True, "CYBERSPACE": True},   # 희귀 바이옴별 @everyone
    "roles": {},              # 바이옴별 멘션할 역할 ID
}
PLAY_DEFAULT = {
    "pos": None,              # Play 버튼 위치 (로블록스 창 기준 비율) — 플레이어가 직접 지정 (기본값 없음)
    "load_wait": 5,           # 로블록스 창이 뜬 뒤 첫 클릭까지 대기 (초)
    "skip_pos": None,         # Click to skip 버튼 위치 — 플레이어가 직접 지정 (Play 와 번갈아 클릭)
    "interval": 0.15,         # Play ↔ Click to skip 클릭 간격 (초)
    "max_time": 300,          # 게임 입장이 확인될 때까지 최대 시도 시간 (초)
}
POP_DEFAULT = {
    "match_threshold": 70,        # 이름 일치율 기준 (%)
    "delays": {},                 # 동작 사이 대기 (popping.DELAY_DEFAULT)
}
BASE_INV_KEYS = ("inventory_pos", "items_pos", "search_pos", "item_pos", "amount_pos", "use_pos")
BASE_DEFAULT = {
    # 매크로 기준 위치 — 한 기능에서만 쓰는 게 아닌 위치 (로블록스 창 기준 비율) · 이동 기능의 기준 위치도 여기에
    # 인벤토리: Inventory · Items · Search 버튼, 아이템 칸, 수량 입력칸, Use 버튼, OCR 영역(검색 결과 아이템 이름 · 개수)
    "inventory_pos": None, "items_pos": None, "search_pos": None,
    "item_pos": None, "amount_pos": None, "use_pos": None,
    "ocr_region": None,
    "notice_region": None,        # 알림 영역 (오른쪽에 뜨는 알림 카드 · 낚시의 "Cannot Fish" 등)
    # 이동 · 기준 장소로 가기에서 누르는 버튼
    "chat_pos": None,             # 채팅 버튼 (왼쪽 위)
    "collection_pos": None,       # 도감 버튼 (왼쪽 메뉴의 Collection)
    "collection_close": None,     # 도감 Exit 버튼
    "dialog_pos": None,           # NPC 대화창 (눌러서 대화 넘기기 · 물고기 판매 등 여러 기능이 같이 씀)
}
MOVE_FEATS = ("mfish", "mpop", "mcraft", "mmatch")    # 장소를 따로 둘 수 있는 기능 (매크로 기준 위치 설정의 각 기능 칸)
# 기능마다 정해진 장소 (사용자가 만들거나 지우지 않음 · 지점 위치와 시간만 지정)
MOVE_TEMPLATES = {"mfish": (("fish_spot", "낚시 장소"), ("sell_spot", "물고기 판매 장소")), "mcraft": (("craft_spot", "포션 제작 장소"),),
                  "mmatch": (("match_spot", "메모리 매치 장소"),)}
ROUTE_KEYS = ("w", "a", "s", "d", "space", "e")         # 길 기록에 쓰는 키 (move.REC_KEYS 와 같음)


def _route(r):
    """기록한 길 정리: {"ev": [[초, 키, 0|1] | [초, "click", [x, y]]], "len": 초} · 이상하면 None"""
    if not isinstance(r, dict) or not isinstance(r.get("ev"), list):
        return None
    ev, last = [], 0.0
    for e in r["ev"][:20000]:
        if not isinstance(e, (list, tuple)) or len(e) != 3:
            continue
        try:
            t = round(min(REC_MAX_SEC, max(last, float(e[0]))), 3)
        except (TypeError, ValueError):
            continue
        if e[1] == "click":
            try:
                pos = [round(min(1.0, max(0.0, float(x))), 4) for x in e[2]][:2]
            except (TypeError, ValueError):
                continue
            if len(pos) != 2:
                continue
            ev.append([t, "click", pos])
        elif e[1] in ROUTE_KEYS:
            ev.append([t, e[1], 1 if e[2] else 0])
        else:
            continue
        last = t
    if not ev:
        return None
    try:
        ln = round(min(REC_MAX_SEC, max(last, float(r.get("len") or 0))), 2)
    except (TypeError, ValueError):
        ln = last
    return {"ev": ev, "len": ln}


REC_MAX_SEC = 300.0
MOVE_DEFAULT = {
    # 기준 장소로 가기 (사용자가 정한 순서):
    #  Esc → R → Enter (리셋 · 0.5초 간격) → 3.5초 → / → 채팅 버튼 → 도감 버튼 → 도감 Exit → / → Enter (각 0.5초)
    #  → W 1초 → W+A 7초 → A 1초 (0.75초 뒤부터 W 0.25초 같이) → 0.5초 → 우클릭 드래그(위에서 내려다보기) → O 2.5초 (최대 줌) → 갈 곳 우클릭
    "reset_wait": 3.5,            # 리셋 후 대기 (초)
    "w_time": 1.0,                # W 누르기 (초)
    "wa_time": 7.0,               # W + A 같이 누르기 (초)
    "a_time": 1.0,                # 그 다음 A 누르기 (초)
    # 반대쪽 기준 장소 (포션 제작 장소 · 낚시와 정반대 방향): 리셋 · 카메라 정렬 · 내려다보기+줌 → S+D → S (끝에 A 같이)
    "rsd_time": 1.0,              # S + D 같이 누르기 (초)
    "rs_time": 6.0,               # 그 다음 S 누르기 (초)
    "ra_time": 1.0,               # S 를 누르는 마지막 이 시간 동안 A 도 같이 (초) — 기본: S 누른 지 5초 뒤부터 1초
    # 반대쪽 길: 1번 지점(퀘스트 보드) 뒤 E → q_wait 초 → 퀘스트 창 Exit → D qd_time 초 → qd_wait 초 대기 · 2번 지점(스텔라 포탈) 뒤 D rd_time 초 = 포션 제작 장소
    "q_wait": 1.5,
    "qd_time": 2.0,               # 퀘스트 창 Exit 다음 D 누르기 (초)
    "qd_wait": 0.65,              # 그 D 뒤 대기 (초)
    "rd_time": 2.0,
    "quest_exit_pos": None,       # 퀘스트 창 Exit 버튼 [x, y]
    "sd_time": 0.25,              # 그 다음 S + D 같이 누르기 (초)
    "w2_time": 0.25,              # A 를 누르는 마지막 이 시간 동안 W 도 같이 (초) — A 를 누른 지 a_time - w2_time 뒤에 시작
    "tilt_px": 800,               # 우클릭을 누른 채 마우스를 아래로 끄는 거리 (px) — 위에서 내려다보기
    "o_time": 2.5,                # O 누르기 (초) — 최대 줌
    "button": "right",            # 이동할 곳을 누를 마우스 버튼 (Click to Move)
    "margin": 0.3,                # 잰 시간에 더 기다릴 여유 (초)
    # 장소: [{"name", "feat": 쓰는 기능, "key": 정해진 장소 이름표, "route": 기록한 길 또는 None}]
    #   route = {"ev": [[초, 키, 1 누름 · 0 뗌] 또는 [초, "click", [x, y]]], "len": 처음 키부터 도착까지 (초)} — 기준 장소에서 WASD · Space · E 로 걸어간 그대로
    #   (예전 Click to Move 의 "points" 는 더 안 씀)
    "places": [],
}
MPOP_DEFAULT = {
    # 매크로 탭 · 레어 바이옴 자동 팝핑 (내 서버) — 위치 · OCR 영역은 매크로 기준 위치(base), 딜레이 · 일치율은 팝핑 공통 설정(pop)을 같이 씀
    "enabled": False,             # 켜짐: 지금 켜져 있는 로블록스(내 서버)에서 레어 바이옴이 감지되면 포션 사용
    "start_delay": 1.0,           # 바이옴 감지 후 인벤토리를 열기까지 대기 (초)
    "close_inventory": True,      # 다 쓰고 Inventory 버튼을 한 번 더 눌러 닫기
    "templates": {},              # 바이옴별 포션 목록
    "biomes_on": {},              # 켜진 바이옴에서만 (기본 전부 켜짐)
}
MBLOOD_DEFAULT = {
    # 매크로 탭 · 블러드 레인 자동 생성 (할로윈 이벤트) — 내 서버에서 PUMPKIN MOON · GRAVEYARD 가 떠 있으면 그 바이옴에 한 번
    # Cursed Rune Fragment 가 2개 이상일 때 1개 사용 (popping.BLOOD_ITEM)
    # 위치 · OCR 영역은 매크로 기준 위치(base), 딜레이 · 일치율은 팝핑 공통 설정(pop)을 같이 씀
    "enabled": False,
    "start_delay": 1.0,           # 바이옴 감지 후 인벤토리를 열기까지 대기 (초)
    "close_inventory": True,      # 다 쓰고 Inventory 버튼을 한 번 더 눌러 닫기
}
MITEM_DEFAULT = {
    # 매크로 탭 · 오토 아이템 사용 — 쿨타임마다 인벤토리에서 아이템 사용
    # 위치 · OCR 영역은 매크로 기준 위치(base), 딜레이 · 일치율은 팝핑 공통 설정(pop)을 같이 씀
    "enabled": False,
    "strange": True,              # Strange Controller 사용
    "strange_min": 10.5,          # Strange Controller 간격 (분)
    "randomizer": True,           # Biome Randomizer 사용
    "randomizer_min": 18.0,       # Biome Randomizer 간격 (분)
    "close_inventory": True,      # 다 쓰고 Inventory 버튼을 한 번 더 눌러 닫기
}
MCRAFT_DEFAULT = {
    # 매크로 탭 · 포션 자동 제작 — 알림에 "Auto Crafted" 가 뜨면 Stella 에게 가서 그 포션을 다시 채워 둠
    # (검색 → Open Recipe → Add Everything → Craft → Add Everything) · 알림 영역은 통합 위치, 가는 길은 이동 탭
    "enabled": False,
    "potion": "",                 # 제작할 포션 (직접 입력) — 있으면 알림이 뜰 때 이 포션 · 비워 두면 알림에서 이름을 읽음 (OCR)
    "f_wait": 2.5,                # F 누른 뒤 제작 창이 뜰 때까지 (초)
    # 제작 창 위치 (자동 보정 · 직접 지정) — 실행 중엔 글자로 먼저 찾고, 못 찾을 때 이 값을 씀
    "search_pos": None,           # 검색창
    "list_region": None,          # 검색 결과 목록
    "shop_close_pos": None,       # 제작 창 X
    "open_recipe_pos": None,      # Open Recipe
    "add_all_pos": None,          # Add Everything
    "craft_pos": None,            # Craft
    "add_close_pos": None,        # Add Ingredients 창 X
    "cal_shop": False,            # 제작 창 자동 보정을 끝냈는지 (튜토리얼 안내용)
}
MMATCH_DEFAULT = {
    # 매크로 탭 · 오토 메모리 매치 — 메모리 매치 장소로 가서 E · 할 수 있으면 카드 짝 맞추기 · 쿨타임이면 남은 시간 뒤에 다시
    "enabled": False,
    "e_wait": 2.0,                # E 누른 뒤 알림 창이 뜰 때까지 (초)
    "flip_wait": 0.35,            # 카드를 누른 뒤 앞면을 읽기까지 (초)
    # 메모리 매치 창 · 카드 판 위치 (자동 보정 · 직접 지정) — 있으면 그 자리를 누름
    "note_region": None,          # 알림 창 영역 [x1, y1, x2, y2] (이 영역만 읽음)
    "note_close_pos": None,       # 알림 창 X
    "note_btn_pos": None,         # Start Memory Match / Available 버튼
    "ad_pos": None,               # Watch AD 칸
    "board_close_pos": None,      # 카드 판 Close
    "board_title": None,          # 카드 판 제목 'Memory Match' 가운데 (카드 자리 계산 기준)
    "board_k": None,              # 카드 판 크기 (1080p 대비)
    "cal_note": False,            # 메모리 매치 창 자동 보정을 한 번이라도 끝냈는지 (튜토리얼 안내용)
    "cal_board": False,           # 카드 판 자동 보정을 끝냈는지 (끝냈으면 카드 자리를 그 값으로 씀)
}
# 상인 아이템
MERCHANT_ITEMS = {
    "Jester": ("Oblivion Potion", "Heavenly Potion", "Potion of Bound", "Rune of Everything", "Random Potion Sack",
               "Stella's Candle", "Lucky Potion"),
    "Mari": ("Void Coin", "Lucky Penny", "Gear A", "Gear B", "Mixed Potion", "Speed Potion", "Lucky Potion", "Fortune Spoid I"),
}
MMERCH_DEFAULT = {
    # 매크로 탭 · 상인 자동 구매 — 채팅에 상인 도착이 뜨면 Merchant Teleporter 로 가서 고른 아이템 구매
    # 위치 템플릿은 1920x1080 값 · 대화창 · 인벤토리는 통합 위치
    "enabled": False,
    "check_sec": 15.0,            # 채팅창 확인 간격 (초)
    "teleport_wait": 3.0,         # Merchant Teleporter 사용 후 대기 (초)
    "buy": {},                    # {"Mari_Void Coin": 1, ...} — 고른 아이템만 (개수는 Set to Max)
    "auto_cal": True,             # 상점이 열리면 Purchase 글자로 상점 위치를 자동 보정
    "cal_shop": False,            # 상인 자동 보정(지금 보정)을 끝냈는지 (튜토리얼 안내용)
    "chat_region": None,          # 채팅 글자 영역 [x1, y1, x2, y2]
    "open_pos": None,             # 대화 선택지 Open (글자로 못 찾을 때)
    "first_slot": None,           # 상점 첫 번째 칸
    "second_slot": None,          # 상점 두 번째 칸 (칸 간격 계산용)
    "item_region": None,          # 칸을 눌렀을 때 아이템 이름이 뜨는 영역
    "max_pos": None,              # Set to Max 버튼 (살 수 있는 만큼)
    "purchase_pos": None,         # Purchase 버튼
    "close_pos": None,            # 상점 닫기 X
}
MFISH_DEFAULT = {
    # 매크로 탭 · 자동 낚시 (제자리 낚시) — 위치는 로블록스 창 기준 비율
    "enabled": False,
    "fish_btn": None,             # Fish / Exit 버튼 (같은 자리 · 파랑 = Fish, 빨강 = Exit)
    "bar_region": None,           # 릴링 바(위쪽 바) 영역 [x1, y1, x2, y2]
    "close_pos": None,            # 결과창 X
    "title_pos": None,            # 결과창 제목 (선택 · 색으로 성공/쓰레기/실패 구분)
    "panel_region": None,         # 낚시 대기 창 영역 → Fish 버튼 (· 미니게임 창이 없으면 릴링 바) 자동 계산
    "reel_region": None,          # 낚시 미니게임 창 영역 → 릴링 바 자동 계산 (대기 창보다 넓음)
    "result_region": None,        # 결과창 영역 → 결과창 X · 제목 자동 계산
    "bite_max": 60.0, "target_pct": 0, "lead_ms": 0, "click_ms": 25, "click_gap_ms": 45, "cast_retry": 3,
    # 판매 (인벤토리가 가득 차면: 물고기 판매 장소 → E → 대화 넘기기 → Sell Fish → (첫 칸 → Sell All → 확인) 반복 → X → 낚시 장소)
    "sell_fish_pos": None,        # 선택지 중 [Sell Fish] 버튼
    "first_fish_pos": None,       # 상점 목록의 첫 번째 물고기 칸
    "sell_all_pos": None,         # Sell All 버튼
    "confirm_sell_pos": None,     # 확인창의 Sell 버튼
    "shop_close_pos": None,       # 상점 닫기 X
    "e_wait": 1.0,                # E 누른 뒤 대화창이 뜰 때까지 (초)
    "sell_delay": 0.0,            # 판매 클릭마다 더 기다릴 시간 (초) — 렉이 있을 때만
    "sell_v2": False,             # 판매를 빠르게 바꾼 것 적용했는지 (예전 기본값 0.4 → 0, 한 번만)
    "sell_max": 100,              # 판매 반복 최대 (클릭이 씹혀 끝없이 도는 것만 막음)
    "debug_log": False,           # 릴링 기록(fishing_log.csv) 저장 — 문제 확인용
    "click_v3": False,            # 클릭 기준을 '구간 왼쪽 변 이하'(목표 0 · 미리 누르기 0)로 바꾼 것 적용했는지 (한 번만)
    "cal_win": False,             # 낚시 창 자동 보정을 끝냈는지 (튜토리얼 안내용)
    "cal_sell": False,            # 판매 자동 보정을 끝냈는지 (튜토리얼 안내용)
}
POP_BIOMES = ("CYBERSPACE", "GLITCHED", "DREAMSPACE")
# 기본 템플릿 — 레어 바이옴 자동 팝핑
# (처음 한 번만 채워짐 · 이후엔 플레이어가 추가/삭제/순서 변경)
_STD = [("Warp Potion", 1), ("Oblivion Potion", "ALL"), ("Godlike Potion", "ALL"),
        ("Heavenly Potion", "ALL"), ("Potion of Bound", "ALL"), ("Popping Potion", "ALL")]
POP_TEMPLATES = {
    "GLITCHED": {"mode": "all", "items": [{"name": n, "amount": a, "min_have": 1} for n, a in _STD]},
    "DREAMSPACE": {"mode": "all", "items": [{"name": n, "amount": a, "min_have": 1} for n, a in _STD]},
    # 우선순위 트리 (위에서부터 조건 맞는 하나만):
    # 워프 5개 이상 → 워프 5개 / 트랜센던트 있음 → 1개 / Tidal Shifter 2개 이상 → 1개 / 워프 있음 → 전부
    "CYBERSPACE": {"mode": "one", "items": [
        {"name": "Warp Potion", "amount": 5, "min_have": 5},
        {"name": "Transcendent Potion", "amount": 1, "min_have": 1},
        {"name": "Tidal Shifter Potion", "amount": 1, "min_have": 2},
        {"name": "Warp Potion", "amount": "ALL", "min_have": 1},
    ]},
}
POP_DELAYS = {"inventory": 0.5, "items": 0.5, "search": 0.5, "typing": 0.5,
              "item_click": 0.35, "dbl_gap": 0.1, "after_dbl": 0.5, "after_enter": 0.35}
REMOVED_KEYS = ("print_all_links", "run_on_link", "cooldown_sec", "kill_multiscope", "popping",
                "debug_port")                 # 디버그 포트는 이제 켤 때마다 무작위

NO_WINDOW = getattr(subprocess, "CREATE_NO_WINDOW", 0)


# ---------------------------------------------------------------- 창과 연결되는 콜백
class Bus:
    on_log = staticmethod(lambda msg, color="": print(time.strftime("%H:%M:%S"), msg, flush=True))
    on_status = staticmethod(lambda state, text="": None)


def log(msg, color=""):
    try:
        Bus.on_log(msg, color)
    except Exception:
        pass


def status(state, text=""):
    try:
        Bus.on_status(state, text)
    except Exception:
        pass


STOP = threading.Event()


# 모드가 자기 설정을 정리하는 함수 (normalize 끝에서 차례로 부름 · d 를 고쳐서 돌려줌)
NORMALIZERS = []


# ---------------------------------------------------------------- 설정
def normalize(raw):
    d = dict(DEFAULT_CONFIG)
    d.update(raw or {})
    for k in REMOVED_KEYS:
        d.pop(k, None)
    play = dict(PLAY_DEFAULT)
    play.update(d.get("play") if isinstance(d.get("play"), dict) else {})
    play.pop("enabled", None)              # 토글 없음 (항상 작동)
    for pk in ("pos", "skip_pos"):
        try:
            play[pk] = [min(1.0, max(0.0, float(v))) for v in play[pk]][:2]
            assert len(play[pk]) == 2
        except Exception:
            play[pk] = None
    if "skip_pos" not in (d.get("play") or {}):   # 예전 설정(Play 만 4초 간격)은 새 기본값으로
        play["interval"] = PLAY_DEFAULT["interval"]
    for k, lo in (("load_wait", 0), ("interval", 0.05), ("max_time", 10)):
        try:
            play[k] = max(lo, float(play[k]))
        except (TypeError, ValueError):
            play[k] = PLAY_DEFAULT[k]
    play.pop("pre_steps", None)              # 예전 설정 (접속 전 동작) 없앰
    d["play"] = play
    def _ratio_list(v, n):
        try:
            v = [min(1.0, max(0.0, float(x))) for x in v][:n]
            return v if len(v) == n else None
        except Exception:
            return None
    pop = dict(d.get("pop") if isinstance(d.get("pop"), dict) else {})   # 모드가 쓰는 다른 키는 그대로 둠
    try:
        th = float(pop.get("match_threshold", 70))
        if not pop.get("th70") and th == 50:      # 예전 기본값(50%) 그대로면 새 기본값 70% 로 한 번 바꿈
            th = 70
        pop["match_threshold"] = min(100.0, max(1.0, th))
    except (TypeError, ValueError):
        pop["match_threshold"] = 70
    pop["th70"] = True
    dl = dict(pop.get("delays") or {}) if isinstance(pop.get("delays"), dict) else {}
    for k, v in POP_DELAYS.items():
        try:
            dl[k] = max(0.0, float(dl.get(k, v)))
        except (TypeError, ValueError):
            dl[k] = v
    pop["delays"] = dl
    d["pop"] = pop
    # 매크로 기준 위치: 예전엔 레어 바이옴 자동 팝핑(mpop) · 자동 낚시(mfish)에 따로 있던 걸 한 번 옮겨 옴
    old_mp = d.get("mpop") if isinstance(d.get("mpop"), dict) else {}
    old_mf = d.get("mfish") if isinstance(d.get("mfish"), dict) else {}
    base = dict(BASE_DEFAULT)
    base.update(d.get("base") if isinstance(d.get("base"), dict) else {})
    for k in (*BASE_INV_KEYS, "ocr_region"):
        if not base.get(k) and old_mp.get(k):
            base[k] = old_mp[k]
    if not base.get("notice_region") and old_mf.get("notice_region"):
        base["notice_region"] = old_mf["notice_region"]
    if not base.get("dialog_pos") and old_mf.get("dialog_pos"):    # 대화창은 자동 낚시에서 통합 위치로 옮김
        base["dialog_pos"] = old_mf["dialog_pos"]
    for k in (*BASE_INV_KEYS, "chat_pos", "collection_pos", "collection_close", "dialog_pos"):
        base[k] = _ratio_list(base.get(k), 2)
    for k in ("ocr_region", "notice_region"):
        base[k] = _ratio_list(base.get(k), 4)
    d["base"] = {k: base[k] for k in BASE_DEFAULT}
    mv = dict(MOVE_DEFAULT)
    mv.update(d.get("move") if isinstance(d.get("move"), dict) else {})
    if not mv.get("v18") and mv.get("reset_wait") == 2.6:      # 예전기본값이면 새 기본값으로 한 번
        mv["reset_wait"] = 3.5
    if not mv.get("v129") and mv.get("w_time") == 0.85:        # 예전 기본값이면 새 기본값(1초)으로 한 번
        mv["w_time"] = 1.0
    if not mv.get("v130") and "wa_time" in (d.get("move") or {}):  # 이동 방식 변경: 저장된 W+A 를 1초 줄이고 뒤에 A → W 를 붙임 (한 번)
        try:
            mv["wa_time"] = max(0.0, float(mv.get("wa_time", 8.0)) - 1.0)
        except (TypeError, ValueError):
            mv["wa_time"] = 7.0
    if not mv.get("v132") and mv.get("rs_time") == 7.0:        # 반대쪽 기준 장소 S 7초 → 6초 (A 시작도 1초 당겨짐 · 한 번)
        mv["rs_time"] = 6.0
    if not mv.get("v131") and mv.get("a_time") == 0.75:        # A 뒤에 W 를 따로 누르던 방식 → A 1초 안에 W 를 겹쳐 누름 (한 번)
        mv["a_time"] = 1.0
    for k, lo, hi, cast in (("reset_wait", 0.5, 15, float), ("w_time", 0, 30, float), ("wa_time", 0, 60, float),
                            ("a_time", 0, 30, float), ("w2_time", 0, 30, float), ("sd_time", 0, 30, float), ("rsd_time", 0, 30, float),
                            ("rs_time", 0, 60, float), ("ra_time", 0, 30, float), ("q_wait", 0, 15, float), ("qd_time", 0, 30, float), ("qd_wait", 0, 15, float), ("rd_time", 0, 30, float),
                            ("o_time", 0, 15, float), ("tilt_px", 0, 5000, int), ("margin", 0, 5, float)):
        try:
            mv[k] = cast(min(hi, max(lo, float(mv.get(k, MOVE_DEFAULT[k])))))
        except (TypeError, ValueError):
            mv[k] = MOVE_DEFAULT[k]
    mv["button"] = "left" if mv.get("button") == "left" else "right"
    mv["quest_exit_pos"] = _ratio_list(mv.get("quest_exit_pos"), 2)
    places = []
    for pl in (mv.get("places") if isinstance(mv.get("places"), list) else [])[:30]:
        if not isinstance(pl, dict):
            continue
        feat = pl.get("feat") if pl.get("feat") in MOVE_FEATS else "mfish"     # 그 장소를 쓰는 기능 (예전 장소는 자동 낚시)
        places.append({"name": str(pl.get("name") or "")[:40], "feat": feat, "key": str(pl.get("key") or ""),
                       "route": _route(pl.get("route"))})
    # 정해진 장소(템플릿)가 있는 기능은 그 장소만 (이름은 템플릿대로 · 없으면 만듦)
    # (순서는 그대로 둠 — 화면이 장소를 순서 번호로 가리킴)
    out, seen = [], set()
    for pl in places:
        tpls = dict(MOVE_TEMPLATES.get(pl["feat"], ()))
        if not tpls:
            out.append(pl)
        elif pl["key"] in tpls and (pl["feat"], pl["key"]) not in seen:
            seen.add((pl["feat"], pl["key"]))
            out.append(dict(pl, name=tpls[pl["key"]]))
    for feat, tpls in MOVE_TEMPLATES.items():
        for key, name in tpls:
            if (feat, key) not in seen:
                out.append({"name": name, "feat": feat, "key": key, "route": None})
    mv["places"] = out
    d["move"] = {k: mv[k] for k in MOVE_DEFAULT}
    d["move"]["v18"] = d["move"]["v129"] = d["move"]["v130"] = d["move"]["v131"] = d["move"]["v132"] = True
    mp = dict(MPOP_DEFAULT)
    mp.update(old_mp)
    for k in (*BASE_INV_KEYS, "ocr_region"):
        mp.pop(k, None)
    mp["enabled"] = bool(mp.get("enabled"))
    mp["close_inventory"] = bool(mp.get("close_inventory", True))
    try:
        mp["start_delay"] = min(60.0, max(0.0, float(mp.get("start_delay", 1.0))))
    except (TypeError, ValueError):
        mp["start_delay"] = 1.0
    mp["templates"], mp["biomes_on"] = _norm_templates(mp)
    mp["seeded"] = True
    d["mpop"] = mp
    mm = dict(MMERCH_DEFAULT)
    mm.update(d.get("mmerch") if isinstance(d.get("mmerch"), dict) else {})
    mm["enabled"] = bool(mm.get("enabled"))
    mm["auto_cal"] = mm.get("auto_cal") is not False
    if not mm.get("v129") and mm.get("check_sec") == 30:        # 예전 기본값이면 새 기본값(15초)으로 한 번
        mm["check_sec"] = 15.0
    for k, lo, hi in (("check_sec", 10, 600), ("teleport_wait", 0.5, 15)):
        try:
            mm[k] = min(hi, max(lo, float(mm.get(k))))
        except (TypeError, ValueError):
            mm[k] = MMERCH_DEFAULT[k]
    valid = {f"{m}_{n}" for m, items in MERCHANT_ITEMS.items() for n in items}
    buy = {}
    for k, v in (mm.get("buy") if isinstance(mm.get("buy"), dict) else {}).items():
        try:
            if k in valid and int(v) > 0:
                buy[k] = min(999, int(v))
        except (TypeError, ValueError):
            pass
    mm["buy"] = buy
    for k in ("open_pos", "first_slot", "second_slot", "max_pos", "purchase_pos", "close_pos"):
        mm[k] = _ratio_list(mm.get(k), 2)
    for k in ("chat_region", "item_region"):
        mm[k] = _ratio_list(mm.get(k), 4)
    mm["cal_shop"] = bool(mm.get("cal_shop"))
    d["mmerch"] = {k: mm[k] for k in MMERCH_DEFAULT}
    d["mmerch"]["v129"] = True
    mb = dict(MBLOOD_DEFAULT)
    mb.update(d.get("mblood") if isinstance(d.get("mblood"), dict) else {})
    mb["enabled"] = bool(mb.get("enabled"))
    mb["close_inventory"] = mb.get("close_inventory") is not False
    try:
        mb["start_delay"] = min(60.0, max(0.0, float(mb.get("start_delay"))))
    except (TypeError, ValueError):
        mb["start_delay"] = MBLOOD_DEFAULT["start_delay"]
    d["mblood"] = {k: mb[k] for k in MBLOOD_DEFAULT}
    mi = dict(MITEM_DEFAULT)
    mi.update(d.get("mitem") if isinstance(d.get("mitem"), dict) else {})
    for k in ("enabled", "strange", "randomizer", "close_inventory"):
        mi[k] = bool(mi.get(k))
    for k in ("strange_min", "randomizer_min"):
        try:
            mi[k] = min(240.0, max(1.0, float(mi.get(k))))
        except (TypeError, ValueError):
            mi[k] = MITEM_DEFAULT[k]
    d["mitem"] = {k: mi[k] for k in MITEM_DEFAULT}
    mc = dict(MCRAFT_DEFAULT)
    mc.update(d.get("mcraft") if isinstance(d.get("mcraft"), dict) else {})
    mc["enabled"] = bool(mc.get("enabled"))
    mc["potion"] = " ".join(str(mc.get("potion") or "").split())[:40]
    try:
        mc["f_wait"] = min(15.0, max(0.5, float(mc.get("f_wait"))))
    except (TypeError, ValueError):
        mc["f_wait"] = MCRAFT_DEFAULT["f_wait"]
    for k in ("search_pos", "shop_close_pos", "open_recipe_pos", "add_all_pos", "craft_pos", "add_close_pos"):
        mc[k] = _ratio_list(mc.get(k), 2)
    mc["list_region"] = _ratio_list(mc.get("list_region"), 4)
    mc["cal_shop"] = bool(mc.get("cal_shop"))
    d["mcraft"] = {k: mc[k] for k in MCRAFT_DEFAULT}
    mt = dict(MMATCH_DEFAULT)
    mt.update(d.get("mmatch") if isinstance(d.get("mmatch"), dict) else {})
    mt["enabled"] = bool(mt.get("enabled"))
    for k, lo, hi in (("e_wait", 0.5, 10), ("flip_wait", 0.1, 3)):
        try:
            mt[k] = min(hi, max(lo, float(mt.get(k))))
        except (TypeError, ValueError):
            mt[k] = MMATCH_DEFAULT[k]
    for k in ("note_close_pos", "note_btn_pos", "ad_pos", "board_close_pos", "board_title"):
        mt[k] = _ratio_list(mt.get(k), 2)
    mt["note_region"] = _ratio_list(mt.get("note_region"), 4)
    try:
        mt["board_k"] = min(5.0, max(0.2, float(mt["board_k"]))) if mt.get("board_k") is not None else None
    except (TypeError, ValueError):
        mt["board_k"] = None
    for k in ("cal_note", "cal_board"):
        mt[k] = bool(mt.get(k))
    d["mmatch"] = {k: mt[k] for k in MMATCH_DEFAULT}
    d["macro_on"] = bool(d.get("macro_on"))
    mf = dict(MFISH_DEFAULT)
    mf.update(d.get("mfish") if isinstance(d.get("mfish"), dict) else {})
    mf["enabled"] = bool(mf.get("enabled"))
    mf["debug_log"] = bool(mf.get("debug_log"))
    if not mf.get("click_v3"):              # 예전 기본값을 그대로 쓰던 사람은 새 기본값(왼쪽 변 · 미리 누르기 없음)으로 한 번만
        try:
            if float(mf.get("target_pct", 0)) in (10, 20):
                mf["target_pct"] = 0
            if float(mf.get("lead_ms", 0)) == 60:
                mf["lead_ms"] = 0
        except (TypeError, ValueError):
            pass
        mf["click_v3"] = True
    for k in ("fish_btn", "close_pos", "title_pos", "sell_fish_pos", "first_fish_pos", "sell_all_pos",
              "confirm_sell_pos", "shop_close_pos"):
        mf[k] = _ratio_list(mf.get(k), 2)
    for k in ("bar_region", "panel_region", "reel_region", "result_region"):
        mf[k] = _ratio_list(mf.get(k), 4)
    for k, lo, hi in (("bite_max", 5, 600), ("target_pct", 0, 90), ("lead_ms", 0, 300), ("click_ms", 5, 200), ("click_gap_ms", 10, 500),
                      ("cast_retry", 1, 10), ("e_wait", 0.2, 10), ("sell_delay", 0, 5), ("sell_max", 1, 300)):
        try:
            mf[k] = min(hi, max(lo, float(mf.get(k, MFISH_DEFAULT[k]))))
        except (TypeError, ValueError):
            mf[k] = MFISH_DEFAULT[k]
    if not mf.get("sell_v2"):
        if mf.get("sell_delay") == 0.4:
            mf["sell_delay"] = 0.0
        if mf.get("e_wait") == 1.5:
            mf["e_wait"] = 1.0
        mf["sell_v2"] = True
    mf["cast_retry"] = int(mf["cast_retry"])
    mf["sell_max"] = int(mf["sell_max"])
    for k in ("cal_win", "cal_sell"):
        mf[k] = bool(mf.get(k))
    d["mfish"] = {k: mf[k] for k in MFISH_DEFAULT}
    if d.get("ocr_engine") not in ("auto", "rapid", "windows"):
        d["ocr_engine"] = "auto"
    d["online_share"] = bool(d.get("online_share", True))
    dc = d.get("discord") if isinstance(d.get("discord"), dict) else {}
    tok = str(dc.get("token") or "")
    d["discord"] = {"token": tok if re.fullmatch(r"[0-9a-f]{64}", tok) else "", "name": str(dc.get("name") or "")[:40]}
    if not re.fullmatch(r"[0-9a-f]{32}", str(d.get("install_id") or "")):
        d["install_id"] = secrets.token_hex(16)
    ret = dict(d.get("ret") if isinstance(d.get("ret"), dict) else {})   # 모드가 쓰는 다른 키는 그대로 둠
    try:
        start_wait = max(0.0, float(ret.get("start_wait", 7.5)))
    except (TypeError, ValueError):
        start_wait = 7.5
    # start_wait: 복귀 후 Play 로 게임 입장이 감지된 뒤, 매크로를 다시 켜기까지 대기 (초)
    ret.update(ps_link=str(ret.get("ps_link") or "").strip(), start_wait=start_wait)
    d["ret"] = ret
    bio = dict(BIOME_DEFAULT)
    bio.update(d.get("biome") if isinstance(d.get("biome"), dict) else {})
    hooks = [str(h).strip() for h in (bio.get("webhooks") or []) if isinstance(h, str)]
    bio["webhooks"] = (hooks + ["", ""])[:2]
    bio["player"] = str(bio.get("player") or "").strip()
    bio["ps_link"] = str(bio.get("ps_link") or "").strip()
    ev = dict(BIOME_DEFAULT["everyone"])
    if bio.get("ping_rare") is False:            # 예전 설정(희귀 바이옴 전체 @everyone 끔) 이어받기
        ev = {k: False for k in ev}
    ev.update({str(k).upper(): bool(v) for k, v in (bio.get("everyone") or {}).items()})
    bio["everyone"] = ev
    bio.pop("ping_rare", None)
    bio["roles"] = {str(k).upper(): str(v).strip() for k, v in (bio.get("roles") or {}).items()
                    if str(v).strip().isdigit()}
    d["biome"] = bio
    st = d.get("stats") if isinstance(d.get("stats"), dict) else {}
    stats = {}
    for k, v in st.items():
        try:
            if k == "since":
                stats[k] = float(v)
            elif isinstance(k, str) and len(k) <= 40:
                stats[k] = max(0, int(v))
        except (TypeError, ValueError):
            pass
    d["stats"] = stats
    if not re.fullmatch(r"[a-z0-9_-]{1,30}", str(d.get("menu_tab") or "")):
        d["menu_tab"] = "macro"
    d["tutorials_done"] = list(dict.fromkeys(str(x) for x in d.get("tutorials_done") or []))
    mods = d.get("mods") if isinstance(d.get("mods"), dict) else {}
    d["mods"] = {str(k)[:40]: {"enabled": bool((v or {}).get("enabled", True))} for k, v in mods.items() if isinstance(v, dict)}
    for fn in list(NORMALIZERS):
        try:
            d = fn(d) or d
        except Exception as e:
            log(f"모드 설정 정리 실패: {e}", "r")
    return d


def _norm_templates(src_cfg, biomes=POP_BIOMES, defaults=None, off=()):
    """바이옴별 포션 템플릿 · 켜진 바이옴 정리 (매크로 탭 팝핑 · 모드 공용)
    처음 한 번(seeded 전)은 비어 있는 템플릿을 기본 템플릿(defaults · 없으면 POP_TEMPLATES)으로 채움 · off: 기본으로 꺼진 바이옴"""
    defaults = defaults or POP_TEMPLATES
    tpls = {}
    src = src_cfg.get("templates") or {}
    for b in biomes:
        t = src.get(b) or {}
        # 처음 한 번 · 새로 생긴 바이옴(저장된 적 없음)만 기본 템플릿으로
        if (not src_cfg.get("seeded") or b not in src) and not (t.get("items") or []):
            t = json.loads(json.dumps(defaults.get(b) or {}))   # 처음 한 번: 기본 템플릿으로 채움
        items = []
        for it in t.get("items") or []:
            if not isinstance(it, dict):
                continue
            amt = it.get("amount", "ALL")
            if str(amt).upper() != "ALL":
                try:
                    amt = max(1, int(amt))
                except (TypeError, ValueError):
                    amt = "ALL"
            else:
                amt = "ALL"
            try:
                mh = max(1, int(it.get("min_have") or 1))
            except (TypeError, ValueError):
                mh = 1
            items.append({"name": str(it.get("name") or "").strip(), "amount": amt, "min_have": mh})
        tpls[b] = {"mode": "one" if t.get("mode") == "one" else "all", "items": items}
    on = src_cfg.get("biomes_on") if isinstance(src_cfg.get("biomes_on"), dict) else {}
    return tpls, {b: bool(on.get(b, b not in off)) for b in biomes}


def load_config():
    if CONFIG_PATH.exists():
        try:
            return normalize(json.loads(CONFIG_PATH.read_text("utf-8-sig")))
        except Exception as e:
            log(f"config.json 읽기 실패 (기본값 사용): {e}", "r")
    return normalize({})


def save_config(d):
    tmp = CONFIG_PATH.with_suffix(".tmp")
    tmp.write_text(json.dumps(d, ensure_ascii=False, indent=2), "utf-8")
    os.replace(tmp, CONFIG_PATH)


class Config:
    """창에서 바꾼 값은 set_data 로 바로 들어오고, 파일을 직접 고치면 reload 로 반영"""

    def __init__(self, data=None):
        self.lock = threading.Lock()
        self.data = data or load_config()
        self.mtime = CONFIG_PATH.stat().st_mtime if CONFIG_PATH.exists() else 0

    def set_data(self, d):
        with self.lock:
            self.data = normalize(d)

    def reload(self):
        try:
            m = CONFIG_PATH.stat().st_mtime
        except OSError:
            return
        if m != self.mtime:
            self.mtime = m
            self.set_data(load_config())

    def __getitem__(self, k):
        with self.lock:
            return self.data[k]




# ---------------------------------------------------------------- 딥링크 변환 + 실행
import urllib.parse as _up

_CODE = r"([A-Za-z0-9_-]{1,200})"


def parse_link(url):
    """비공개 서버 링크 → ("private", placeId, linkCode) / ("share", code) / None
    붙여 넣은 주소를 그대로 실행하지 않고, 필요한 값(숫자 ID · 코드)만 뽑아서 새로 만든 링크만 실행함"""
    texts = [str(url or "")]
    try:
        d = _up.unquote(texts[0])
        if d != texts[0]:
            texts.append(d)
    except Exception:
        pass
    for t in texts:
        place = re.search(r"(?:/games/|[?&/]placeId=)(\d{1,20})(?!\d)", t, re.I)
        code = re.search(r"[?&](?:privateServerLink(?:Code)?|linkCode)=" + _CODE, t, re.I)
        if place and code:
            return ("private", place.group(1), code.group(1))
        share = re.search(r"[?&]code=" + _CODE, t, re.I)
        if share and re.search(r"share[-_]?links?|/share\?|type=Server", t, re.I):
            return ("share", share.group(1))
    return None


def to_deeplink(url):
    """로블록스 비공개 서버 링크 → roblox:// 딥링크 (새로 만든 것). 변환 못 하면 None"""
    p = parse_link(url)
    if not p:
        return None
    if p[0] == "private":
        return f"roblox://placeId={p[1]}&linkCode={p[2]}"
    return f"roblox://navigation/share_links?code={p[1]}&type=Server"


def to_weblink(url):
    """같은 링크의 웹 주소 (딥링크 실행이 안 될 때 브라우저로 열기용, 새로 만든 것)"""
    p = parse_link(url)
    if not p:
        return None
    if p[0] == "private":
        return f"https://www.roblox.com/games/{p[1]}?privateServerLinkCode={p[2]}"
    return f"https://www.roblox.com/share?code={p[1]}&type=Server"


# 링크를 열기 전에 강제 종료할 로블록스 클라이언트 프로세스
ROBLOX_PROCESSES = ["RobloxPlayerBeta.exe", "RobloxPlayerLauncher.exe", "RobloxCrashHandler.exe"]


# 매크로가 일부러 로블록스를 끄거나 다시 켜는 중 (이 동안 로블록스가 꺼져도 '갑자기 꺼짐' 알림 안 띄움)
EXPECT_CLOSE = {"until": 0.0}


def expect_roblox_close(sec=45):
    EXPECT_CLOSE["until"] = max(EXPECT_CLOSE["until"], time.time() + sec)


def taskkill(names):
    """프로세스 이름 목록을 한 번에 강제 종료. 실제로 종료된 게 있으면 True"""
    if any("roblox" in n.lower() for n in names):
        expect_roblox_close()
    try:
        r = procs.kill(names)
    except Exception as e:
        log(f"프로세스 종료 실패: {e}", "r")
        return False
    if r["denied"]:
        log(f"권한이 없어 종료 못 함: {', '.join(r['denied'])} — 관리자 권한으로 실행된 프로그램이면 Acrux 도 관리자 권한으로 실행 필요", "r")
    if r["failed"]:
        log(f"프로세스 종료 실패: {', '.join(r['failed'])}", "r")
    return bool(r["killed"])



def open_link(url):
    """링크를 딥링크로 바꿔서 바로 실행 (안 되면 브라우저로) — 매크로 복귀용"""
    deep, web = to_deeplink(url), to_weblink(url)
    if not deep:
        raise ValueError("로블록스 비공개 서버 링크 형식이 아님")
    expect_roblox_close()
    try:
        os.startfile(deep)
        log(f"복귀 링크 실행 → {deep}", "g")
    except OSError:
        os.startfile(web)


def kill_roblox():
    if taskkill(ROBLOX_PROCESSES):
        log("로블록스 클라이언트 전부 종료", "d")


