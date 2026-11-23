# -*- coding: utf-8 -*-
"""
바이옴 매크로
- 바이옴 감지: 로블록스 로그 파일(%LOCALAPPDATA%\\Roblox\\logs)의 [BloxstrapRPC] 줄에서 hoverText 읽기
- 플레이어 이름이 설정돼 있으면 그 계정의 로그만 사용 (TutorialCursor 줄로 확인)
- 디스코드 웹후크: 바이옴 시작 · 끝 임베드
"""
# =====================================================================
#  이 코드는 보안을 위해 건드리지 마십시오.
#  건드릴 경우 변조 클라이언트 감지 시스템으로 IP밴을 당하실 수 있습니다.
# =====================================================================

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
DISCORD_URL = "https://discord.gg/cc9APrrnNv"
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
# 서버에 들어갈 때마다 찍힘: [FLog::Output] ! Joining game '<서버 ID>' place <장소 ID> at <IP> — 이 서버 ID 로 내 서버인지 구분
SAFE_SEC = 60                  # 안전 모드: 서버 ID 가 이만큼 그대로여야 바이옴 알림을 보냄 (초)
JOIN_RE = re.compile(r"! Joining game '([0-9a-fA-F-]{36})' place (\d+)")


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


# =====================================================================
#  이 코드는 보안을 위해 건드리지 마십시오.
#  건드릴 경우 변조 클라이언트 감지 시스템으로 IP밴을 당하실 수 있습니다.
# =====================================================================
# 차단된 서버 (웹사이트 /api/block 에서 어드민이 관리) — 바이오미 웹후크의 목적지 서버가 차단됐으면
#  그 웹후크로는 알림 대신 안내를 한 번만 보내고 전송을 막음.
#  속도: 보낼 때마다 확인하지 않음 — 웹후크별로 서버 ID·차단 여부를 한 번 알아내 캐시(10분)하고 전송은 캐시만 봄.
block_lookup = None          # app 이 설정: (guild_id) -> (status, json) | None   (/api/macro/blocked)
_BLOCK_TTL = 600.0
_block_cache = {}
_block_resolving = set()
_block_lock = threading.Lock()


def _webhook_guild(hook):
    """웹후크 주소를 GET 하면 그 웹후크가 속한 서버 ID(guild_id) 를 줌 (못 알아내면 None)"""
    url = (hook or "").strip().split("?", 1)[0]
    try:
        req = urllib.request.Request(url, method="GET", headers={"User-Agent": "AcruxMacro/1.0"})
        with urllib.request.urlopen(req, timeout=8) as r:
            j = json.loads(r.read(8192).decode("utf-8") or "{}")
        g = str(j.get("guild_id") or "")
        return g or None
    except Exception:
        return None


def _resolve_block(hook):
    """웹후크 → 서버 ID → 웹사이트에 "이 서버 차단?" 조회 → 캐시 (네트워크 · 뒤 스레드에서만)"""
    info = {"at": time.time(), "resolved": True, "blocked": False, "guild": None,
            "reason": "", "block_at": 0, "discord": "", "images": [], "notice_sent": False}
    try:
        info["guild"] = _webhook_guild(hook)
        if info["guild"] and block_lookup:
            res = block_lookup(info["guild"])
            if res:
                st, j = res
                if st == 200 and j.get("blocked"):
                    info["blocked"] = True
                    info["reason"] = str(j.get("reason") or "")
                    info["block_at"] = int(j.get("at") or 0)
                    info["discord"] = str(j.get("discord") or "")
                    info["images"] = [str(u) for u in (j.get("images") or []) if u][:10]
                    prev = _block_cache.get(hook)
                    if prev and prev.get("blocked") and prev.get("notice_sent") and prev.get("guild") == info["guild"]:
                        info["notice_sent"] = True
    except Exception:
        info["at"] = time.time() - _BLOCK_TTL + 60    # 실패 → 통과하되 1분 뒤 다시 시도
    finally:
        with _block_lock:
            _block_cache[hook] = info
            _block_resolving.discard(hook)
    if info["blocked"]:
        _notice_once(hook, info)       # 차단 확인되면 (웹후크 입력·전송 무관) 바로 안내 1회


def _block_for(hook):
    """캐시만 봄 — 없거나 오래됐으면 뒤에서 조회 시작, 지금은 있는 값(없으면 None) → 전송은 통과"""
    now = time.time()
    with _block_lock:
        b = _block_cache.get(hook)
        if b and now - b["at"] < _BLOCK_TTL:
            return b
        if hook not in _block_resolving:
            _block_resolving.add(hook)
            threading.Thread(target=_resolve_block, args=(hook,), daemon=True).start()
    return b


def _blocked_notice_payload(b):
    """차단된 서버 웹후크로 한 번만 보내는 안내 (양식 그대로)"""
    invite = b.get("discord") or DISCORD_URL
    at = b.get("block_at") or 0
    try:
        date = datetime.fromtimestamp(at / 1000).strftime("%Y/%m/%d") if at else "-"
    except Exception:
        date = "-"
    reason = ((b.get("reason") or "").strip().replace("\n", "\n> ")) or "-"
    desc = ("> ## 이 서버는 아크룩스 매크로 [ Acrux Macro ] 에서 차단되었습니다.\n"
            "> 본 서버에서 티켓을 열어, 차단 해제 신청을 할 수 있습니다:\n"
            "> " + invite + "\n> \n"
            "> **차단된 날짜:** " + date + " ~\n"
            "> **차단 사유:**\n> " + reason)
    embeds = [{"description": desc, "color": EXIT_COLOR, "timestamp": _now_iso(), "footer": {"text": MACRO_NAME_LABEL}}]
    imgs = b.get("images") or []
    if imgs:
        embeds[0]["image"] = {"url": imgs[0]}
    for u in imgs[1:10]:
        embeds.append({"color": EXIT_COLOR, "image": {"url": u}})
    return {"embeds": embeds}


def _notice_once(hook, b):
    if b.get("notice_sent"):
        return
    b["notice_sent"] = True
    _enqueue(hook, _blocked_notice_payload(b), lambda r: None)
    _log("차단된 서버라 바이오미 알림 대신 안내를 보냄", "y")


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
        blk = _block_for(h)
        if blk and blk["resolved"] and blk["blocked"]:   # 차단된 서버 → 알림은 건너뛰고 안내만 한 번
            _notice_once(h, blk)
            results.append((h[-8:], "blocked"))
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
        self.on_change = on_change      # (이전, 새 바이옴, 안전 모드로 붙잡는 중인지) — 바이옴이 바뀔 때마다 (웹후크와 무관)
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
        self._last_hooks = None          # 웹후크 설정이 바뀌면 바로 차단 확인
        # 안전 모드: 서버 ID (로그의 Joining game) 가 SAFE_SEC 초 이상 그대로일 때만 바이옴 웹후크를 보냄
        # (다른 사람 서버에 잠깐 들어갔다 나오는 경우 알림이 안 가게) · K 키로 타이머 건너뜀
        self.job = None                 # 지금 접속한 서버 ID (로그 파일이 바뀌어도 같은 서버면 이어서)
        self.job_since = None           # 이 서버 ID 를 처음 본 시각
        self.skip_job, self.skip_file = None, None    # K 로 건너뛴 서버 (서버 ID 를 모를 때 누르면 그 로그 파일)
        self.held = None                # 안전 모드 때문에 아직 안 보낸 바이옴 (시작 알림)
        self.announced = None           # 시작 알림을 보낸 바이옴 (안전 모드일 때 끝 알림은 이것만)

    def safe_on(self):
        return bool((self.get_cfg() or {}).get("safe_mode"))

    def safe_left(self):
        """안전 모드 타이머 남은 초 · 0 이면 보내도 됨"""
        if not self.safe_on():
            return 0
        if self.skip_file and self.skip_file == self.file and self.skip_job in (None, self.job):
            return 0
        if not self.job or self.job_since is None:
            return SAFE_SEC
        return max(0, int(SAFE_SEC - (time.time() - self.job_since) + 0.999))

    def holding(self):
        """안전 모드로 바이옴 알림을 붙잡는 중인지 (타이머가 안 끝남)"""
        return self.safe_left() > 0

    def skip_safe(self):
        """K: 지금 서버의 안전 모드 타이머를 건너뜀 — 붙잡은 바이옴은 다음 확인 때 바로 보냄"""
        if not self.holding():
            return False
        self.skip_job, self.skip_file = self.job, self.file
        _log("안전 모드 타이머 건너뜀 (K)" + (f" — 서버 ID {self.job[:8]}" if self.job else ""), "g")
        return True

    def _server(self, lines):
        """새 줄에서 서버 ID 를 읽음 (한 로그 파일 안에서도 텔레포트로 여러 번 들어갈 수 있음 → 마지막 것)"""
        for ln in lines:
            m = JOIN_RE.search(ln)
            if not m:
                continue
            job = m.group(1).lower()
            if job != self.job:
                if self.skip_file == self.file and self.skip_job is None and self.job is None:
                    self.skip_job = job          # 서버 ID 를 모를 때 K → 이 접속의 서버로
                self.job, self.job_since = job, time.time()
                if self.safe_on():
                    _log(f"서버 접속 (서버 ID {job[:8]}) — 안전 모드: {SAFE_SEC}초 동안 그대로면 바이옴 알림 보냄", "d")

    def _release(self, cfg, hooks, player, active):
        """안전 모드 타이머가 끝나면 붙잡아 둔 바이옴 시작 알림을 보냄 (아직 그 바이옴일 때만)"""
        if not self.held or self.holding():
            return
        held, self.held = self.held, None
        if held != self.current or not (active and hooks) or self.file_owner != "ok":
            return
        uptime = fmt_uptime(time.time() - (self.active_since or time.time()))
        ps = (cfg.get("ps_link") or "").strip()
        _log(f"안전 모드 통과 — {show_name(held)} 알림 보냄", "g")
        self.announced = held
        send_all(hooks, biome_payload(held, "started", player, ps, uptime, mention_text(held, cfg)))

    def start(self):
        if self.thread and self.thread.is_alive():
            return
        self.thread = threading.Thread(target=self._loop, daemon=True)
        self.thread.start()

    def state(self):
        left = self.safe_left()
        return {"current": self.current, "log": bool(self.file), "owner": self.file_owner,
                "job": self.job[:8] if self.job else None,
                "safe": self.safe_on(), "safe_left": left, "held": self.held if left else None,
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

        if hooks != self._last_hooks:          # 설정에서 웹후크를 입력·변경하면 바로 차단 확인 (차단이면 안내)
            self._last_hooks = hooks
            for h in hooks:
                if is_webhook(h):
                    _block_for(h)

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
        if not self.safe_on():
            self.held = None
        self._release(cfg, hooks, player, active)
        if path is None:
            self.file = None
            return
        new_file = path != self.file
        if new_file:
            self.file, self.pos, self.file_owner, self.pending = path, 0, None, None
            self.fresh = True                    # 이 파일을 처음 읽는 중 (기존 기록은 알림 안 보냄)
            self.left_sent = False

        try:
            text, self.pos = read_new(path, self.pos)     # 끝까지 써진 줄만 (반쯤 써진 줄은 다음에)
        except OSError:
            return
        lines = text.splitlines()
        if not lines:
            return
        self._server(lines)

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
            if active and hooks and not self.holding():
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
                self.on_change(prev, found, self.holding())
            except Exception as e:
                _log(f"바이옴 변경 처리 오류: {e}", "r")

        # 앱을 켜거나 이름을 바꾼 직후 읽은 기존 기록이면 알림 없이 현재 바이옴만 맞춤
        if fresh and prev is None:
            return
        if not (active and hooks):
            return
        safe = self.safe_on()
        uptime = fmt_uptime(time.time() - (self.active_since or time.time()))
        ps = (cfg.get("ps_link") or "").strip()
        if prev and prev in TARGET_BIOMES and prev != found and (not safe or prev == self.announced):
            send_all(hooks, biome_payload(prev, "ended", player, ps, uptime))
        if self.held and self.held != found:
            self.held = None                     # 붙잡아 둔 바이옴이 끝남 → 안 보냄
        if found in TARGET_BIOMES:
            if safe and self.holding():
                self.held = found
                _log(f"안전 모드 — {show_name(found)} 알림 대기 ({self.safe_left()}초 · K 로 건너뛰기)", "y")
                return
            self.announced = found
            send_all(hooks, biome_payload(found, "started", player, ps, uptime, mention_text(found, cfg)))
