# -*- coding: utf-8 -*-
"""
바이옴 매크로
- 바이옴 감지: 로블록스 로그 파일(%LOCALAPPDATA%\\Roblox\\logs)의 [BloxstrapRPC] 줄에서 hoverText 읽기
- 플레이어 이름이 설정돼 있으면 그 계정의 로그만 사용 (TutorialCursor 줄로 확인)
- 디스코드 웹후크: 바이옴 시작 · 끝 임베드
"""
import difflib
import json
import os
import re
import threading
import time
import unicodedata
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
                 "CORRUPTION", "NULL", "GLITCHED", "DREAMSPACE", "CYBERSPACE", "SINGULARITY",
                 "LOSTFOREST", "RADIANTFOREST",
                 "PUMPKINMOON", "GRAVEYARD", "BLOODRAIN"}           # 할로윈 이벤트 바이옴
# 웹후크 썸네일 이미지가 있는 바이옴 (없는 바이옴은 썸네일 없이 보냄)
THUMB_BIOMES = {"WINDY", "SNOWY", "RAINY", "SANDSTORM", "HELL", "STARFALL", "HEAVEN",
                "CORRUPTION", "NULL", "GLITCHED", "DREAMSPACE", "CYBERSPACE", "SINGULARITY"}
# 여러 낱말 바이옴의 보여줄 이름 (키는 띄어쓰기 없이)
_SHOW = {"LOSTFOREST": "LOST FOREST", "RADIANTFOREST": "RADIANT FOREST", "PUMPKINMOON": "PUMPKIN MOON", "BLOODRAIN": "BLOOD RAIN"}

LOG_DIR = Path(os.environ.get("LOCALAPPDATA", "")) / "Roblox" / "logs"
RPC_RE = re.compile(r'"largeImage"\s*:\s*\{[^}]*"hoverText"\s*:\s*"([^"]+)"')
USER_RE = re.compile(r"Players\.([^.']+)\.PlayerGui:WaitForChild\((?:\"|\\\")TutorialCursor(?:\"|\\\")\)", re.I)
DISCONNECT_MARK = "[FLog::Network] Client:Disconnect"


def _log(msg, color=""):
    try:
        import core
        core.log(msg, color)
    except Exception:
        print(msg)


def _known(s):
    """아는 바이옴 키 (같거나 · 글자 한두 개만 다르면) 또는 None"""
    if s in TARGET_BIOMES:
        return s
    if len(s) >= 5:
        near = difflib.get_close_matches(s, TARGET_BIOMES, n=1, cutoff=0.8)
        if near:
            return near[0]
    return None


def norm_biome(name):
    """바이옴 이름 → 키: 'Sand Storm' → 'SANDSTORM' · 'The Lost Forest' → 'LOSTFOREST'
    영문 · 숫자만 남김 (꾸밈 글자 · 기호 · 띄어쓰기가 섞여도 같은 키) · 앞의 THE 는 뗌 ·
    아는 바이옴과 거의 같으면(글자 한두 개 차이) 그 바이옴으로"""
    s = re.sub(r"[^A-Z0-9]", "", unicodedata.normalize("NFKD", name or "").upper())
    if not s:
        return ""
    k = _known(s) or (_known(s[3:]) if s.startswith("THE") else None)
    return k or s


def show_name(b):
    """키 → 보여줄 이름 ('LOSTFOREST' → 'LOST FOREST')"""
    return _SHOW.get(b, b or "")




def read_new(path, pos):
    """로그 파일의 pos(바이트) 뒤에 새로 '끝까지 써진 줄'만 읽음 → (글자, 새 pos)
    로블록스가 긴 줄(바이옴 정보 줄)을 나눠 쓰는 중에 읽으면 줄이 반으로 잘려서, 다음에 읽을 때
    나머지 반만 남아 그 줄의 바이옴 · 게임 입장 표시를 통째로 놓쳤음 → 줄 끝(\\n)이 안 써진 마지막 줄은 다음에 다시 읽음"""
    with open(path, "rb") as f:
        size = os.fstat(f.fileno()).st_size
        if size < pos:                          # 파일이 새로 써짐 → 처음부터
            pos = 0
        f.seek(pos)
        data = f.read()
    cut = data.rfind(b"\n")
    if cut < 0:
        return "", pos
    return data[:cut + 1].decode("utf-8", "ignore"), pos + cut + 1


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


# 웹후크마다 한 줄로 차례대로 보냄 — 전에는 알림마다 스레드를 따로 띄워서 바이옴 끝 · 시작이 동시에 나가면
# 가끔 Started 가 Ended 보다 먼저 도착했음 (같은 웹후크로 가는 건 보낸 순서 그대로)
_queues = {}
_queues_lock = threading.Lock()


def _hook_worker(q):
    while True:
        h, payload, done = q.get()
        try:
            done(_send_one(h, payload))
        except Exception as e:
            _log(f"웹후크 처리 오류: {e}", "r")


def _send_one(h, payload):
    """한 웹후크로 전송 → True / 에러 글자 · 디스코드가 너무 빠르다고(429) 하면 잠깐 기다렸다 다시 (순서 유지)"""
    import urllib.error
    for tries in range(4):
        try:
            st = _post(h, payload)
            return st in (200, 204)
        except urllib.error.HTTPError as e:
            if e.code == 429 and tries < 3:
                try:
                    wait = float(e.headers.get("Retry-After") or 1.5)
                except ValueError:
                    wait = 1.5
                time.sleep(min(10.0, max(0.5, wait)))
                continue
            _log(f"웹후크 전송 실패: {e}", "r")
            return str(e)
        except Exception as e:
            _log(f"웹후크 전송 실패: {e}", "r")
            return str(e)
    return "429"


def _enqueue(h, payload, done):
    with _queues_lock:
        q = _queues.get(h)
        if q is None:
            import queue
            q = queue.Queue()
            _queues[h] = q
            threading.Thread(target=_hook_worker, args=(q,), daemon=True).start()
    q.put((h, payload, done))


def send_all(hooks, payload, wait=False):
    """모든 웹후크로 전송. wait=True 면 결과 [(url 끝부분, 성공여부/에러)] 반환
    디스코드 웹후크 주소가 아닌 건 보내지 않음 (내 브섭 링크가 들어 있는 내용이라 다른 곳으로 새지 않게)"""
    hooks = [h.strip() for h in hooks if h and h.strip()]
    results = []
    pending = []
    for h in hooks:
        if not is_webhook(h):
            results.append((h[-8:], "디스코드 웹후크 주소 형식이 아님"))
            continue
        ev = threading.Event()
        pending.append(ev)

        def done(r, h=h, ev=ev):
            results.append((h[-8:], r))
            ev.set()
        _enqueue(h, payload, done)
    if wait:
        for ev in pending:
            ev.wait(15)
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
    desc = f"> ## Biome {'Started' if started else 'Ended'} - {show_name(b)}"
    # 제목 아래: 시작 / 끝난 시각 (디스코드가 보는 사람 기준 'N분 전' 으로 보여줌)
    desc += f"\n> \n> **{'Started' if started else 'Ended'}:** <t:{int(time.time())}:R>"
    embed = {
        "description": desc,
        "color": EMBED_COLOR,
        "timestamp": _now_iso(),
        "fields": [
            {"name": "Uptime", "value": uptime or "Unknown", "inline": True},
        ],
    }
    if b in THUMB_BIOMES:
        embed["thumbnail"] = {"url": THUMB_URL.format(b)}
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
    """폴더에서 가장 최근에 바뀐 .log — os.scandir 는 윈도우에서 파일 정보를 목록과 같이 받아 와서
    파일마다 따로 묻지 않음 (로블록스 로그가 수백 · 수천 개 쌓여도 가벼움)"""
    best, best_t = None, -1.0
    try:
        with os.scandir(d) as it:
            for e in it:
                if not e.name.endswith(".log"):
                    continue
                try:
                    t = e.stat().st_mtime
                except OSError:
                    continue
                if t > best_t:
                    best, best_t = e.path, t
    except OSError:
        return None
    return Path(best) if best else None


def fmt_uptime(sec):
    """매크로를 켜둔 시간 → 'Xhr Ymn Zsc' (3시간 2분 5초 = 3hr 2mn 5sc)"""
    s = max(0, int(sec))
    return f"{s // 3600}hr {s % 3600 // 60}mn {s % 60}sc"


class BiomeWatcher:
    """1초마다 최신 로그의 새 줄을 읽어서 바이옴 변화를 감지"""

    def __init__(self, get_cfg, is_active, log_dir=None, on_change=None):
        self.get_cfg = get_cfg          # () -> biome 설정 dict
        self.on_change = on_change      # (이전, 새 바이옴, 다른 서버 접속인지) — 바이옴이 바뀔 때마다 (웹후크와 무관)
        self.is_active = is_active      # () -> 웹후크를 보낼 상태인지 (바이옴 매크로 토글 켜짐)
        self.log_dir = log_dir
        self.current = None             # 지금 바이옴 (예: 'GLITCHED')
        self.seen_file = None           # 지금 바이옴을 마지막으로 확인한 로그 파일 (로블록스를 다시 켜면 새 파일에서 확인될 때까지 옛 바이옴)
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
        # 모드가 다른 사람 서버로 접속: 그 접속의 로그 파일 동안은 바이옴 웹후크를 안 보냄
        self.away_pending, self.away_base, self.away_file = False, None, None
        self.away_at = 0.0

    def away_next_session(self):
        """모드가 다른 서버에 접속할 때 호출 — 다음에 새로 생기는 로그 파일(= 그 접속) 동안 웹후크 끔"""
        self.away_base = latest_log(self.log_dir)
        self.away_at = time.time()
        self.away_pending = True

    def own_next_session(self):
        """내 서버로 들어갈 때 호출 (매크로 복귀 · 내 브섭 바로 접속) — 다음 접속은 내 서버라 웹후크를 다시 보냄
        다른 서버 접속이 로블록스를 못 띄웠으면 '다음 접속은 조용히' 가 남아서 내 서버가 조용해지던 문제를 막음
        (다른 서버 접속의 로그가 이미 생겼는데 아직 못 읽었으면 그 로그만 조용히)"""
        if not self.away_pending:
            return
        p = _scan_latest(Path(self.log_dir or LOG_DIR))      # 캐시 말고 지금 폴더 그대로
        if p and p != self.away_base:
            self.away_file = p
        self.away_pending = False

    def away(self):
        """지금 로그가 다른 사람 서버 접속인지"""
        return bool(self.file) and self.file == self.away_file

    def start(self):
        if self.thread and self.thread.is_alive():
            return
        self.thread = threading.Thread(target=self._loop, daemon=True)
        self.thread.start()

    def state(self):
        return {"current": self.current, "log": bool(self.file), "owner": self.file_owner, "away": self.away(),
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
        if self.away_pending and time.time() - self.away_at > 900:
            self.away_pending = False            # 15분 동안 새 접속이 안 생김 → 접속 못 한 것 (다음 접속을 조용히 하지 않음)
        if path is None:
            self.file = None
            return
        new_file = path != self.file
        if new_file:
            if self.file and self.file == self.away_file:
                self.current = None              # 다른 서버에서 본 바이옴은 내 서버로 이어서 알리지 않음
            self.file, self.pos, self.file_owner, self.pending = path, 0, None, None
            self.fresh = True                    # 이 파일을 처음 읽는 중 (기존 기록은 알림 안 보냄)
            self.left_sent = False
            if self.away_pending and path != self.away_base:
                self.away_pending, self.away_file = False, path
                self.current = None
                _log("다른 서버 접속 — 이번 접속 동안 바이옴 웹후크 안 보냄", "y")

        try:
            text, self.pos = read_new(path, self.pos)     # 끝까지 써진 줄만 (반쯤 써진 줄은 다음에)
        except OSError:
            return
        lines = text.splitlines()
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
            if active and hooks and not self.away():
                send_all(hooks, exit_payload(player))

        fresh, self.fresh = getattr(self, "fresh", False), False
        if found:
            self.seen_file = self.file
        if not found or found == self.current:
            return

        prev, self.current = self.current, found
        self.history.append((time.time(), found))
        self.history = self.history[-50:]
        _log(f"바이옴: {show_name(prev) or '-'} → {show_name(found)}", "g" if found in RARE_BIOMES else "c")
        if self.on_change:
            try:
                self.on_change(prev, found, self.away())
            except Exception as e:
                _log(f"바이옴 변경 처리 오류: {e}", "r")

        # 앱을 켜거나 이름을 바꾼 직후 읽은 기존 기록이면 알림 없이 현재 바이옴만 맞춤
        if fresh and prev is None:
            return
        if not (active and hooks) or self.away():
            return
        uptime = fmt_uptime(time.time() - (self.active_since or time.time()))
        ps = (cfg.get("ps_link") or "").strip()
        if prev and prev in TARGET_BIOMES and prev != found:
            send_all(hooks, biome_payload(prev, "ended", player, ps, uptime))
        if found in TARGET_BIOMES:
            send_all(hooks, biome_payload(found, "started", player, ps, uptime, mention_text(found, cfg)))
