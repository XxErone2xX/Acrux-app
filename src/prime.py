# -*- coding: utf-8 -*-
"""
Prime Snipe [ Beta ] — 링크가 바로 없는 봇 알림에서 버튼(Get Link 등)을 대신 눌러 링크를 받아 접속

흐름: 봇 메시지 (감시 대상 · 버튼 있음 · 로블록스 링크 없음)
     → 필터 (바이옴 · 자동 제외 · 봇 제한) → 누를 버튼 · 메뉴 항목 고르기 (이름 · 이모지 · 우선순위 · 안 누를 이름)
     → 무작위 대기 (기본 2 ~ 3.5초) → 디스코드 화면에서 누르기
        (열려 있으면 바로 · 아니면 그 메시지로 이동해서 · 화면에서 못 찾으면 디스코드 내부 기능)
     → 봇 응답 기다리기 (나만 보이는 메시지 · 원래 메시지 수정 · '생각 중…' 뒤 수정 · 답장 · DM)
     → 응답에서 링크 찾기 (본문 · 임베드 · 링크 버튼 · 주소 안에 숨은 링크 · 다른 사이트 주소면 따라가서)
       버튼이 또 있으면 이어서 누르기 (최대 N단계) → 접속 (오토 스나이핑과 같은 접속 · 접속 중이면 건너뜀)
- 디스코드 토큰을 읽거나 디스코드 서버에 직접 요청하지 않음 — 버튼은 켜져 있는 디스코드 앱 화면에서 누름
- 링크 버튼(Join 등)은 누르지 않고 주소만 읽음 (로블록스 주소면 일반 스나이핑이 바로 처리)
"""
import html
import ipaddress
import json
import re
import secrets
import socket
import threading
import time
import urllib.parse as up
import urllib.request
from collections import deque

EPHEMERAL, LOADING = 64, 128
URL_RE = re.compile(r"https?://[^\s\"'<>()\[\]|`*]+", re.I)
# 따라가지 않는 주소 (디스코드 자체 · 첨부 파일)
NO_FOLLOW = re.compile(r"(^|\.)(discord\.com|discord\.gg|discordapp\.com|discordapp\.net|discord\.media)$", re.I)
# 응답이 실패를 뜻하는 말 (더 기다리지 않음)
FAIL_RE = re.compile(r"(don'?t|do not|doesn'?t) have (permission|access)|not allowed|no permission|missing (role|access)|"
                     r"expired|no longer|already (claimed|used|taken)|cooldown|try again (later|in)|rate.?limit|"
                     r"something went wrong|an error|권한이 없|만료|이미 (받|사용)|잠시 후|다시 시도", re.I)
SAFE_UA = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/124.0 Safari/537.36"


def _s(v):
    return None if v is None else str(v)


def _emoji(e):
    return str(e.get("name") or "") if isinstance(e, dict) else ""


def fields(m, dm=False, me=None):
    """디스코드 메시지(통신 형식) → Prime Snipe 가 쓰는 칸 (hook.js 의 primeFields 와 같은 모양)
    buttons: 버튼 (n = 메시지 안 버튼 순서) · selects: 글자 선택 메뉴 (고정 항목이 있는 것만)"""
    m = m if isinstance(m, dict) else {}
    author = m.get("author") if isinstance(m.get("author"), dict) else {}
    im = m.get("interaction_metadata") if isinstance(m.get("interaction_metadata"), dict) else {}
    it = m.get("interaction") if isinstance(m.get("interaction"), dict) else {}
    ref = m.get("message_reference") if isinstance(m.get("message_reference"), dict) else {}
    buttons, selects = [], []

    def walk(c, depth=0):
        if not isinstance(c, dict) or depth > 8:
            return
        t = c.get("type")
        if t == 2:
            buttons.append({"label": str(c.get("label") or ""), "emoji": _emoji(c.get("emoji")), "style": int(c.get("style") or 0),
                            "customId": _s(c.get("custom_id")), "url": c.get("url") or None,
                            "disabled": bool(c.get("disabled")), "n": len(buttons)})
        elif t == 3:
            opts = [{"label": str(o.get("label") or ""), "value": _s(o.get("value")), "description": str(o.get("description") or ""),
                     "emoji": _emoji(o.get("emoji"))} for o in (c.get("options") or []) if isinstance(o, dict)]
            selects.append({"customId": _s(c.get("custom_id")), "placeholder": str(c.get("placeholder") or ""),
                            "options": opts, "disabled": bool(c.get("disabled")), "n": len(selects)})
        for x in c.get("components") or []:
            walk(x, depth + 1)
        walk(c.get("accessory"), depth + 1)
    for c in m.get("components") or []:
        walk(c)
    try:
        flags = int(m.get("flags") or 0)
    except (TypeError, ValueError):
        flags = 0
    iuser = im.get("user") if isinstance(im.get("user"), dict) else (it.get("user") if isinstance(it.get("user"), dict) else {})
    return {
        "authorId": _s(author.get("id")), "bot": bool(author.get("bot")),
        "appId": _s(m.get("application_id") or m.get("webhook_id")),
        "flags": flags, "ephemeral": bool(flags & EPHEMERAL), "loading": bool(flags & LOADING),
        "interMsgId": _s(im.get("interacted_message_id")), "interUser": _s(iuser.get("id")),
        "refId": _s(ref.get("message_id")), "dm": bool(dm), "me": _s(me),
        "buttons": buttons, "selects": selects,
    }


# ---------------------------------------------------------------- 링크 찾기
def unwrap(text, link_re):
    """글자 · 주소 안의 로블록스 링크 (주소 안에 인코딩돼 숨은 것 · HTML 글자도: af_web_dp=https%3A%2F%2Fwww.roblox.com…)"""
    seen, t = [], str(text or "")
    for _ in range(4):
        if t in seen:
            break
        seen.append(t)
        m = link_re.search(t)
        if m:
            return m.group(0)
        t = html.unescape(up.unquote(t)).replace("\\/", "/")
    return None


def _public(host):
    """공개 인터넷 주소인지 (내 PC · 공유기 · 사내망 주소면 안 따라감)"""
    try:
        infos = socket.getaddrinfo(host, None)
    except Exception:
        return False
    try:
        return bool(infos) and all(ipaddress.ip_address(i[4][0]).is_global for i in infos)
    except ValueError:
        return False


class _Found(Exception):
    def __init__(self, url):
        super().__init__(url)
        self.url = url


_META_RE = re.compile(r"""(?:http-equiv=["']?refresh["']?[^>]*content=["'][^"']*url=|location(?:\.href)?\s*=\s*|window\.open\(\s*)["']?([^"'\s>;)]+)""", re.I)


def resolve(url, link_re, timeout=6.0, allow_local=False, max_bytes=262144):
    """로블록스가 아닌 주소 → 따라가서 로블록스 링크 찾기 (리다이렉트 · 짧은 주소 · 안내 페이지 안의 링크) · 못 찾으면 None
    https/http 공개 주소만 · 디스코드 주소는 안 따라감 · 쿠키 · 로그인 정보 없이"""
    hit = unwrap(url, link_re)
    if hit:
        return hit
    try:
        u = up.urlparse(str(url))
    except ValueError:
        return None
    host = (u.hostname or "").lower()
    if u.scheme not in ("http", "https") or not host or NO_FOLLOW.search(host):
        return None
    if not allow_local and not _public(host):
        return None

    class Redirect(urllib.request.HTTPRedirectHandler):
        max_redirections = 8

        def redirect_request(self, req, fp, code, msg, headers, newurl):
            found = unwrap(newurl, link_re)
            if found:
                raise _Found(found)                    # roblox:// 같은 주소로 보내면 거기서 바로 끝
            h = (up.urlparse(newurl).hostname or "").lower()
            if not allow_local and (not h or not _public(h)):
                return None
            return super().redirect_request(req, fp, code, msg, headers, newurl)

    opener = urllib.request.build_opener(Redirect)
    req = urllib.request.Request(url, headers={"User-Agent": SAFE_UA, "Accept": "text/html,application/xhtml+xml,*/*;q=0.8"})
    try:
        with opener.open(req, timeout=timeout) as r:
            hit = unwrap(r.geturl(), link_re)
            if hit:
                return hit
            ctype = (r.headers.get("Content-Type") or "").lower()
            if ctype and not any(x in ctype for x in ("text", "json", "javascript", "xml")):
                return None
            body = r.read(max_bytes).decode("utf-8", "ignore")
    except _Found as f:
        return f.url
    except Exception:
        return None
    hit = unwrap(body, link_re)
    if hit:
        return hit
    for m in _META_RE.finditer(body):             # 안내 페이지가 다른 주소로 보내는 경우 (한 번만 더)
        nxt = up.urljoin(url, html.unescape(m.group(1)))
        if nxt != url:
            return resolve(nxt, link_re, timeout, allow_local, max_bytes) if nxt.startswith("http") else unwrap(nxt, link_re)
    return None


def urls_in(d):
    """메시지 안의 주소 (로블록스 아닌 것 포함) — 링크 버튼 주소 먼저"""
    out = [b.get("url") for b in d.get("buttons") or [] if b.get("url")]
    out += URL_RE.findall(d.get("blob") or "")
    return list(dict.fromkeys(u.rstrip(".,!?;:'\"") for u in out if u))


def snippet(d, n=70):
    t = re.sub(r"\s+", " ", (d or {}).get("blob") or "").strip()
    return t if len(t) <= n else t[:n - 1] + "…"


# ---------------------------------------------------------------- 누를 것 고르기
def _rank(text, words):
    t = text.lower()
    for i, w in enumerate(words):
        if w and w.lower() in t:
            return i
    return None


def choose(d, conf, done=()):
    """누를 것 목록 (먼저 해 볼 순서): {"kind": link(주소만 읽음) · button · select, ...}
    - 안 누를 이름이 들어 있거나 · 꺼져 있거나 · 이미 누른 것은 빼고
    - 이름 · 이모지가 누를 이름 목록에 맞는 것 (앞에 있는 이름일수록 먼저) · 같으면 링크 버튼(누르지 않아도 됨) 먼저
    - 맞는 게 없고 누를 수 있는 버튼이 하나뿐이면 그 버튼 (설정)"""
    words, skip = conf.get("words") or [], [s.lower() for s in conf.get("skip") or [] if s]
    out, plain = [], []
    for b in d.get("buttons") or []:
        text = f"{b.get('label', '')} {b.get('emoji', '')}".strip()
        if b.get("disabled") or b.get("style") == 6 or any(s in text.lower() for s in skip):
            continue
        key = ("button", b.get("n"), b.get("customId") or b.get("label"))
        if b.get("url"):
            rank = _rank(text, words)
            out.append((rank if rank is not None else len(words) + 1, 0, {"kind": "link", "url": b["url"], "label": b.get("label", ""),
                                                                         "emoji": b.get("emoji", ""), "key": key}))
            continue
        if not b.get("customId") or key in done:
            continue
        rank = _rank(text, words)
        t = {"kind": "button", "label": b.get("label", ""), "emoji": b.get("emoji", ""), "n": b.get("n"),
             "same": sum(1 for x in d["buttons"][:b.get("n", 0)] if x.get("label") == b.get("label") and x.get("emoji") == b.get("emoji")),
             "customId": b.get("customId"), "key": key}
        if rank is None:
            plain.append(t)
        else:
            out.append((rank, 1, t))
    if conf.get("selects", True):
        for s in d.get("selects") or []:
            if s.get("disabled") or not s.get("customId"):
                continue
            best = None
            for o in s.get("options") or []:
                text = f"{o.get('label', '')} {o.get('emoji', '')} {o.get('description', '')}".strip()
                if any(x in text.lower() for x in skip):
                    continue
                r = _rank(text, words)
                if r is not None and (best is None or r < best[0]):
                    best = (r, o)
            key = ("select", s.get("n"), best[1].get("value") if best else None)
            if best and key not in done:
                out.append((best[0], 2, {"kind": "select", "n": s.get("n"), "placeholder": s.get("placeholder", ""),
                                         "option": best[1].get("label", ""), "value": best[1].get("value"),
                                         "customId": s.get("customId"), "label": best[1].get("label", ""), "emoji": "", "key": key}))
    out.sort(key=lambda x: (x[0], x[1]))
    targets = [t for _, _, t in out]
    # 이름이 안 맞는 링크 버튼은 맨 뒤 (주소만 읽어 보는 것이라 손해 없음) · 버튼이 하나뿐이면 그것도
    if conf.get("single", True) and len(plain) == 1 and not any(t["kind"] in ("button", "select") for t in targets):
        targets.append(plain[0])
    return targets


def target_name(t):
    if not t:
        return ""
    if t["kind"] == "select":
        return f"{t.get('placeholder') or '메뉴'} → {t.get('option')}"
    return t.get("label") or t.get("emoji") or ("링크 버튼" if t["kind"] == "link" else "버튼")


# ---------------------------------------------------------------- 실행기
class Job:
    def __init__(self, d, test=False):
        self.d = d                      # 원래 알림 (바이옴 · 자동 제외 · 들어가서 바이옴 확인 힌트는 이 글자로)
        self.cur = d                    # 지금 누를 버튼이 있는 메시지 (여러 단계면 봇 응답)
        self.test = test                # 테스트: 링크만 받고 접속 안 함
        self.inbox = deque()
        self.ix = deque()               # 버튼을 누른 결과 신호 (INTERACTION_*)
        self.ev = threading.Event()
        self.watch = {d["id"]}          # 이 메시지들의 수정 · 응답을 받음
        self.seen = set()               # 같은 응답이 여러 방식(통신 · 내부)으로 와도 한 번만
        self.done = set()               # 이미 누른 것 (같은 버튼을 또 누르지 않게)
        self.clicked_at = 0.0
        self.navigated = False
        self.rec = None                 # 최근 목록의 한 줄 (화면 표시)


class PrimeSniper:
    RETRY_AFTER = 4.0                   # 누른 뒤 이 시간 동안 아무 반응(보냄 신호 · 응답)도 없으면 다른 방식으로 한 번 더

    def __init__(self, cfg, core, handler=None):
        self.cfg, self.core, self.handler = cfg, core, handler
        self.lock = threading.RLock()
        self.jobs = deque()
        self.handled = deque(maxlen=2000)
        self.by_msg = {}                # 메시지 ID → 응답을 기다리는 Job
        self.current = None
        self.clicks = deque()
        self.recent = deque(maxlen=25)
        self.wake = threading.Event()
        self.thread = None
        self.stats = {"pressed": 0, "links": 0}
        self.hook_on = None             # hook.js 에 알려 준 켜짐 상태

    # ---- 설정 · 상태
    def conf(self):
        try:
            return self.cfg["prime"] or {}
        except Exception:
            return {}

    def enabled(self):
        return bool(self.conf().get("enabled"))

    def want_extra(self):
        """링크가 없는 메시지도 받아야 하는지 (켜져 있거나 · 응답을 기다리는 중)"""
        return self.enabled() or self.current is not None

    def snapshot(self):
        with self.lock:
            rec = [{k: v for k, v in r.items() if k != "info"} for r in reversed(self.recent)]
        return {"enabled": self.enabled(), "busy": self.current is not None, "recent": rec, **self.stats}

    def _record(self, d, status, target=None, note=""):
        r = {"id": d.get("id"), "time": time.time(), "where": self._where(d), "author": d.get("author") or "",
             "buttons": [b.get("label") or b.get("emoji") or ("링크" if b.get("url") else "?") for b in d.get("buttons") or []]
                        + [f"[{s.get('placeholder') or '메뉴'}]" for s in d.get("selects") or []],
             "target": target_name(target), "status": status, "note": note, "url": "", "info": d}
        with self.lock:
            self.recent.append(r)
        return r

    @staticmethod
    def _where(d):
        g = d.get("guildName") or d.get("guildId") or "DM"
        ch = d.get("channelName") or d.get("channelId")
        return f"{g} #{ch}"

    def _log(self, msg, color="c"):
        self.core.log(f"[Prime] {msg}", color)

    # ---- 메시지 받기 (연결 루프에서)
    def route(self, d):
        """누른 버튼에 대한 봇 응답이면 그 일에 넘기고 True (일반 스나이핑은 이 메시지를 처리하지 않음)"""
        job = self.current
        if job is None or not d.get("id"):
            return False
        mid, ev = d["id"], d.get("event") or "MESSAGE_CREATE"
        if mid == job.d["id"] and ev == "MESSAGE_CREATE":
            return False                            # 원래 알림이 다른 방식으로 또 들어온 것
        mine = False
        if d.get("interMsgId") and d["interMsgId"] in job.watch:
            mine = True                             # 누른 메시지에 대한 응답 (가장 확실)
        elif d.get("refId") and d["refId"] in job.watch:
            mine = True                             # 그 메시지에 답장
        elif mid in job.watch and ev == "MESSAGE_UPDATE":
            mine = True                             # 원래 메시지 · 응답이 수정됨 (생각 중… → 결과)
        elif job.clicked_at and time.time() - job.clicked_at < float(self.conf().get("timeout", 15)) + 2:
            bot = {job.cur.get("authorId"), job.cur.get("appId"), job.d.get("authorId"), job.d.get("appId")} - {None}
            from_bot = bool(bot & {d.get("authorId"), d.get("appId")})
            if from_bot and d.get("ephemeral") and d.get("channelId") == job.cur.get("channelId"):
                mine = True                         # 나만 보이는 메시지 (나한테만 오는 것)
            elif from_bot and d.get("dm") and self.conf().get("dm", True):
                mine = True                         # 봇이 DM 으로 보냄
            elif from_bot and d.get("interUser") and d.get("interUser") == d.get("me") and not d.get("interMsgId"):
                mine = True                         # 내가 누른 것에 대한 응답 (누른 메시지 표시가 없는 형식)
        if not mine:
            return False
        key = (mid, ev, hash(d.get("blob") or ""), len(d.get("buttons") or []))
        with self.lock:
            job.watch.add(mid)
            if key not in job.seen:
                job.seen.add(key)
                job.inbox.append(d)
        job.ev.set()
        return True

    def interaction(self, t, d=None):
        """버튼 누른 결과 신호 (디스코드 통신 · 내부): INTERACTION_CREATE(보냄) · SUCCESS(봇이 받음) · FAILURE(실패) · MODAL_CREATE(입력 창)"""
        job = self.current
        if job is not None and job.clicked_at and str(t or "").startswith("INTERACTION_"):
            job.ix.append(str(t))
            job.ev.set()

    def candidate(self, d, age_ms=0.0, old=False):
        """링크는 없지만 봇 버튼 · 메뉴가 있는 메시지 → 조건이 맞으면 누르기 예약"""
        if not (d.get("buttons") or d.get("selects")) or not d.get("id"):
            return
        core, cfg, conf = self.core, self.cfg, self.conf()
        if d.get("thread") or old:
            return
        targeted = d.get("channelId") in cfg["channel_ids"] or (d.get("guildId") and d["guildId"] in cfg["guild_ids"])
        if not targeted:
            return
        with self.lock:
            if d["id"] in self.handled:
                return
            self.handled.append(d["id"])
        if d.get("authorId") and not d.get("bot") and not d.get("appId"):
            return                                  # 사람이 보낸 메시지 (버튼은 봇만 붙일 수 있음)
        bots = [b.lower() for b in conf.get("bots") or []]
        if bots and not any(b in (d.get("authorId") or "", (d.get("author") or "").lower(), d.get("appId") or "") or
                            (len(b) > 2 and b in (d.get("author") or "").lower()) for b in bots):
            self._record(d, "봇 제한")
            return
        text = (d.get("blob") or "").lower()
        if cfg["keywords"] and not any(core.has_keyword(text, k) for k in cfg["keywords"]):
            self._record(d, "바이옴 불일치")
            return
        if any(core.has_word(text, w) for w in cfg["ignore_words"]):
            self._record(d, "제외됨")
            return
        targets = choose(d, conf)
        if not targets:
            self._record(d, "누를 버튼 없음")
            return
        if not conf.get("enabled"):
            self._record(d, "꺼짐", targets[0])
            return
        if not cfg["enabled"] or not core.ARMED.is_set():
            self._record(d, "대기", targets[0], "오토 스나이핑을 시작하면 누름")
            return
        why = self._busy()
        if why:
            self._record(d, "건너뜀", targets[0], why)
            return
        job = Job(d)
        job.rec = self._record(d, "누르기 대기", targets[0], f"{age_ms / 1000:.1f}초 전 알림")
        self._enqueue(job)

    def test(self, mid):
        """최근 목록의 메시지로 지금 해 보기 (접속은 안 함 · 받은 링크만 보여 줌)"""
        with self.lock:
            r = next((x for x in reversed(self.recent) if x["id"] == mid), None)
        if not r:
            return "최근 목록에 없는 메시지"
        if self.current is not None:
            return "다른 메시지를 누르는 중"
        if not choose(r["info"], self.conf()):
            return "누를 버튼 없음"
        job = Job(r["info"], test=True)
        job.rec = self._record(r["info"], "테스트 대기", choose(r["info"], self.conf())[0])
        self._enqueue(job)
        return ""

    def _busy(self):
        core = self.core
        if core.JOIN.get("pending"):
            return "다른 링크로 접속하는 중"
        try:
            if core.Bus.busy():
                return "스나이핑한 서버에서 할 일이 도는 중"
        except Exception:
            pass
        return ""

    def _enqueue(self, job):
        with self.lock:
            if len(self.jobs) >= 3:
                old = self.jobs.popleft()
                self._set(old, "건너뜀", "밀린 알림이 많아서")
            self.jobs.append(job)
            if not self.thread or not self.thread.is_alive():
                self.thread = threading.Thread(target=self._worker, daemon=True)
                self.thread.start()
        self.wake.set()

    def _set(self, job, status, note=None, url=None, raw=False):
        """최근 목록 한 줄 바꾸기 · raw: note 가 봇이 보낸 글자 그대로 (번역 안 함)"""
        if job and job.rec is not None:
            job.rec["status"] = status
            if note is not None:
                job.rec["note"] = note
                job.rec["raw"] = bool(raw)
            if url:
                job.rec["url"] = url

    # ---- 일하는 스레드 (한 번에 하나씩 · 화면 이동이 겹치지 않게)
    def _worker(self):
        while not self.core.STOP.is_set():
            with self.lock:
                job = self.jobs.popleft() if self.jobs else None
            if job is None:
                self.wake.wait(5)
                self.wake.clear()
                with self.lock:
                    if not self.jobs:
                        return
                continue
            try:
                self._run(job)
            except Exception as e:
                self._set(job, "오류", str(e)[:80], raw=True)
                self._log(f"오류: {e}", "r")
            finally:
                self._end(job)

    def _end(self, job):
        self.current = None
        self.core.gateway_extra(until=0.0)
        self._hook("window.__lwPrimeWatch = 0")
        if job.navigated and self.conf().get("go_back", True):
            self._eval("window.__lwPrime ? window.__lwPrime.restore() : null", 8)

    def _alive(self, job):
        """기다리는 사이 그만둘 이유 (없으면 빈 글자)"""
        if self.core.STOP.is_set():
            return "종료"
        if job.test:
            return ""
        if not self.enabled():
            return "Prime Snipe 꺼짐"
        if not self.cfg["enabled"] or not self.core.ARMED.is_set():
            return "오토 스나이핑 중지됨"
        return self._busy()

    def _sleep(self, job, sec):
        end = time.time() + sec
        while time.time() < end:
            why = self._alive(job)
            if why:
                return why
            time.sleep(0.05)
        return ""

    def _rate_ok(self):
        now = time.time()
        per = int(self.conf().get("per_min", 6))
        while self.clicks and now - self.clicks[0] > 60:
            self.clicks.popleft()
        return len(self.clicks) < per

    def _run(self, job):
        conf = self.conf()
        d = job.d
        targets = choose(d, conf)
        name = target_name(targets[0]) if targets else ""
        self._log(f"{self._where(d)} — 봇 알림 · {d.get('author') or '봇'} · 누를 것: {name or '없음'}"
                  + (" (테스트)" if job.test else ""), "c")
        # 1) 링크 버튼: 누르지 않고 주소만 따라가 봄
        for t in [t for t in targets if t["kind"] == "link"]:
            if conf.get("resolve", True):
                self._set(job, "주소 확인 중", target_name(t))
                url = resolve(t["url"], self.core.LINK_RE)
                if url:
                    return self._finish(job, url, f"링크 버튼 '{target_name(t)}' 주소")
        # 2) 버튼 · 메뉴 누르기 (응답에 버튼이 또 있으면 이어서)
        presses = [t for t in targets if t["kind"] != "link"]
        if not presses:
            return self._fail(job, "링크 없음", "링크 버튼 주소에서 로블록스 링크를 못 찾음")
        target, step, steps = presses[0], 1, int(conf.get("steps", 3))
        while True:
            lo, hi = float(conf.get("delay_min", 2.0)), float(conf.get("delay_max", 3.5))
            wait = secrets.SystemRandom().uniform(lo, max(lo, hi))
            self._set(job, "누르기 대기", f"{wait:.1f}초 뒤 '{target_name(target)}'" + (f" ({step}단계)" if step > 1 else ""))
            why = self._sleep(job, wait)
            if why:
                return self._fail(job, "취소", why, event=False)
            if not job.test and not self._rate_ok():
                return self._fail(job, "건너뜀", f"1분에 {conf.get('per_min', 6)}번까지만 누름", event=False)
            res = self._press(job, target, step)
            if not res.get("ok"):
                return self._fail(job, "누르기 실패", res.get("note") or "")
            out = self._await(job, float(conf.get("timeout", 15)), target, res)
            if out.get("url"):
                return self._finish(job, out["url"], out.get("how") or f"'{target_name(target)}' 응답")
            if out.get("error"):
                return self._fail(job, out["error"], out.get("note") or "", raw=out.get("raw", False))
            resp = out.get("resp")
            if not resp:
                return self._fail(job, "응답 없음", f"{conf.get('timeout', 15)}초 동안 봇이 답하지 않음")
            # 응답 안의 다른 주소 (짧은 주소 · 안내 페이지)
            if conf.get("resolve", True):
                for u in urls_in(resp)[:4]:
                    self._set(job, "주소 확인 중", u[:60])
                    url = resolve(u, self.core.LINK_RE)
                    if url:
                        return self._finish(job, url, "응답 속 주소")
            # 버튼이 또 있으면 다음 단계 (같은 메시지에서 이미 누른 버튼은 다시 안 누름)
            job.done.add((job.cur.get("id"), target["key"]))
            done_here = {k for m, k in job.done if m == resp.get("id")}
            nxt = [t for t in choose(resp, conf, done_here) if t["kind"] != "link"]
            if nxt and step < steps:
                job.cur, target, step = resp, nxt[0], step + 1
                self._log(f"응답에 버튼이 또 있음 → '{target_name(target)}' ({step}단계)", "d")
                continue
            return self._fail(job, "링크 없음", snippet(resp), raw=True)

    # ---- 누르기
    def _eval(self, expr, timeout=10):
        r = self.core.cdp_request("Runtime.evaluate", {"expression": expr, "awaitPromise": True, "returnByValue": True},
                                  timeout=timeout)
        if not isinstance(r, dict):
            return None
        if r.get("exceptionDetails"):
            return {"ok": False, "reason": "script-error: " + str((r["exceptionDetails"] or {}).get("text"))[:80]}
        return (r.get("result") or {}).get("value")

    def _hook(self, expr):
        self.core.cdp_request("Runtime.evaluate", {"expression": expr}, timeout=0)

    def _req(self, msg, target):
        conf = self.conf()
        return {"channelId": msg.get("channelId"), "guildId": msg.get("guildId"), "messageId": msg.get("id"),
                "kind": target["kind"], "label": target.get("label", ""), "emoji": target.get("emoji", ""),
                "n": target.get("n", 0), "same": target.get("same", 0), "placeholder": target.get("placeholder", ""),
                "option": target.get("option", ""), "value": target.get("value"), "customId": target.get("customId"),
                "appId": msg.get("appId") or msg.get("authorId"), "flags": msg.get("flags", 0),
                "navigate": bool(conf.get("navigate", True))}

    def _trusted_click(self, rect):
        """진짜 마우스 클릭처럼 (디스코드 창 안에서만 · 실제 마우스 커서는 안 움직임) — 버튼 안 무작위 점"""
        rnd = secrets.SystemRandom()
        x = rect["x"] + rect["w"] * rnd.uniform(0.3, 0.7)
        y = rect["y"] + rect["h"] * rnd.uniform(0.3, 0.7)
        send = self.core.cdp_request
        send("Input.dispatchMouseEvent", {"type": "mouseMoved", "x": x - rnd.uniform(4, 14), "y": y + rnd.uniform(3, 9)}, timeout=3)
        time.sleep(rnd.uniform(0.04, 0.09))
        send("Input.dispatchMouseEvent", {"type": "mouseMoved", "x": x, "y": y}, timeout=3)
        time.sleep(rnd.uniform(0.05, 0.12))
        r1 = send("Input.dispatchMouseEvent", {"type": "mousePressed", "x": x, "y": y, "button": "left", "buttons": 1, "clickCount": 1}, timeout=3)
        time.sleep(rnd.uniform(0.06, 0.13))
        r2 = send("Input.dispatchMouseEvent", {"type": "mouseReleased", "x": x, "y": y, "button": "left", "buttons": 0, "clickCount": 1}, timeout=3)
        return isinstance(r1, dict) and isinstance(r2, dict) and "error" not in r1 and "error" not in r2

    def _press(self, job, target, step):
        """누르기 → {"ok", "method", "note"}
        signal (기본) · 누른 것처럼: 디스코드 앱이 '이 버튼 눌렀음' 신호를 보냄 — 화면 이동 · 클릭 없음 (토큰은 앱 안에서만)
        screen · 화면에서: 보이면 진짜 클릭처럼 → 안 되면 화면 클릭(대신) · 그 채널이 안 열려 있으면 메시지로 이동해서
        고른 방식이 안 되면 다른 방식으로 (설정)"""
        conf = self.conf()
        msg = job.cur
        q = self._req(msg, target)
        q["sessionId"] = (getattr(self.core, "GW_STATE", None) or {}).get("session")
        # 응답을 놓치지 않게 누르기 전에 기다리기 시작
        until = time.time() + float(conf.get("timeout", 15)) + 5
        self.current = job
        self.core.gateway_extra(until=until)
        self._hook(f"window.__lwPrimeWatch = Date.now() + {int((until - time.time()) * 1000)}")
        js = json.dumps(q, ensure_ascii=False)
        order = ["signal", "screen"] if conf.get("mode", "signal") == "signal" else ["screen", "signal"]
        if not conf.get("fallback", True):
            order = order[:1]
        notes = []
        for way in order:
            if way == "signal":
                if not q.get("customId"):
                    notes.append("버튼 ID 없음")
                    continue
                job.clicked_at = time.time()
                job.ix.clear()
                r = self._eval(f"window.__lwPrime ? window.__lwPrime.signal({js}) : null", 12) or {}
                if not r.get("ok"):
                    rs = str(r.get("reason") or "디스코드 연결 안 됨")
                    notes.append({"no-rest": "누른 것처럼: 디스코드 내부 통신 기능을 못 찾음",
                                  "no-session": "누른 것처럼: 세션 정보 없음"}.get(rs, f"누른 것처럼: {rs}"))
                    continue
                self._pressed(job, target, step, "누른 것처럼")
                return {"ok": True, "method": "signal"}
            # 화면에서 누르기
            prep = self._eval(f"window.__lwPrime ? window.__lwPrime.prepare({js}) : null", 12) or {}
            if prep.get("navigated"):
                job.navigated = True
            if not prep.get("ok"):
                notes.append({"no-message": "화면: 메시지를 못 찾음 (채널 이동 실패)", "no-button": "화면: 버튼을 못 찾음",
                              "no-select": "화면: 메뉴를 못 찾음", "disabled": "화면: 버튼이 꺼져 있음"}
                             .get(prep.get("reason"), f"화면: {prep.get('reason') or '디스코드 연결 안 됨'}"))
                continue
            ways = (["trusted"] if target["kind"] == "button" and prep.get("visible") and prep.get("hit") and prep.get("rect") else []) + ["dom"]
            for i, mth in enumerate(ways):
                job.clicked_at = time.time()
                job.ix.clear()
                if mth == "trusted":
                    ok = self._trusted_click(prep["rect"])
                else:
                    fn = "choose" if target["kind"] == "select" else "click"
                    r = self._eval(f"window.__lwPrime ? window.__lwPrime.{fn}({js}) : null", 10) or {}
                    ok = bool(r.get("ok"))
                    if not ok:
                        notes.append(f"화면: {r.get('reason') or '누르기 실패'}")
                if not ok:
                    continue
                self._pressed(job, target, step, ("화면 클릭" if mth == "trusted" else "화면 클릭 (대신)")
                              + (" · 이동해서" if prep.get("navigated") else ""))
                # 진짜 클릭처럼 누른 게 안 먹혔으면(보냄 신호 · 응답이 없음) 화면 클릭(대신)으로 한 번 더
                if i + 1 < len(ways) and not self._reacted(job, self.RETRY_AFTER):
                    self._log("반응 없음 — 다른 방식으로 다시 누름", "d")
                    continue
                return {"ok": True, "method": mth}
        return {"ok": False, "note": " · ".join(dict.fromkeys(notes)) or "누르기 실패"}

    def _pressed(self, job, target, step, how):
        with self.lock:
            self.clicks.append(time.time())
        self.stats["pressed"] += 1
        self._set(job, "응답 기다리는 중", f"'{target_name(target)}' · {how}" + (f" · {step}단계" if step > 1 else ""))
        self._log(f"'{target_name(target)}' 누름 ({how}{f' · {step}단계' if step > 1 else ''})", "c")

    def _reacted(self, job, sec):
        end = time.time() + sec
        while time.time() < end:
            if job.ix or job.inbox:
                return True
            job.ev.wait(0.1)
            job.ev.clear()
        return bool(job.ix or job.inbox)

    # ---- 응답 기다리기
    def _await(self, job, timeout, target, res):
        """봇 응답 → {"url"} · {"error", "note"} · {"resp"} (링크 없는 응답) · {} (응답 없음)"""
        end = time.time() + timeout
        last_resp, changed = None, 0.0
        while time.time() < end:
            if self.core.STOP.is_set():
                return {"error": "취소", "note": "종료"}
            while job.ix:
                t = job.ix.popleft()
                if t == "INTERACTION_FAILURE":
                    return {"error": "봇 응답 실패", "note": "This interaction failed (봇이 꺼져 있거나 늦음)"}
                if t == "INTERACTION_MODAL_CREATE":
                    self._eval("window.__lwPrime ? window.__lwPrime.escape() : null", 4)
                    return {"error": "입력 창", "note": "봇이 입력 창(모달)을 띄워서 자동으로 못 함"}
            while job.inbox:
                r = job.inbox.popleft()
                url = unwrap(r.get("blob") or "", self.core.LINK_RE)
                if url:
                    how = "DM" if r.get("dm") else "나만 보이는 메시지" if r.get("ephemeral") else \
                        "원래 메시지 수정" if r.get("id") == job.d.get("id") else "봇 응답"
                    return {"url": url, "how": how}
                if r.get("loading") and not (r.get("blob") or "").strip():
                    continue                             # '생각 중…' — 수정될 때까지 기다림
                if (r.get("blob") or "").strip() or r.get("buttons") or r.get("selects"):
                    last_resp, changed = r, time.time()
                    if FAIL_RE.search(r.get("blob") or "") and not (r.get("buttons") or r.get("selects")):
                        return {"error": "봇이 거절", "note": snippet(r), "raw": True}
            # 내용 있는 응답이 왔고 잠깐(1.5초) 더 바뀌는 게 없으면 그걸로 판단
            if last_resp is not None and time.time() - changed > 1.5:
                return {"resp": last_resp}
            job.ev.wait(0.2)
            job.ev.clear()
        return {"resp": last_resp} if last_resp is not None else {}

    # ---- 끝
    def _finish(self, job, url, how):
        core, cfg, d = self.core, self.cfg, job.d
        self.stats["links"] += 1
        url = core.to_weblink(url) or url             # 깔끔한 주소로 (주소 안에 숨어 있던 링크 · 딥링크도)
        mine = core.own_codes(cfg)
        if mine and core.link_codes(url) & mine:
            self._set(job, "내 서버 링크", how, url)
            self._log("받은 링크가 내 브섭 링크 — 접속 안 함", "d")
            return
        if job.test:
            self._set(job, "링크 받음", f"테스트라 접속 안 함 · {how}", url)
            self._log(f"테스트 — 링크 받음 ({how}) → {url}", "g")
            return
        entry = {"time": time.time(), "url": url, "via": "prime", "age": max(0.0, time.time() - core.snowflake_ms(d["id"]) / 1000),
                 "guildId": d.get("guildId"), "channelId": d.get("channelId"), "parentId": d.get("parentId"),
                 "guildName": d.get("guildName") or "", "channelName": d.get("channelName") or "",
                 "author": d.get("author") or "", "where": self._where(d) + " · Prime", "status": ""}
        if not cfg["enabled"] or not core.ARMED.is_set():
            entry["status"] = "대기"
        else:
            entry["status"] = core.snipe_skip(url, cfg["snipe"])
        if not entry["status"]:
            entry["status"] = "열림"
            if self.handler is not None:
                self.handler.last_fire = time.time()
                self.handler.count += 1
            core.join_server(url, cfg["stable_join"], cfg["snipe"], hint=core.biome.biome_hint(d.get("blob") or ""))
            if cfg["beep"]:
                core.beep()
        self._set(job, entry["status"], how, url)
        core.log(f"[{entry['status']}] {entry['where']}  → {url}  (Prime · {how})", "g" if entry["status"] == "열림" else "d")
        try:
            core.Bus.on_event(entry)
        except Exception:
            pass

    def _fail(self, job, status, note="", event=True, raw=False):
        d = job.d
        self._set(job, status, note, raw=raw)
        self._log(f"{self._where(d)} — {status}{' · ' + note if note else ''}", "y" if event else "d")
        if event and not job.test:
            entry = {"time": time.time(), "url": "", "via": "prime", "age": 0.0, "guildId": d.get("guildId"),
                     "channelId": d.get("channelId"), "parentId": d.get("parentId"), "guildName": d.get("guildName") or "",
                     "channelName": d.get("channelName") or "", "author": d.get("author") or "",
                     "where": self._where(d) + " · Prime", "status": status, "note": note, "raw": raw}
            try:
                self.core.Bus.on_event(entry)
            except Exception:
                pass
