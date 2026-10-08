# -*- coding: utf-8 -*-
"""
링크 감시 엔진 (창 버전 LinkWatcher.pyw 가 이 모듈을 씀)
- PC 디스코드를 디버그 포트와 함께 실행하고, 앱 내부 이벤트에서 새 메시지를 받는다.
- 토큰 사용 X, 디스코드 API 직접 호출 X (앱이 이미 받은 데이터만 읽음)
"""
import csv
import json
import os
import re
import secrets
import socket
import struct
import subprocess
import sys
import threading
import time
import urllib.request
from collections import deque
from pathlib import Path

import websocket  # pip install websocket-client

import gateway as gw_mod
import procs

if getattr(sys, "frozen", False):          # exe 로 묶었을 때
    BASE = Path(sys.executable).resolve().parent
    HOOK_PATH = Path(getattr(sys, "_MEIPASS", BASE)) / "hook.js"
else:
    BASE = Path(__file__).resolve().parent
    HOOK_PATH = BASE / "hook.js"
# 런처로 설치한 경우 설정은 앱 폴더 밖(ACRUX_DATA)에 둠 — 업데이트로 앱 폴더를 갈아도 설정 유지
DATA_BASE = Path(os.environ["ACRUX_DATA"]) if os.environ.get("ACRUX_DATA") else BASE
DATA_BASE.mkdir(parents=True, exist_ok=True)
CONFIG_PATH = DATA_BASE / "config.json"
BINDING = "__lwEmit"

DEFAULT_CONFIG = {
    "enabled": True,
    "open_link": True,        # 항상 켜짐 (감지되면 바로 접속) — 예전 설정 호환용
    "discord": "stable",
    "auto_restart_discord": True,
    "guild_ids": [],
    "channel_ids": [],
    "names": {},              # 서버 ID → 서버 이름 (창에 표시용)
    "channels": {},           # 채널 ID → {"name": 채널 이름, "guild": 서버 이름, "guildId": 서버 ID}
    "keywords": [],           # 바이옴 이름 (하나라도 포함돼야 탐, 비우면 전부)
    "ignore_words": ["Ended", "End"],   # 단어 단위로 포함되면 무시 (대소문자 구분 X)
    "max_age_sec": 15,        # 화면에는 없음: 채널을 열 때 다시 그려지는 옛 메시지 무시용
    "beep": True,
    "stable_join": True,         # 안정화 접속: 로블록스 전부 종료 → 0.5초 후 실행 (끄면 즉시 실행)
    "show_all_links": False,     # 감시 대상 밖 링크도 기록에 표시 (기본 꺼짐)
    "autostart": False,
    "lang": "",               # 화면 언어: ko / en / ja (비어 있으면 처음 켤 때 윈도우 언어로 정함)
    "tutorials_done": [],     # 끝까지 완료한 튜토리얼 ID 목록
    "online_share": True,     # 사용자 수 집계에 참여 ('켜져 있음' 신호만 보냄 · 끄면 숫자만 봄)
    "install_id": "",         # 사용자 수 집계용 무작위 번호 (처음 켤 때 만듦 · 개인 정보 아님)
    "menu_tab": "snipe",      # 메인 화면 설정 탭 (biome / snipe / macro) — 마지막에 본 탭
    "win_size": "",           # 기본 창 크기를 맞춘 적 있는지 (맞춘 크기) — 한 번만 맞춤
    "play": {},               # 오토 팝핑: Play 버튼 자동 클릭 (아래 PLAY_DEFAULT)
    "pop": {},                # 오토 팝핑: 레어 바이옴 포션 사용 (아래 POP_DEFAULT)
    "ret": {},                # 매크로 복귀: {"ps_link": 내 브섭 링크}
    "biome": {},              # 바이옴 매크로 설정 (아래 BIOME_DEFAULT)
    "ocr_engine": "auto",     # Acrux 설정 · OCR 감지 방식: auto / rapid / windows
    "macro_on": False,        # 메인 화면 '매크로' 버튼 — 꺼져 있으면 매크로 탭 기능이 전부 안 돎 (켤 때마다 꺼진 상태로 시작)
    "mfish": {},              # 매크로 탭 · 자동 낚시 (아래 MFISH_DEFAULT)
    "mitem": {},              # 매크로 탭 · 오토 아이템 사용 (아래 MITEM_DEFAULT)
    "mcraft": {},             # 매크로 탭 · 포션 자동 제작 (아래 MCRAFT_DEFAULT)
    "mmerch": {},             # 매크로 탭 · 상인 자동 구매 (아래 MMERCH_DEFAULT)
    "mpop": {},               # 매크로 탭 · 레어 바이옴 자동 팝핑 (내 서버) (아래 MPOP_DEFAULT)
    "base": {},               # 매크로 기준 위치 — 여러 기능이 같이 쓰는 위치 (아래 BASE_DEFAULT)
    "move": {},               # 이동 — 기준 장소로 가는 방법 · 장소별 경로 (아래 MOVE_DEFAULT)
    "snipe": {}               # 스나이핑 안정성 설정 (아래 SNIPE_DEFAULT)
}
# 스나이핑 안정성: 최대한 사람이 직접 링크를 누르고 들어가는 것처럼
SNIPE_DEFAULT = {
    "delay_min": 2.0,         # 링크 감지 후 접속까지 무작위 대기 (초) — 최소
    "delay_max": 4.0,         #                                       — 최대
    "direct": True,           # 켜짐: 로블록스를 바로 실행 (딥링크) / 꺼짐: 웹브라우저로 링크를 열어서 접속
    "same_link_min": 10,      # 같은 서버 링크는 N분 동안 다시 안 들어감 (0 = 끔)
    "cooldown_sec": 5,        # 한 번 접속한 뒤 다음 접속까지 최소 간격 (초)
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
    # 버튼 위치 (로블록스 창 기준 비율) — 전부 플레이어가 직접 지정
    "inventory_pos": None, "items_pos": None, "search_pos": None,
    "item_pos": None, "amount_pos": None, "use_pos": None,
    "ocr_region": None,           # 검색 결과 아이템 이름·개수 OCR 영역 [x1, y1, x2, y2]
    "match_threshold": 70,        # 이름 일치율 기준 (%)
    "delays": {},                 # 동작 사이 대기 (popping.DELAY_DEFAULT)
    # 바이옴별 템플릿: mode = all(목록 전부 순서대로) / one(위에서부터 조건 맞는 하나만)
    # items: [{name, amount: 숫자 또는 "ALL", min_have: 최소 보유 개수}]
    "templates": {},
    "biomes_on": {},              # 팝핑 바이옴 설정 — 켜진 바이옴에서만 오토 팝핑 (기본 전부 켜짐)
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
MOVE_FEATS = ("mfish", "mpop", "mcraft")    # 장소를 따로 둘 수 있는 기능 (매크로 기준 위치 설정의 각 기능 칸)
# 기능마다 정해진 장소 (사용자가 만들거나 지우지 않음 · 지점 위치와 시간만 지정)
MOVE_TEMPLATES = {"mfish": (("fish_spot", "낚시 장소"), ("sell_spot", "물고기 판매 장소")), "mcraft": (("craft_spot", "포션 제작 장소"),)}
# 지점 수가 정해진 장소 (포션 제작 장소: 1번 퀘스트 보드 · 2번 스텔라 포탈)
MOVE_FIXED_POINTS = {("mcraft", "craft_spot"): 2}
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
    # 반대쪽 길: 1번 지점(퀘스트 보드) 뒤 E → q_wait 초 → 퀘스트 창 Exit → D qd_time 초 · 2번 지점(스텔라 포탈) 뒤 D rd_time 초 = 포션 제작 장소
    "q_wait": 1.5,
    "qd_time": 2.0,               # 퀘스트 창 Exit 다음 D 누르기 (초)
    "rd_time": 2.0,
    "quest_exit_pos": None,       # 퀘스트 창 Exit 버튼 [x, y]
    "w2_time": 0.25,              # A 를 누르는 마지막 이 시간 동안 W 도 같이 (초) — A 를 누른 지 a_time - w2_time 뒤에 시작
    "tilt_px": 800,               # 우클릭을 누른 채 마우스를 아래로 끄는 거리 (px) — 위에서 내려다보기
    "o_time": 2.5,                # O 누르기 (초) — 최대 줌
    "button": "right",            # 이동할 곳을 누를 마우스 버튼 (Click to Move)
    "margin": 0.3,                # 잰 시간에 더 기다릴 여유 (초)
    # 장소: [{"name", "feat": 쓰는 기능, "key": 정해진 장소 이름표, "points": [{"pos": [x, y] (기준 장소 화면에서 누를 곳), "time": 걸린 시간(초) 또는 None}]}]
    # 지점이 여러 개면 앞 지점에 도착한 화면에서 다음 지점을 누름 (멀리 갈 때)
    "places": [],
}
MPOP_DEFAULT = {
    # 매크로 탭 · 레어 바이옴 자동 팝핑 (내 서버) — 위치 · OCR 영역은 매크로 기준 위치(base), 딜레이 · 일치율은 오토 팝핑(pop) 설정을 같이 씀
    "enabled": False,             # 켜짐: 지금 켜져 있는 로블록스(내 서버)에서 레어 바이옴이 감지되면 포션 사용
    "start_delay": 1.0,           # 바이옴 감지 후 인벤토리를 열기까지 대기 (초)
    "close_inventory": True,      # 다 쓰고 Inventory 버튼을 한 번 더 눌러 닫기
    "templates": {},              # 바이옴별 포션 목록 (오토 팝핑과 따로)
    "biomes_on": {},              # 켜진 바이옴에서만 (기본 전부 켜짐)
}
MITEM_DEFAULT = {
    # 매크로 탭 · 오토 아이템 사용 — 쿨타임마다 인벤토리에서 아이템 사용 (스크립트 매크로와 같은 간격)
    # 위치 · OCR 영역은 매크로 기준 위치(base), 딜레이 · 일치율은 오토 팝핑(pop) 설정을 같이 씀
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
    "f_wait": 2.5,                # F 누른 뒤 제작 창이 뜰 때까지 (초)
    # 제작 창 위치 (자동 보정 · 직접 지정) — 실행 중엔 글자로 먼저 찾고, 못 찾을 때 이 값을 씀
    "search_pos": None,           # 검색창
    "list_region": None,          # 검색 결과 목록
    "shop_close_pos": None,       # 제작 창 X
    "open_recipe_pos": None,      # Open Recipe
    "add_all_pos": None,          # Add Everything
    "craft_pos": None,            # Craft
    "add_close_pos": None,        # Add Ingredients 창 X
}
# 상인 아이템 (스크립트 매크로의 상인 아이템 설정과 같은 목록)
MERCHANT_ITEMS = {
    "Jester": ("Oblivion Potion", "Heavenly Potion", "Potion of Bound", "Rune of Everything", "Random Potion Sack",
               "Stella's Candle", "Lucky Potion"),
    "Mari": ("Void Coin", "Lucky Penny", "Gear A", "Gear B", "Mixed Potion", "Speed Potion", "Lucky Potion", "Fortune Spoid I"),
}
MMERCH_DEFAULT = {
    # 매크로 탭 · 상인 자동 구매 — 채팅에 상인 도착이 뜨면 Merchant Teleporter 로 가서 고른 아이템 구매
    # 위치는 Noteab 매크로(Apache 2.0)의 1920x1080 값을 템플릿으로 씀 · 대화창 · 인벤토리는 통합 위치
    "enabled": False,
    "check_sec": 15.0,            # 채팅창 확인 간격 (초)
    "teleport_wait": 3.0,         # Merchant Teleporter 사용 후 대기 (초)
    "buy": {},                    # {"Mari_Void Coin": 1, ...} — 고른 아이템만 (개수는 Set to Max)
    "auto_cal": True,             # 상점이 열리면 Purchase 글자로 상점 위치를 자동 보정
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
}
POP_BIOMES = ("CYBERSPACE", "GLITCHED", "DREAMSPACE")
# 기본 템플릿 — 얼로니 SolsRNG 스크립트의 레어 바이옴 자동 팝핑(_RareBiomePotionTable) 그대로
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
POP_DELAYS = {"after_join": 7.5, "inventory": 0.5, "items": 0.5, "search": 0.5, "typing": 0.5,
              "item_click": 0.35, "dbl_gap": 0.1, "after_dbl": 0.5, "after_enter": 0.35}
DEFAULT_IGNORE = ["Ended", "End"]
REMOVED_KEYS = ("print_all_links", "run_on_link", "cooldown_sec", "kill_multiscope", "popping",
                "debug_port")                 # 디버그 포트는 이제 켤 때마다 무작위

_U = r"[^\s\"'<>()\[\]]"     # URL 에 들어갈 수 있는 글자
LINK_RE = re.compile(
    r"https?://(?:www\.)?roblox\.com/(?:"
    r"share\?" + _U + r"*code=" + _U + r"+"                                     # roblox.com/share?code=...
    r"|games/\d+" + _U + r"*privateServerLinkCode=" + _U + r"+)"                 # games/ID/...?privateServerLinkCode=...
    r"|roblox://" + _U + r"*(?:privateServerLinkCode|linkCode|code)=" + _U + r"+",  # roblox:// 딥링크
    re.I,
)
DISCORD_EPOCH = 1420070400000
NO_WINDOW = getattr(subprocess, "CREATE_NO_WINDOW", 0)


_CODE_RE = re.compile(r"(?:privateServerLinkCode|linkCode|code)=([A-Za-z0-9_\-]+)", re.I)


def link_codes(text):
    """링크 안의 비공개 서버 코드들 (share?code= / privateServerLinkCode= / linkCode=)"""
    return {c.lower() for c in _CODE_RE.findall(text or "")}


def own_codes(cfg):
    """내 브섭 링크(매크로 복귀 설정 · 바이옴 매크로 설정)의 코드들"""
    codes = set()
    for sec in ("ret", "biome"):
        try:
            codes |= link_codes((cfg[sec] or {}).get("ps_link") or "")
        except Exception:
            pass
    return codes


def snowflake_ms(i):
    try:
        return (int(i) >> 22) + DISCORD_EPOCH
    except (TypeError, ValueError):
        return 0


# ---------------------------------------------------------------- 창과 연결되는 콜백
class Bus:
    on_log = staticmethod(lambda msg, color="": print(time.strftime("%H:%M:%S"), msg, flush=True))
    on_status = staticmethod(lambda state, text="": None)
    on_event = staticmethod(lambda entry: None)
    on_joined = staticmethod(lambda url: None)      # 로블록스 실행 직후 (팝핑 시작용)


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
CONNECTED = threading.Event()          # 디스코드 앱에 붙어 있는 동안 켜짐
ARMED = threading.Event()              # 시작 버튼을 눌러야 켜짐 — 꺼져 있으면 감지·기록만 하고 아무것도 안 함


# ---------------------------------------------------------------- 이름 조회 (다른 스레드 → 연결 루프에 부탁)
import queue as _queue

_lookup_q = _queue.Queue()


class _LookupReq:
    def __init__(self, ids):
        self.ids, self.event, self.result = list(ids), threading.Event(), None


def lookup_names(ids, timeout=4.0):
    """디스코드 앱에서 ID 의 이름을 가져옴. 연결 안 돼 있으면 None"""
    if not ids or not CONNECTED.is_set():
        return None
    req = _LookupReq(ids)
    _lookup_q.put(req)
    req.event.wait(timeout)
    return req.result


# ---------------------------------------------------------------- 설정
def normalize(raw):
    d = dict(DEFAULT_CONFIG)
    d.update(raw or {})
    if "print_all_links" in d and "show_all_links" not in (raw or {}):
        d["show_all_links"] = d.pop("print_all_links")
    for k in REMOVED_KEYS:
        d.pop(k, None)
    for k in ("guild_ids", "channel_ids"):
        d[k] = list(dict.fromkeys(str(x).strip() for x in d.get(k) or [] if str(x).strip()))
    for k in ("keywords", "ignore_words"):
        d[k] = list(dict.fromkeys(str(x).strip() for x in d.get(k) or [] if str(x).strip()))
    have = {w.lower() for w in d["ignore_words"]}
    d["ignore_words"] += [w for w in DEFAULT_IGNORE if w.lower() not in have]   # 기본 제외 단어는 항상 포함
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
    play.pop("pre_steps", None)              # 접속 전 동작 없앰 (스나이핑 중엔 매크로가 알아서 쉼)
    d["play"] = play
    pop = dict(POP_DEFAULT)
    pop.update(d.get("pop") if isinstance(d.get("pop"), dict) else {})

    def _ratio_list(v, n):
        try:
            v = [min(1.0, max(0.0, float(x))) for x in v][:n]
            return v if len(v) == n else None
        except Exception:
            return None
    for k in ("inventory_pos", "items_pos", "search_pos", "item_pos", "amount_pos", "use_pos"):
        pop[k] = _ratio_list(pop.get(k), 2)
    pop["ocr_region"] = _ratio_list(pop.get("ocr_region"), 4)
    try:
        th = float(pop.get("match_threshold", 70))
        if not pop.get("th70") and th == 50:      # 예전 기본값(50%) 그대로면 새 기본값 70% 로 한 번 바꿈
            th = 70
        pop["match_threshold"] = min(100.0, max(1.0, th))
    except (TypeError, ValueError):
        pop["match_threshold"] = 70
    pop["th70"] = True
    dl = dict(POP_DELAYS)
    for k, v in (pop.get("delays") or {}).items():
        if k in dl:
            try:
                dl[k] = max(0.0, float(v))
            except (TypeError, ValueError):
                pass
    pop["delays"] = dl
    pop["templates"], pop["biomes_on"] = _norm_templates(pop)
    pop["seeded"] = True                    # 기본 템플릿은 한 번만 (지운 건 다시 안 채움)
    pop["use_base"] = bool(pop.get("use_base"))   # 스나이프 오토 팝핑도 매크로 기준 위치를 씀
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
    if not mv.get("v18") and mv.get("reset_wait") == 2.6:      # 예전(FishSol 방식) 기본값이면 새 기본값으로 한 번
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
                            ("a_time", 0, 30, float), ("w2_time", 0, 30, float), ("rsd_time", 0, 30, float),
                            ("rs_time", 0, 60, float), ("ra_time", 0, 30, float), ("q_wait", 0, 15, float), ("qd_time", 0, 30, float), ("rd_time", 0, 30, float),
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
        pts = []
        for pt in (pl.get("points") if isinstance(pl.get("points"), list) else [])[:20]:
            if not isinstance(pt, dict):
                continue
            try:
                t = round(min(600.0, max(0.0, float(pt["time"]))), 2) if pt.get("time") is not None else None
            except (TypeError, ValueError):
                t = None
            pts.append({"pos": _ratio_list(pt.get("pos"), 2), "time": t})
        feat = pl.get("feat") if pl.get("feat") in MOVE_FEATS else "mfish"     # 그 장소를 쓰는 기능 (예전 장소는 자동 낚시)
        places.append({"name": str(pl.get("name") or "")[:40], "feat": feat, "key": str(pl.get("key") or ""),
                       "points": pts or [{"pos": None, "time": None}]})
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
            fixed = MOVE_FIXED_POINTS.get((pl["feat"], pl["key"]))
            if fixed:                                              # 지점 수를 딱 맞춤 (남는 건 버리고 모자라면 빈 지점)
                pts = out[-1]["points"][:fixed]
                out[-1]["points"] = pts + [{"pos": None, "time": None} for _ in range(fixed - len(pts))]
    for feat, tpls in MOVE_TEMPLATES.items():
        for key, name in tpls:
            if (feat, key) not in seen:
                n = MOVE_FIXED_POINTS.get((feat, key), 1)
                out.append({"name": name, "feat": feat, "key": key, "points": [{"pos": None, "time": None} for _ in range(n)]})
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
    d["mmerch"] = {k: mm[k] for k in MMERCH_DEFAULT}
    d["mmerch"]["v129"] = True
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
    try:
        mc["f_wait"] = min(15.0, max(0.5, float(mc.get("f_wait"))))
    except (TypeError, ValueError):
        mc["f_wait"] = MCRAFT_DEFAULT["f_wait"]
    for k in ("search_pos", "shop_close_pos", "open_recipe_pos", "add_all_pos", "craft_pos", "add_close_pos"):
        mc[k] = _ratio_list(mc.get(k), 2)
    mc["list_region"] = _ratio_list(mc.get("list_region"), 4)
    d["mcraft"] = {k: mc[k] for k in MCRAFT_DEFAULT}
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
    d["mfish"] = {k: mf[k] for k in MFISH_DEFAULT}
    if d.get("ocr_engine") not in ("auto", "rapid", "windows"):
        d["ocr_engine"] = "auto"
    d["open_link"] = True                   # '실제 접속' 토글 없앰: 감지되면 항상 접속
    d["online_share"] = bool(d.get("online_share", True))
    if not re.fullmatch(r"[0-9a-f]{32}", str(d.get("install_id") or "")):
        d["install_id"] = secrets.token_hex(16)
    ret = d.get("ret") if isinstance(d.get("ret"), dict) else {}
    try:
        start_wait = max(0.0, float(ret.get("start_wait", 7.5)))
    except (TypeError, ValueError):
        start_wait = 7.5
    # start_wait: 복귀 후 Play 로 게임 입장이 감지된 뒤, 매크로를 다시 켜기까지 대기 (초) · 복귀 후 동작은 없앰
    d["ret"] = {"ps_link": str(ret.get("ps_link") or "").strip(), "start_wait": start_wait}
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
    sn = dict(SNIPE_DEFAULT)
    sn.update(d.get("snipe") if isinstance(d.get("snipe"), dict) else {})

    def _num(v, lo, hi, default):
        try:
            return min(hi, max(lo, float(v)))
        except (TypeError, ValueError):
            return default
    sn["delay_min"] = _num(sn["delay_min"], 0, 60, SNIPE_DEFAULT["delay_min"])
    sn["delay_max"] = max(sn["delay_min"], _num(sn["delay_max"], 0, 120, SNIPE_DEFAULT["delay_max"]))
    sn["direct"] = bool(sn["direct"])
    sn["same_link_min"] = _num(sn["same_link_min"], 0, 1440, SNIPE_DEFAULT["same_link_min"])
    sn["cooldown_sec"] = _num(sn["cooldown_sec"], 0, 600, SNIPE_DEFAULT["cooldown_sec"])
    d["snipe"] = {k: sn[k] for k in SNIPE_DEFAULT}
    d["names"] = dict(d.get("names") or {})
    if d.get("menu_tab") not in ("biome", "snipe", "macro", "acrux"):
        d["menu_tab"] = "snipe"
    d["tutorials_done"] = list(dict.fromkeys(str(x) for x in d.get("tutorials_done") or []))
    d["channels"] = {str(k): dict(v) for k, v in (d.get("channels") or {}).items() if isinstance(v, dict)}
    for k in ("max_age_sec",):
        try:
            d[k] = int(d[k])
        except (TypeError, ValueError):
            d[k] = DEFAULT_CONFIG[k]
    return d


def _norm_templates(src_cfg):
    """바이옴별 포션 템플릿 · 켜진 바이옴 정리 (오토 팝핑 · 매크로 탭 팝핑 공용)
    처음 한 번(seeded 전)은 비어 있는 템플릿을 기본 템플릿으로 채움"""
    tpls = {}
    src = src_cfg.get("templates") or {}
    for b in POP_BIOMES:
        t = src.get(b) or {}
        if not src_cfg.get("seeded") and not (t.get("items") or []):
            t = json.loads(json.dumps(POP_TEMPLATES[b]))      # 처음 한 번: 기본 템플릿으로 채움
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
    return tpls, {b: bool(on.get(b, True)) for b in POP_BIOMES}


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


# ---------------------------------------------------------------- 디스코드 실행/연결
FLAVORS = {
    "stable": ("Discord", "Discord.exe"),
    "ptb": ("DiscordPTB", "DiscordPTB.exe"),
    "canary": ("DiscordCanary", "DiscordCanary.exe"),
}


def discord_paths(flavor):
    folder, exe = FLAVORS.get(flavor, FLAVORS["stable"])
    root = Path(os.environ.get("LOCALAPPDATA", "")) / folder
    apps = sorted(root.glob("app-*"),
                  key=lambda p: [int(x) for x in re.findall(r"\d+", p.name)], reverse=True)
    exe_path = next((a / exe for a in apps if (a / exe).exists()), None)
    return root, exe, exe_path


def is_running(exe_name):
    return procs.running(exe_name)


# 디버그 포트 조회는 이 PC 안(127.0.0.1)으로만 — 윈도우 프록시 설정이 있어도 프록시를 거치지 않게
_LOCAL = urllib.request.build_opener(urllib.request.ProxyHandler({}))


def http_json(port, path, timeout=2):
    with _LOCAL.open(f"http://127.0.0.1:{port}{path}", timeout=timeout) as r:
        return json.loads(r.read().decode("utf-8"))


def port_alive(port):
    """그 포트에서 디스코드(크로미움) 디버그 서버가 응답하는지"""
    try:
        v = http_json(port, "/json/version", timeout=1)
        return isinstance(v, dict) and "Browser" in v
    except Exception:
        return False


def port_busy(port):
    """다른 프로그램이 이미 쓰고 있는 포트인지 (그러면 디스코드가 거기에 디버그 서버를 못 엶)"""
    for host in ("127.0.0.1", "0.0.0.0"):
        s = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
        try:
            if os.name == "nt":
                s.setsockopt(socket.SOL_SOCKET, getattr(socket, "SO_EXCLUSIVEADDRUSE", -5), 1)
            s.bind((host, port))
        except OSError:
            return True
        finally:
            s.close()
    return False


# 디버그 포트는 켤 때마다 무작위 (고정 포트면 다른 프로그램이 찾기 쉬움)
PORT_RANGE = (20000, 60999)


def random_port():
    rnd = secrets.SystemRandom()
    for _ in range(60):
        p = rnd.randint(*PORT_RANGE)
        if not port_busy(p):
            return p
    s = socket.socket()
    s.bind(("127.0.0.1", 0))
    p = s.getsockname()[1]
    s.close()
    return p


# ---------------------------------------------------------------- 윈도우: 프로세스 · 포트 정보 (윈도우 언어와 상관없이 동작)
_W = {}


def _win():
    """kernel32 / ntdll / iphlpapi 함수 준비 (처음 한 번)"""
    if _W:
        return _W
    import ctypes
    from ctypes import wintypes
    k32 = ctypes.WinDLL("kernel32", use_last_error=True)
    nt = ctypes.WinDLL("ntdll")
    ip = ctypes.WinDLL("iphlpapi")
    k32.OpenProcess.argtypes = (wintypes.DWORD, wintypes.BOOL, wintypes.DWORD)
    k32.OpenProcess.restype = wintypes.HANDLE
    k32.CloseHandle.argtypes = (wintypes.HANDLE,)
    k32.QueryFullProcessImageNameW.argtypes = (wintypes.HANDLE, wintypes.DWORD, wintypes.LPWSTR,
                                               ctypes.POINTER(wintypes.DWORD))
    nt.NtQueryInformationProcess.argtypes = (wintypes.HANDLE, wintypes.ULONG, ctypes.c_void_p, wintypes.ULONG,
                                             ctypes.POINTER(wintypes.ULONG))
    nt.NtQueryInformationProcess.restype = ctypes.c_long
    ip.GetExtendedTcpTable.argtypes = (ctypes.c_void_p, ctypes.POINTER(wintypes.DWORD), wintypes.BOOL,
                                       wintypes.ULONG, ctypes.c_int, wintypes.ULONG)
    ip.GetExtendedTcpTable.restype = wintypes.DWORD
    _W.update(ctypes=ctypes, wt=wintypes, k32=k32, nt=nt, ip=ip)
    return _W


def _pids(exe_name):
    """그 이름으로 실행 중인 프로세스 PID 목록"""
    return procs.pids(exe_name)


def _pid_path(pid):
    """프로세스 실행 파일 전체 경로 (모르면 빈 글자)"""
    if os.name != "nt":
        return ""
    try:
        w = _win()
        h = w["k32"].OpenProcess(0x1000, False, pid)          # PROCESS_QUERY_LIMITED_INFORMATION
        if not h:
            return ""
        try:
            buf = w["ctypes"].create_unicode_buffer(1024)
            n = w["wt"].DWORD(1024)
            if w["k32"].QueryFullProcessImageNameW(h, 0, buf, w["ctypes"].byref(n)):
                return buf.value
        finally:
            w["k32"].CloseHandle(h)
    except Exception:
        pass
    return ""


def _cmdline(pid):
    """프로세스 실행 옵션 (모르면 빈 글자)"""
    if os.name != "nt":
        return ""
    try:
        w = _win()
        ct = w["ctypes"]
        h = w["k32"].OpenProcess(0x1000, False, pid)
        if not h:
            return ""
        try:
            size, need = 8192, w["wt"].ULONG(0)
            for _ in range(3):
                buf = ct.create_string_buffer(size)
                st = w["nt"].NtQueryInformationProcess(h, 60, buf, size, ct.byref(need))   # ProcessCommandLineInformation
                if st == 0:
                    break
                if need.value <= size:
                    return ""
                size = need.value + 64
            else:
                return ""

            class _US(ct.Structure):
                _fields_ = [("Length", ct.c_ushort), ("MaximumLength", ct.c_ushort), ("Buffer", ct.c_void_p)]
            us = _US.from_buffer(buf)
            return ct.wstring_at(us.Buffer, us.Length // 2) if us.Buffer and us.Length else ""
        finally:
            w["k32"].CloseHandle(h)
    except Exception:
        return ""


def _parse_tcp_table(raw, af):
    """GetExtendedTcpTable(TCP_TABLE_OWNER_PID_LISTENER) 결과 → [(주소, 포트, PID)]"""
    out = []
    n = struct.unpack_from("<I", raw, 0)[0]
    size = 24 if af == socket.AF_INET else 56
    n = min(n, (len(raw) - 4) // size)
    for i in range(n):
        off = 4 + i * size
        if af == socket.AF_INET:
            _state, laddr, lport, _ra, _rp, pid = struct.unpack_from("<6I", raw, off)
            addr = socket.inet_ntoa(struct.pack("<I", laddr))
        else:
            addr = socket.inet_ntop(socket.AF_INET6, bytes(raw[off:off + 16]))
            lport = struct.unpack_from("<I", raw, off + 20)[0]
            pid = struct.unpack_from("<I", raw, off + 52)[0]
        port = ((lport & 0xFF) << 8) | ((lport >> 8) & 0xFF)       # 네트워크 바이트 순서 → 숫자
        out.append((addr, port, pid))
    return out


def tcp_listeners():
    """지금 연결을 기다리는 TCP 포트 전부 [(주소, 포트, PID)] (IPv4 + IPv6)"""
    if os.name != "nt":
        return []
    w = _win()
    out = []
    for af in (socket.AF_INET, socket.AF_INET6):
        win_af = 2 if af == socket.AF_INET else 23
        size = w["wt"].DWORD(0)
        w["ip"].GetExtendedTcpTable(None, w["ctypes"].byref(size), False, win_af, 3, 0)   # 3 = OWNER_PID_LISTENER
        for _ in range(3):
            buf = w["ctypes"].create_string_buffer(max(size.value, 4) + 4096)
            size = w["wt"].DWORD(len(buf))
            r = w["ip"].GetExtendedTcpTable(buf, w["ctypes"].byref(size), False, win_af, 3, 0)
            if r == 0:
                out += _parse_tcp_table(buf.raw, af)
                break
            if r != 122:                     # 122 = 버퍼가 작음 → 다시
                break
    return out


def _is_loopback(addr):
    return addr.startswith("127.") or addr == "::1"


def _real(p):
    try:
        return os.path.normcase(os.path.realpath(str(p)))
    except Exception:
        return str(p).lower()


def discord_procs(root, exe_name):
    """실행 중인 진짜 디스코드 프로세스 [(PID, 실행 옵션)] — 디스코드 설치 폴더 안의 exe 만 (메인 프로세스 먼저)"""
    base = _real(root).rstrip("\\/") + os.sep
    out = []
    for pid in _pids(exe_name):
        path = _pid_path(pid)
        if path and not _real(path).startswith(base):
            continue                         # 이름만 같은 다른 프로그램
        out.append((pid, _cmdline(pid)))
    out.sort(key=lambda x: "--type=" in x[1])
    return out


def debug_port_check(port, root, exe_name):
    """디버그 포트가 '디스코드'가 '이 PC 안(127.0.0.1)에서만' 연 것인지 → (True, "") / (False, 이유)"""
    if os.name != "nt":
        return True, ""
    try:
        rows = [(a, pid) for a, p, pid in tcp_listeners() if p == port]
    except Exception:
        return True, ""                      # 확인 기능 자체가 안 되는 PC (드묾) — 막지는 않음
    ok_pids = {pid for pid, _ in discord_procs(root, exe_name)}
    if not rows or not ok_pids:
        # 포트는 응답하는데 표에 없음 / 디스코드 프로세스를 못 찾음 = 이 PC 에선 확인 기능이 안 됨 → 막지 않음
        # (잘못 막으면 디스코드를 계속 다시 켜게 되므로)
        return True, ""
    other = [pid for _, pid in rows if pid not in ok_pids]
    if other:
        p = _pid_path(other[0])
        return False, f"디버그 포트 {port} 를 디스코드가 아닌 프로그램이 쓰고 있음 ({os.path.basename(p) or 'PID ' + str(other[0])})"
    if any(not _is_loopback(a) for a, _ in rows):
        return False, f"디버그 포트 {port} 가 이 PC 밖(네트워크)에도 열려 있음"
    return True, ""


ACTIVE_PORT = {"port": None}     # 이번에 연결한 디스코드 디버그 포트
SAFETY = {"fails": 0}            # 디버그 포트 안전 확인 연속 실패 횟수 (2번이면 디스코드 재시작을 멈춤)
UNSAFE_FLAGS = re.compile(r"--remote-(allow-origins|debugging-address)", re.I)
_WARNED = set()


class NeedManualRestart(Exception):
    """사용자가 디스코드를 직접 다시 켜야 함 (자동 재시작 꺼짐)"""


def existing_port(cfg):
    """이미 디버그 포트로 켜져 있는 디스코드의 포트 (안전하게 열린 경우만). 없거나 위험하면 None → 다시 켬"""
    root, exe_name, _ = discord_paths(cfg["discord"])
    for pid, cl in discord_procs(root, exe_name):
        m = re.search(r"--remote-debugging-port[= ]\"?(\d+)", cl)
        if not m:
            continue
        port = int(m.group(1))
        if UNSAFE_FLAGS.search(cl):
            if not cfg["auto_restart_discord"]:
                if "unsafe" not in _WARNED:
                    _WARNED.add("unsafe")
                    log("디스코드가 예전 방식(웹페이지도 접속 가능)으로 켜져 있음 — 디스코드를 껐다가 다시 켜주세요 (자동 재시작 꺼짐)", "r")
                status("error", "디코 재시작 필요")
                raise NeedManualRestart()
            log("디스코드가 예전 방식(웹페이지도 접속 가능)으로 켜져 있음 → 안전한 방식으로 다시 켬", "y")
            return None
        ok, why = debug_port_check(port, root, exe_name)
        if not ok:
            SAFETY["fails"] += 1
            log(why + " → 디스코드를 다시 켬", "y")
            return None
        if port_alive(port):
            ACTIVE_PORT["port"] = port
            SAFETY["fails"] = 0
            return port
        return None
    # 실행 옵션을 못 읽는 경우: 이번에 켠 포트가 아직 살아 있으면 그대로
    port = ACTIVE_PORT["port"]
    if port and port_alive(port) and debug_port_check(port, root, exe_name)[0]:
        return port
    return None


def launch_discord(cfg):
    root, exe_name, exe_path = discord_paths(cfg["discord"])
    if STOP.is_set():
        return False
    if SAFETY["fails"] >= 2:
        log("디버그 포트 안전 확인이 계속 실패해서 디스코드를 다시 켜지 않음 — 디스코드와 Acrux 를 껐다가 다시 켜주세요", "r")
        status("error", "포트 확인 실패")
        STOP.wait(30)
        return False

    if not exe_path:
        log(f"디스코드를 찾을 수 없음: {root}  (기타 설정에서 디스코드 종류 확인 필요)", "r")
        status("error", "디스코드 없음")
        return False

    if is_running(exe_name):
        if not cfg["auto_restart_discord"]:
            log("디스코드가 디버그 포트 없이 실행 중 — 디스코드 종료 또는 '자동 재시작' 설정 필요", "y")
            status("error", "디코 재시작 필요")
            return False
        status("launching", "디스코드 재시작 중")
        log("디스코드를 디버그 포트와 함께 재시작하는 중…", "y")
        _kill(exe_name)
        if STOP.is_set():
            return False
    else:
        status("launching", "디스코드 실행 중")
        log("디스코드 실행 중…", "y")

    # 포트는 켤 때마다 무작위 · 이 PC 안에서만 · 웹페이지는 접속 불가 (--remote-allow-origins 안 씀)
    port = random_port()
    ACTIVE_PORT["port"] = port
    ACTIVE_PORT["ours"] = True            # Acrux 가 켠 디스코드 → 끌 때 보통 모드로 되돌림
    args = [f"--remote-debugging-port={port}"]

    # 1) Update.exe 를 거치지 않고 직접 실행 (업데이트 설치로 BetterDiscord 가 지워지는 것 방지)
    subprocess.Popen([str(exe_path)] + args, cwd=str(exe_path.parent),
                     creationflags=getattr(subprocess, "DETACHED_PROCESS", 0),
                     stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    if _wait_port(port, 30):
        return _port_safe(port, root, exe_name)
    if STOP.is_set():
        return False

    # 2) 안 열리면 (디스코드가 업데이트하면서 옵션 없이 다시 켜진 경우 등)
    #    디스코드를 끄고 디스코드 업데이터(Update.exe)로 옵션을 붙여서 다시 실행
    updater = root / "Update.exe"
    if updater.exists():
        log("직접 실행으로는 디버그 포트가 안 열림 → 디스코드 업데이터로 다시 실행", "y")
        status("launching", "디스코드 재시작 중")
        _kill(exe_name)
        if STOP.is_set():
            return False
        subprocess.Popen([str(updater), "--processStart", exe_name, "--process-start-args", " ".join(args)],
                         cwd=str(root), creationflags=getattr(subprocess, "DETACHED_PROCESS", 0),
                         stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        if _wait_port(port, 90, 30):
            return _port_safe(port, root, exe_name)
    if STOP.is_set():
        return False
    log("디버그 포트 열기 실패 — 디스코드가 이 옵션을 막았을 가능성 있음", "r")
    status("error", "포트 안 열림")
    return False


def _port_safe(port, root, exe_name):
    ok, why = debug_port_check(port, root, exe_name)
    if ok:
        SAFETY["fails"] = 0
    else:
        SAFETY["fails"] += 1
        log(why + " — 연결 안 함", "r")
        status("error", "포트 확인 실패")
    return ok


def close_debug_port(cfg):
    """Acrux 를 끌 때: 디버그 포트로 켜 둔 디스코드를 보통 모드로 다시 켬 (포트가 계속 열려 있지 않게)
    Acrux 가 디스코드를 켰거나 '자동 재시작'이 켜져 있고(= Acrux 가 디스코드를 관리), 이번에 쓴 포트로 켜져 있을 때만"""
    port = ACTIVE_PORT["port"]
    if os.name != "nt" or not port:
        return
    try:
        if not (cfg["auto_restart_discord"] or ACTIVE_PORT.get("ours")):
            return                           # 사용자가 직접 디버그 포트로 켠 디스코드는 건드리지 않음
        root, exe_name, exe_path = discord_paths(cfg["discord"])
        procs = discord_procs(root, exe_name)
        if not procs:
            return
        if any(cl for _, cl in procs):           # 실행 옵션을 읽을 수 있으면: 이번 포트로 켜져 있을 때만
            if not any(re.search(rf"--remote-debugging-port[= ]\"?{port}(?!\d)", cl) for _, cl in procs):
                return
        elif not (port_alive(port) and debug_port_check(port, root, exe_name)[0]):
            return
        log("디스코드를 보통 모드로 다시 켜는 중 (디버그 포트 닫기)", "d")
        taskkill([exe_name])
        for _ in range(20):
            if not is_running(exe_name):
                break
            time.sleep(0.5)
        updater = root / "Update.exe"
        if updater.exists():
            cmd, cwd = [str(updater), "--processStart", exe_name], root
        elif exe_path:
            cmd, cwd = [str(exe_path)], exe_path.parent
        else:
            return
        subprocess.Popen(cmd, cwd=str(cwd), creationflags=getattr(subprocess, "DETACHED_PROCESS", 0),
                         stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, close_fds=True)
        ACTIVE_PORT["port"] = None
    except Exception as e:
        log(f"디스코드 보통 모드 재시작 실패: {e}", "r")


def _kill(exe_name):
    taskkill([exe_name])
    for _ in range(20):
        if not is_running(exe_name) or STOP.wait(0.5):
            break
    STOP.wait(1)


def _wait_port(port, secs, start=0):
    """디버그 포트가 열릴 때까지 기다림 — 멈춘 것처럼 안 보이게 15초마다 로그"""
    for n in range(1, secs + 1):
        if port_alive(port):
            return True
        if STOP.wait(1):
            return False
        if n % 15 == 0 and n < secs:
            log(f"디스코드 응답 대기 중 ({start + n}초)…", "d")
    return port_alive(port)


MAIN_PATH = re.compile(r"https://(ptb\.|canary\.)?discord\.com/(channels|app|login|activity|store|library|shop|guild-discovery|discovery|quest)", re.I)


def find_target(port):
    shown = False
    for _ in range(60):
        try:
            pages = [t for t in http_json(port, "/json/list")
                     if t.get("type") == "page" and t.get("webSocketDebuggerUrl")]
            main = [t for t in pages if MAIN_PATH.match(t.get("url", ""))]
            if main:
                return main[0]
            if not shown and pages:
                shown = True
                log("메인 창 대기 중… 현재 창: " + ", ".join(t.get("url", "")[:60] for t in pages), "d")
        except Exception:
            pass
        if STOP.wait(1):
            return None
    return None


def ws_url(port, target):
    """연결 주소는 받은 값을 그대로 쓰지 않고 직접 만듦 (항상 이 PC 의 그 포트, 그 창으로만)"""
    tid = str(target.get("id") or "")
    if not re.fullmatch(r"[A-Za-z0-9-]{8,64}", tid):
        raise ValueError("디스코드 창 ID 가 이상함")
    return f"ws://127.0.0.1:{int(port)}/devtools/page/{tid}"


class CDP:
    def __init__(self, url):
        # Origin 헤더 없이 연결 (디스코드를 --remote-allow-origins 없이 켜도 연결되고, 웹페이지는 접속 못 함)
        self.ws = websocket.create_connection(url, timeout=10, suppress_origin=True,
                                              http_no_proxy=["127.0.0.1", "localhost"])
        self.n = 0

    def send(self, method, params=None):
        self.n += 1
        self.ws.send(json.dumps({"id": self.n, "method": method, "params": params or {}}))
        return self.n

    def recv(self, timeout=None):
        return json.loads(self.recv_raw(timeout))

    def recv_raw(self, timeout=None):
        if timeout != getattr(self, "_to", object()):
            self.ws.settimeout(timeout)
            self._to = timeout
        return self.ws.recv()

    def close(self):
        try:
            self.ws.close()
        except Exception:
            pass


# ---------------------------------------------------------------- 링크 처리
def has_word(text, word):
    """단어 단위 포함 여부 (대소문자 구분 X). 'End' 는 'End', 'end!' 는 잡고 'Legend', 'Endless' 는 안 잡음"""
    return re.search(r"(?<![0-9a-z])" + re.escape(word.lower()) + r"(?![0-9a-z])", text) is not None


# ---------------------------------------------------------------- 딥링크 변환 + 실행
# (roblox_link_catcher_service.py 의 변환 방식 — Made By RN_Gelidness [ 얼로니 ])
import urllib.parse as _up

_CODE = r"([A-Za-z0-9_-]{1,200})"


def parse_link(url):
    """비공개 서버 링크 → ("private", placeId, linkCode) / ("share", code) / None
    디스코드 메시지(누구나 쓸 수 있는 글)에서 온 주소라 그대로 실행하지 않고,
    필요한 값(숫자 ID · 코드)만 뽑아서 새로 만든 링크만 실행함"""
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
JOIN_DELAY_SEC = 0.5


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


def _join(url, stable, snipe=None, manual=False):
    try:
        _join_inner(url, stable, snipe or SNIPE_DEFAULT, manual)
    finally:
        JOIN["pending"] = False


def _join_inner(url, stable, snipe, manual):
    deep, web = to_deeplink(url), to_weblink(url)
    if not deep:
        log("비공개 서버 링크 형식이 아니라서 열지 않음", "y")
        return
    if not manual:
        # 사람처럼: 링크를 보고 누르기까지 잠깐 (설정한 범위 안에서 매번 다르게)
        lo, hi = float(snipe.get("delay_min", 0)), float(snipe.get("delay_max", 0))
        wait = secrets.SystemRandom().uniform(lo, max(lo, hi))
        if wait > 0:
            log(f"{wait:.1f}초 뒤 접속 (사람처럼 잠깐 대기)", "d")
            end = time.time() + wait
            while time.time() < end:
                if STOP.is_set() or not ARMED.is_set():
                    log("접속 취소 — 작동 중지됨", "y")
                    return
                time.sleep(0.05)
    JOIN["last"] = time.time()
    for c in link_codes(url):
        JOIN["recent"][c] = JOIN["last"]
    if len(JOIN["recent"]) > 500:
        JOIN["recent"] = dict(sorted(JOIN["recent"].items(), key=lambda x: x[1])[-200:])
    expect_roblox_close()                       # 다른 서버로 옮겨가는 중 — 로블록스가 잠깐 꺼지는 건 정상
    if stable:
        # 안정화 접속: 로블록스 클라이언트 전부 종료 → 0.5초 후 실행
        if taskkill(ROBLOX_PROCESSES):
            log("로블록스 클라이언트 강제 종료", "d")
        time.sleep(JOIN_DELAY_SEC)
    if snipe.get("direct", True):
        try:
            os.startfile(deep)
            log(f"딥링크 실행{' (안정화)' if stable else ''} → {deep}", "g")
            _joined(url)
            return
        except Exception as e:
            log(f"딥링크 실행 실패 ({e}) → 브라우저로 열기", "r")
    try:
        os.startfile(web)                        # 기본 웹브라우저로 링크 열기 → 로블록스 사이트에서 접속
        if not snipe.get("direct", True):
            log(f"웹브라우저로 링크 열기{' (안정화)' if stable else ''} → {web}", "g")
        _joined(url)
    except Exception as e:
        log(f"링크 열기 실패: {e}", "r")


def _joined(url):
    try:
        Bus.on_joined(url)
    except Exception as e:
        log(f"접속 후 처리 오류: {e}", "r")


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


JOIN = {"pending": False, "last": 0.0, "recent": {}}   # 접속 대기 중 · 마지막 접속 시각 · 최근 접속한 서버 코드 → 시각


def join_server(url, stable=True, snipe=None, manual=False):
    """stable: 로블록스 전부 종료 → 0.5초 후 실행 / 아니면 즉시 실행 (별도 스레드라 감지는 안 멈춤)
    snipe: 스나이핑 안정성 설정 (접속 전 무작위 대기 · 링크 여는 방식) / manual: 직접 [열기] 를 누른 경우 (대기 없음)"""
    JOIN["pending"] = True
    threading.Thread(target=_join, args=(url, stable, snipe, manual), daemon=True).start()


def snipe_skip(url, snipe):
    """이 링크를 지금 타면 안 되는 이유 (없으면 빈 글자)"""
    now = time.time()
    if JOIN["pending"]:
        return "건너뜀"                         # 다른 링크로 접속하는 중
    if now - JOIN["last"] < float(snipe.get("cooldown_sec", 0)):
        return "건너뜀"                         # 방금 접속함 (연속 접속 최소 간격)
    win = float(snipe.get("same_link_min", 0)) * 60
    if win > 0:
        for c in link_codes(url):
            if now - JOIN["recent"].get(c, 0) < win:
                return "중복"                   # 같은 서버에 최근에 들어감
    return ""


def beep():
    try:
        import winsound
        winsound.MessageBeep(winsound.MB_ICONASTERISK)
    except Exception:
        pass


class Handler:
    def __init__(self, cfg):
        self.cfg = cfg
        self.done = deque(maxlen=500)
        self.skew = None           # 이 PC 시계 - 디스코드 시계 (ms). 시계가 틀려도 '옛 메시지' 판단·속도 표시가 맞게
        self.skew_warned = False
        self.last_fire = 0.0
        self.count = 0

    @staticmethod
    def where(d):
        g = d.get("guildName") or d.get("guildId") or "DM"
        ch = d.get("channelName") or d.get("channelId")
        return f"{g} #{ch}"

    def handle(self, d):
        cfg = self.cfg
        mid = d.get("id")
        blob = d.get("blob") or ""
        m = LINK_RE.search(blob)
        if not mid or not m:
            return
        now = time.time()
        if mid in self.done:
            return                 # 같은 메시지를 다른 방식이 나중에 잡음 → 무시
        url = m.group(0)
        # 내 브섭 링크면 (내 서버에 레어 바이옴이 뜬 것) 아무것도 안 하고 넘어감
        mine = own_codes(cfg)
        if mine and link_codes(url) & mine:
            self.done.append(mid)
            return
        raw_age = now * 1000 - snowflake_ms(mid)
        # PC 시계 오차: 통신으로 실시간 받은 새 메시지 중 가장 작은 값 = 시계 차이 (+ 최소 전달 시간)
        # (화면 방식은 채널을 열 때 옛 메시지도 다시 보여서 기준으로 안 씀)
        if d.get("via") == "gateway" and d.get("event") == "MESSAGE_CREATE" and (self.skew is None or raw_age < self.skew):
            self.skew = raw_age
        skew = self.skew if self.skew is not None else 0.0
        if abs(skew) > 3000 and not self.skew_warned:
            self.skew_warned = True
            log(f"PC 시계가 디스코드와 {abs(skew) / 1000:.0f}초 차이 남 ({'느림' if skew < 0 else '빠름'}) — "
                "윈도우 설정 → 시간 → '지금 동기화' 권장 (감지는 보정해서 정상 작동)", "y")
        age = max(0.0, raw_age - skew)
        if raw_age > max(1, cfg["max_age_sec"]) * 1000 + max(0.0, skew):
            return  # 옛 메시지 (시계 차이만큼 보정)

        gids, cids = cfg["guild_ids"], cfg["channel_ids"]
        # 감시 대상을 등록해야만 감지 (비어 있으면 아무것도 안 잡음)
        if d.get("thread"):
            return  # 스레드 / 포럼 글은 감지 안 함
        targeted = d.get("channelId") in cids or \
            (d.get("guildId") and d["guildId"] in gids)

        self.done.append(mid)
        text = blob.lower()
        entry = {
            "time": time.time(), "url": url, "via": d.get("via"), "age": age / 1000,
            "guildId": d.get("guildId"), "channelId": d.get("channelId"), "parentId": d.get("parentId"),
            "guildName": d.get("guildName") or "", "channelName": d.get("channelName") or "",
            "author": d.get("author") or "", "where": self.where(d), "status": ""
        }

        if not targeted:
            if not cfg["show_all_links"]:
                return
            entry["status"] = "보임"
        elif cfg["keywords"] and not any(k.lower() in text for k in cfg["keywords"]):
            entry["status"] = "바이옴 불일치"
        elif any(has_word(text, w) for w in cfg["ignore_words"]):
            entry["status"] = "제외됨"
        elif not cfg["enabled"]:
            entry["status"] = "꺼짐"
        elif not ARMED.is_set():
            entry["status"] = "대기"          # 연결·감지는 되지만 시작 전이라 작동 안 함
        elif snipe_skip(url, cfg["snipe"]):
            entry["status"] = snipe_skip(url, cfg["snipe"])   # 접속 중 · 방금 접속 · 같은 서버 → 안 탐
        else:
            self.last_fire = time.time()
            self.count += 1
            entry["status"] = "열림"
            join_server(url, cfg["stable_join"], cfg["snipe"])   # 무작위 대기 후 설정한 방식으로 접속
            if cfg["beep"]:
                beep()

        color = {"열림": "g", "감지": "g"}.get(entry["status"], "d")
        # 메시지가 올라온 뒤 몇 초 만에 잡았는지 · 어느 방식으로 잡았는지 (통신 / 내부 / 화면)
        via = {"gateway": "통신", "flux": "내부", "dom": "화면"}.get(d.get("via"), d.get("via") or "?")
        log(f"[{entry['status']}] {entry['where']}  → {url}  ({age / 1000:.2f}초 · {via})", color)
        try:
            Bus.on_event(entry)
        except Exception:
            pass


# ---------------------------------------------------------------- 연결 루프
def session(cfg, handler, hook_src):
    try:
        port = existing_port(cfg)             # 이미 안전하게 디버그 포트로 켜진 디스코드가 있으면 그대로
    except NeedManualRestart:
        return False
    if STOP.is_set():
        return False
    if not port:
        if not launch_discord(cfg):
            return False
        port = ACTIVE_PORT["port"]
    if STOP.is_set():
        return False

    status("connecting", "디스코드 창 찾는 중")
    target = find_target(port)
    if not target:
        if not STOP.is_set():
            log("디스코드 메인 창을 찾을 수 없음 (로그인 화면이면 로그인 필요)", "r")
        return False

    cdp = CDP(ws_url(port, target))
    try:
        cdp.send("Runtime.enable")
        cdp.send("Runtime.addBinding", {"name": BINDING})
        cdp.send("Page.enable")
        cdp.send("Page.addScriptToEvaluateOnNewDocument", {"source": hook_src})
        eval_id = cdp.send("Runtime.evaluate", {"expression": hook_src})
        # 통신 읽기: 디스코드가 받는 웹소켓 데이터를 직접 읽음 (주 방식) — hook.js 는 예비
        gw = gw_mod.Gateway()
        # 응답 내용 보관용 버퍼는 작게 (디스코드가 무거워지지 않게) — 웹소켓 데이터 전달에는 영향 없음
        cdp.send("Network.enable", {"maxTotalBufferSize": 1048576, "maxResourceBufferSize": 262144, "maxPostDataSize": 0})
        gw_deadline = time.time() + 6          # 이 안에 디스코드 통신 연결이 안 보이면 화면을 새로고침해서 처음부터 받음
        gw_reloaded = False
        gw_reported = False
        log(f"디스코드에 연결됨 ({target.get('url', '')[:60]}) — 통신 연결 대기 중…", "c")
        status("waiting", "통신 연결 대기")

        CONNECTED.set()
        waiting = {}
        house = 0.0
        while not STOP.is_set():
            now = time.time()
            if now - house >= 0.5:          # 설정 다시 읽기 등은 0.5초에 한 번만 (메시지 처리를 늦추지 않게)
                house = now
                cfg.reload()
            if not gw.streams and not gw_reloaded and time.time() > gw_deadline:
                # 이미 연결된 통신은 중간부터라 압축을 못 풂 → 디스코드 화면을 한 번 새로고침
                gw_reloaded = True
                log("통신을 처음부터 받기 위해 디스코드 화면을 새로고침", "d")
                cdp.send("Page.reload", {"ignoreCache": False})
            while True:  # 이름 조회 요청 처리
                try:
                    req = _lookup_q.get_nowait()
                except _queue.Empty:
                    break
                known = gw.lookup(req.ids)
                if known is not None:              # 통신에서 받은 이름으로 바로 답함
                    req.result = known
                    req.event.set()
                    continue
                mid = cdp.send("Runtime.evaluate", {
                    "expression": f"window.__lwLookup ? window.__lwLookup({json.dumps(req.ids)}) : null",
                    "returnByValue": True})
                waiting[mid] = req
            try:
                raw = cdp.recv_raw(timeout=0.5)
            except websocket.WebSocketTimeoutException:
                continue
            # 디스코드가 이미지 등을 받을 때마다 오는 네트워크 알림은 해석도 안 하고 버림 (가장 많고 쓸모없음)
            head = raw[:64]
            if '"method":"Network.' in head and '"method":"Network.webSocket' not in head:
                continue
            if '"method":"Network.webSocketFrameSent' in head or '"method":"Network.webSocketWillSendHandshake' in head \
                    or '"method":"Network.webSocketHandshakeResponseReceived' in head:
                continue
            msg = json.loads(raw)

            if msg.get("id") in waiting:
                req = waiting.pop(msg["id"])
                req.result = (msg.get("result") or {}).get("result", {}).get("value") or {}
                req.event.set()
                continue

            if msg.get("id") == eval_id and msg.get("result", {}).get("exceptionDetails"):
                log(f"주입 오류: {msg['result']['exceptionDetails'].get('text')}", "r")

            method = msg.get("method")
            if method == "Network.webSocketCreated":
                if gw.created(msg["params"].get("requestId"), msg["params"].get("url", "")):
                    s = gw.streams[msg["params"].get("requestId")]
                    if s.dead:
                        log(f"통신 압축 형식({s.compress})을 풀 수 없음 — 예비 방식 사용", "y")
                continue
            if method == "Network.webSocketClosed":
                gw.closed(msg["params"].get("requestId"))
                continue
            if method == "Network.webSocketFrameReceived":
                p = msg["params"]
                resp = p.get("response") or {}
                for t, d in gw.frame(p.get("requestId"), resp.get("opcode"), resp.get("payloadData", "")):
                    if t == "READY" and not gw_reported:
                        gw_reported = True
                        log("통신 연결됨 — 모든 채널 감시 중", "g")
                        status("flux", "모든 채널 감시 중")
                    m = gw.message(t, d)
                    if m:
                        handler.handle(m)
                continue
            if method and method.startswith("Network."):
                continue
            if method == "Runtime.bindingCalled" and msg["params"].get("name") == BINDING:
                try:
                    p = json.loads(msg["params"]["payload"])
                except Exception:
                    continue
                kind, data = p.get("kind"), p.get("data") or {}
                if kind == "msg":
                    handler.handle(data)
                elif kind == "diag":
                    log(f"[진단] {json.dumps(data, ensure_ascii=False)}", "d")
                elif kind == "status":
                    # 예비 방식(디스코드 내부) 상태 — 통신 읽기가 되고 있으면 참고로만
                    if gw.ready:
                        log("예비 감지: " + ("연결됨" if data.get("flux") else "실패 (통신 읽기로 감시 중이라 괜찮음)"), "d")
                    elif data.get("flux"):
                        log("Flux 연결됨 — 모든 채널 감시 중", "g")
                        status("flux", "모든 채널 감시 중")
                    elif data.get("flux") is False:
                        log("Flux 연결 실패 — 화면에 열린 채널만 감시 (DOM)", "y")
                        if gw.errors:
                            log(f"통신 읽기 오류: {gw.last_error}", "d")
                        status("dom", "열린 채널만 감시")
            elif method in ("Inspector.detached", "Inspector.targetCrashed"):
                log("디스코드 연결 끊김", "y")
                return True
        return True
    except (websocket.WebSocketConnectionClosedException, ConnectionError, OSError):
        log("디스코드 연결 끊김 (앱 종료/업데이트 재시작)", "y")
        return True
    finally:
        CONNECTED.clear()
        cdp.close()


def run(cfg, handler):
    """STOP 이 켜질 때까지 연결/재연결 반복"""
    if not HOOK_PATH.exists():
        log("hook.js 가 같은 폴더에 없음", "r")
        status("error", "hook.js 없음")
        return
    hook_src = HOOK_PATH.read_text("utf-8")
    while not STOP.is_set():
        try:
            ok = session(cfg, handler, hook_src)
        except Exception as e:
            log(f"오류: {e}", "r")
            ok = False
        if STOP.is_set():
            break
        status("disconnected", "재연결 대기")
        if STOP.wait(3 if ok else 10):
            break
        log("다시 연결 시도…", "d")
    status("stopped", "중지됨")
