# -*- coding: utf-8 -*-
"""
바이옴 매크로
- 바이옴 감지: 로블록스 로그 파일(%LOCALAPPDATA%\\Roblox\\logs)의 [BloxstrapRPC] 줄에서 hoverText 읽기
- 플레이어 이름이 설정돼 있으면 그 계정의 로그만 사용 (TutorialCursor 줄로 확인)
- 디스코드 웹후크: 바이옴 시작 · 끝 임베드
"""
import json
import os
import re
import threading
import time
import urllib.request
from datetime import datetime, timezone
from pathlib import Path

from version import VERSION
# 매크로 받는 곳 — 디스코드 서버 초대 링크
DISCORD_URL = "https://discord.gg/B9QzJuseHR"
MACRO_NAME_LABEL = f"Acrux macro V{VERSION}"
EMBED_COLOR = 0xFFFFFF
EXIT_COLOR = 0xFF0000
THUMB_URL = "https://raw.githubusercontent.com/rngenesis0-coder/Acrux-macro/refs/heads/main/{}.png"
RARE_BIOMES = {"GLITCHED", "DREAMSPACE", "CYBERSPACE"}
TARGET_BIOMES = {"WINDY", "SNOWY", "RAINY", "SANDSTORM", "HELL", "STARFALL", "HEAVEN",
                 "CORRUPTION", "NULL", "GLITCHED", "DREAMSPACE", "CYBERSPACE", "SINGULARITY"}

LOG_DIR = Path(os.environ.get("LOCALAPPDATA", "")) / "Roblox" / "logs"
RPC_RE = re.compile(r'"largeImage"\s*:\s*\{[^}]*"hoverText"\s*:\s*"([^"]+)"')
USER_RE = re.compile(r"Players\.([^.']+)\.PlayerGui:WaitForChild\((?:\"|\\\")TutorialCursor(?:\"|\\\")\)", re.I)
DISCONNECT_MARK = "[FLog::Network] Client:Disconnect"


def _log(msg, color=""):
    try:
        import watcher_core as core
        core.log(msg, color)
    except Exception:
        print(msg)


def norm_biome(name):
    """'Sand Storm' → 'SANDSTORM'"""
    return (name or "").upper().replace(" ", "").strip()


# ---------------------------------------------------------------- 웹후크
def _post(url, payload, timeout=8):
    data = json.dumps(payload).encode("utf-8")
    req = urllib.request.Request(url, data=data, method="POST", headers={
        "Content-Type": "application/json", "User-Agent": "AcruxMacro/1.0"})
    with urllib.request.urlopen(req, timeout=timeout) as r:
        return r.status


# 디스코드 웹후크 주소만 허용 (https · 디스코드 도메인 · /api/webhooks/숫자ID/토큰)
WEBHOOK_RE = re.compile(r"https://(?:(?:ptb|canary)\.)?discord(?:app)?\.com/api/(?:v\d+/)?webhooks/\d+/[A-Za-z0-9_\-]+/?"
                        r"(?:\?[A-Za-z0-9_=&\-]*)?")     # ?thread_id= (포럼 채널) 등


def is_webhook(url):
    return bool(WEBHOOK_RE.fullmatch((url or "").strip()))


def send_all(hooks, payload, wait=False):
    """모든 웹후크로 전송. wait=True 면 결과 [(url 끝부분, 성공여부/에러)] 반환
    디스코드 웹후크 주소가 아닌 건 보내지 않음 (내 브섭 링크가 들어 있는 내용이라 다른 곳으로 새지 않게)"""
    hooks = [h.strip() for h in hooks if h and h.strip()]
    results = []

    def one(h):
        if not is_webhook(h):
            results.append((h[-8:], "디스코드 웹후크 주소 형식이 아님"))
            return
        try:
            st = _post(h, payload)
            results.append((h[-8:], st in (200, 204)))
        except Exception as e:
            results.append((h[-8:], str(e)))
            _log(f"웹후크 전송 실패: {e}", "r")

    threads = [threading.Thread(target=one, args=(h,), daemon=True) for h in hooks]
    for t in threads:
        t.start()
    if wait:
        for t in threads:
            t.join(10)
    return results


def _now_iso():
    return datetime.now(timezone.utc).isoformat()


def toggle_payload(enabled, player=None):        # 플레이어 이름은 웹후크에 안 띄움 (감지용으로만 씀)
    return {"embeds": [{
        # 바이옴 매크로를 켤 때 · 끌 때: 매크로를 받을 수 있는 디스코드 서버 안내
        "description": "> ## Biome Macro " + ("Enabled" if enabled else "Disabled")
                       + f"\nJoin discord to download macro\n{DISCORD_URL}",
        "color": EMBED_COLOR,
        "timestamp": _now_iso(),
        "footer": {"text": MACRO_NAME_LABEL},
    }]}


def mention_text(biome, cfg):
    """바이옴 시작 알림의 멘션: 희귀 바이옴별 @everyone + 바이옴별 역할"""
    b = norm_biome(biome)
    parts = []
    if b in RARE_BIOMES and (cfg.get("everyone") or {}).get(b, True):
        parts.append("@everyone")
    role = str((cfg.get("roles") or {}).get(b) or "").strip()
    if role.isdigit():
        parts.append(f"<@&{role}>")
    return " ".join(parts)


def biome_payload(biome, action, player, ps_link, uptime, mention=""):
    """action: 'started' / 'ended'. mention 은 시작 알림에만 붙음"""
    b = norm_biome(biome)
    started = action == "started"
    desc = f"> ## Biome {'Started' if started else 'Ended'} - {b}"
    # 제목 아래: 시작 / 끝난 시각 (디스코드가 보는 사람 기준 'N분 전' 으로 보여줌)
    desc += f"\n> \n> **{'Started' if started else 'Ended'}:** <t:{int(time.time())}:R>"
    embed = {
        "description": desc,
        "color": EMBED_COLOR,
        "timestamp": _now_iso(),
        "fields": [
            {"name": "Uptime", "value": uptime or "Unknown", "inline": True},
        ],
        "thumbnail": {"url": THUMB_URL.format(b)},
    }
    if started:
        embed["fields"].append({"name": "Private Server Link", "value": ps_link or "None", "inline": False})
    embed["footer"] = {"text": MACRO_NAME_LABEL}             # 시작 · 끝 둘 다 아래에 Acrux 매크로 표시
    return {"content": mention if started else "", "embeds": [embed],
            "allowed_mentions": {"parse": ["everyone", "roles"]}}


def exit_payload(player):
    return {"embeds": [{
        "description": "> ## Player has left the game (Disconnected/Left)",
        "color": EXIT_COLOR,
        "timestamp": _now_iso(),
        "footer": {"text": MACRO_NAME_LABEL},
    }]}


def test_payload(player, ps_link):
    return {"embeds": [{
        "description": "> ## Webhook Test\n> Webhook connection test",
        "color": EMBED_COLOR,
        "timestamp": _now_iso(),
        "fields": [
            {"name": "Private Server Link", "value": ps_link or "Not set", "inline": False},
        ],
        "footer": {"text": MACRO_NAME_LABEL},
    }]}


# ---------------------------------------------------------------- 로그 감시
_LATEST_CACHE = {}          # 로그 폴더 → (확인한 시각, 최신 파일)
_LATEST_TTL = 1.0           # 폴더 전체 검색은 1초에 한 번만 (자주 부르는 곳이 많아서)


def latest_log(log_dir=None):
    d = Path(log_dir or LOG_DIR)
    now = time.time()
    hit = _LATEST_CACHE.get(str(d))
    if hit and now - hit[0] < _LATEST_TTL:
        return hit[1]
    best = _scan_latest(d)
    _LATEST_CACHE[str(d)] = (now, best)
    return best


def _scan_latest(d):
    try:
        files = [p for p in d.iterdir() if p.suffix == ".log"]
    except OSError:
        return None
    best, best_t = None, -1
    for p in files:
        try:
            t = p.stat().st_mtime
        except OSError:
            continue
        if t > best_t:
            best, best_t = p, t
    return best


def fmt_uptime(sec):
    """매크로를 켜둔 시간 → 'Xhr Ymn Zsc' (3시간 2분 5초 = 3hr 2mn 5sc)"""
    s = max(0, int(sec))
    return f"{s // 3600}hr {s % 3600 // 60}mn {s % 60}sc"


class BiomeWatcher:
    """1초마다 최신 로그의 새 줄을 읽어서 바이옴 변화를 감지"""

    def __init__(self, get_cfg, is_active, log_dir=None, on_change=None):
        self.get_cfg = get_cfg          # () -> biome 설정 dict
        self.on_change = on_change      # (이전, 새 바이옴, 스나이핑 접속인지) — 바이옴이 바뀔 때마다 (웹후크와 무관)
        self.is_active = is_active      # () -> 웹후크를 보낼 상태인지 (바이옴 매크로 토글 켜짐)
        self.log_dir = log_dir
        self.current = None             # 지금 바이옴 (예: 'GLITCHED')
        self.file = None
        self.pos = 0
        self.file_owner = None          # 'ok' / 'other' / None(아직 모름)
        self.file_started = time.time()
        self.was_active = False
        self.active_since = None        # 바이옴 매크로가 켜진 시각 (Uptime 기준)
        self.left_sent = False
        self.stop_ev = threading.Event()
        self.thread = None
        self.history = []               # [(time, biome)]
        self.pending = None             # 계정 확인 전에 읽은 바이옴 (확인되면 적용)
        self.last_player = None
        # 스나이핑(다른 사람 서버) 접속: 그 접속의 로그 파일 동안은 바이옴 웹후크를 안 보냄
        self.mute_pending, self.mute_base, self.muted_file = False, None, None

    def mute_next_session(self):
        """링크로 다른 서버에 접속할 때 호출 — 다음에 새로 생기는 로그 파일(= 그 접속) 동안 웹후크 끔"""
        self.mute_base = latest_log(self.log_dir)
        self.mute_pending = True

    def muted(self):
        return bool(self.file) and self.file == self.muted_file

    def start(self):
        if self.thread and self.thread.is_alive():
            return
        self.thread = threading.Thread(target=self._loop, daemon=True)
        self.thread.start()

    def state(self):
        return {"current": self.current, "log": bool(self.file), "owner": self.file_owner, "muted": self.muted(),
                "history": self.history[-50:]}

    # 설정값
    def _cfg(self):
        c = self.get_cfg() or {}
        hooks = [h for h in (c.get("webhooks") or []) if h]
        return c, hooks, (c.get("player") or "").strip().lstrip("@").strip()

    def _loop(self):
        while not self.stop_ev.wait(1.0):
            try:
                self.tick()
            except Exception as e:
                _log(f"바이옴 감시 오류: {e}", "r")

    def tick(self):
        cfg, hooks, player = self._cfg()
        active = bool(self.is_active())

        # 켜짐/꺼짐 알림 (스크립트의 Biome Macro Enabled/Disabled)
        if active != self.was_active:
            self.was_active = active
            self.active_since = time.time() if active else None
            if hooks:
                send_all(hooks, toggle_payload(active, player))
            _log("바이옴 매크로 " + ("켜짐" if active else "꺼짐"), "c")

        # 플레이어 이름이 바뀌면 최신 로그를 처음부터 다시 확인
        if player != self.last_player:
            self.last_player = player
            self.file, self.current, self.pending = None, None, None
        if not player:
            self.file_owner = "noname"           # 이름이 없으면 감지 안 함
            return

        path = latest_log(self.log_dir)
        if path is None:
            self.file = None
            return
        new_file = path != self.file
        if new_file:
            if self.file and self.file == self.muted_file:
                self.current = None              # 스나이핑 접속에서 본 바이옴은 내 서버로 이어서 알리지 않음
            self.file, self.pos, self.file_owner, self.pending = path, 0, None, None
            self.fresh = True                    # 이 파일을 처음 읽는 중 (기존 기록은 알림 안 보냄)
            self.left_sent = False
            if self.mute_pending and path != self.mute_base:
                self.mute_pending, self.muted_file = False, path
                self.current = None
                _log("다른 서버 접속 (스나이핑) — 이번 접속 동안 바이옴 웹후크 안 보냄", "y")

        try:
            with open(path, "r", encoding="utf-8", errors="ignore") as f:
                f.seek(self.pos)
                lines = f.readlines()
                self.pos = f.tell()
        except OSError:
            return
        if not lines:
            return

        # 마지막으로 나온 바이옴
        found = None
        for ln in reversed(lines):
            if "[BloxstrapRPC]" in ln and '"largeImage"' in ln:
                m = RPC_RE.search(ln)
                if m:
                    found = norm_biome(m.group(1))
                    break

        # 계정 확인: 로그 속 실제 닉네임이 입력한 이름과 같아야 함 (대소문자 무시)
        if self.file_owner != "ok":
            for ln in lines:
                m = USER_RE.search(ln)
                if m:
                    name = m.group(1).strip()
                    self.file_owner = "ok" if name.lower() == player.lower() else "other"
                    if self.file_owner == "other":
                        _log(f"닉네임 불일치 — 로그의 닉네임 '{name}' ≠ 입력한 '{player}' · 감지 안 함", "n")
                    else:
                        _log(f"계정 확인됨: {name}", "d")
                    break
            if self.file_owner != "ok":
                if found:
                    self.pending = found         # 확인되면 적용
                return
            if not found:
                found, self.pending = self.pending, None

        # 접속 끊김
        if any(DISCONNECT_MARK in ln for ln in lines) and not self.left_sent:
            self.left_sent = True
            _log("로블록스 접속 끊김 감지 (로그)", "y")
            if active and hooks and not self.muted():
                send_all(hooks, exit_payload(player))

        fresh, self.fresh = getattr(self, "fresh", False), False
        if not found or found == self.current:
            return

        prev, self.current = self.current, found
        self.history.append((time.time(), found))
        self.history = self.history[-50:]
        _log(f"바이옴: {prev or '-'} → {found}", "g" if found in RARE_BIOMES else "c")
        if self.on_change:
            try:
                self.on_change(prev, found, self.muted())
            except Exception as e:
                _log(f"바이옴 변경 처리 오류: {e}", "r")

        # 앱을 켜거나 이름을 바꾼 직후 읽은 기존 기록이면 알림 없이 현재 바이옴만 맞춤
        if fresh and prev is None:
            return
        if not (active and hooks) or self.muted():
            return
        uptime = fmt_uptime(time.time() - (self.active_since or time.time()))
        ps = (cfg.get("ps_link") or "").strip()
        if prev and prev in TARGET_BIOMES and prev != found:
            send_all(hooks, biome_payload(prev, "ended", player, ps, uptime))
        if found in TARGET_BIOMES:
            send_all(hooks, biome_payload(found, "started", player, ps, uptime, mention_text(found, cfg)))
