# -*- coding: utf-8 -*-
"""
Prime Snipe [ Beta ] · AI 도우미 — 정해진 규칙으로 못 뚫는 봇 알림에서 어떤 버튼을 누를지 AI 에게 물어봄

- 사용자가 직접 넣은 API 키로 (키는 이 PC 의 config.json 에만 저장 · 로그에 안 남김)
- OpenAI 호환 방식 하나로: Groq (추천 · 가장 빠름) · Google Gemini · OpenRouter · 직접 입력
- 모델은 '자동'이면 그 키로 쓸 수 있는 모델 목록에서 가장 좋은 것을 골라 씀 (새 모델이 나오면 자동으로 그쪽으로)
- 보내는 것: 봇 메시지 글자 · 버튼 / 메뉴 이름 · 지금까지 누른 기록 (디스코드 로그인 정보 · 서버 목록 등은 안 보냄)
"""
import json
import re
import threading
import time
import urllib.error
import urllib.request

PROVIDERS = {
    "groq": {"name": "Groq", "base": "https://api.groq.com/openai/v1", "keys": "https://console.groq.com/keys",
             "note": "추천 · 가장 빠름 · 무료로 하루 약 1,000번"},
    "gemini": {"name": "Google Gemini", "base": "https://generativelanguage.googleapis.com/v1beta/openai",
               "keys": "https://aistudio.google.com/apikey", "note": "판단력이 더 좋음 · 무료 횟수는 적음"},
    "openrouter": {"name": "OpenRouter", "base": "https://openrouter.ai/api/v1", "keys": "https://openrouter.ai/keys",
                   "note": "여러 회사 모델을 한 키로 · 무료 모델 있음"},
    "custom": {"name": "직접 입력", "base": "", "keys": "", "note": "OpenAI 호환 주소"},
}

# 자동 모델 고르기: 앞에 있을수록 먼저 (이름에 들어 있으면) · 없으면 이름의 크기(…b) · 버전 숫자가 큰 것
PREFER = {
    "groq": ["gpt-oss-120b", "kimi-k2", "llama-4-maverick", "qwen3", "llama-3.3-70b", "gpt-oss-20b", "llama-4-scout", "llama-3.1-8b"],
    "gemini": ["flash"],
    "openrouter": [":free"],
    "custom": [],
}
SKIP_MODEL = re.compile(r"whisper|tts|orpheus|embed|guard|safeguard|moderation|audio|image|vision-preview|transcri|"
                        r"live|realtime|playai|compound|imagen|veo|aqa|-pro\b|pro-preview|lyria|robotics", re.I)
UA = "AcruxMacro"


class AIError(Exception):
    pass


def provider(conf):
    p = (conf or {}).get("provider") or "groq"
    return p if p in PROVIDERS else "groq"


def base_url(conf):
    p = provider(conf)
    b = PROVIDERS[p]["base"] if p != "custom" else str((conf or {}).get("base") or "").strip()
    return b.rstrip("/")


def _request(conf, path, body=None, timeout=12.0):
    key = str((conf or {}).get("key") or "").strip()
    base = base_url(conf)
    if not key:
        raise AIError("API 키가 없음")
    if not base.startswith("https://"):
        raise AIError("주소가 https:// 로 시작해야 함")
    headers = {"Authorization": f"Bearer {key}", "User-Agent": UA, "Accept": "application/json"}
    data = None
    if body is not None:
        data = json.dumps(body).encode("utf-8")
        headers["Content-Type"] = "application/json"
    if provider(conf) == "openrouter":
        headers["X-Title"] = "Acrux macro"
    req = urllib.request.Request(base + path, data=data, headers=headers, method="POST" if data else "GET")
    try:
        with urllib.request.urlopen(req, timeout=timeout) as r:
            return json.loads(r.read().decode("utf-8", "ignore") or "{}")
    except urllib.error.HTTPError as e:
        try:
            msg = json.loads(e.read().decode("utf-8", "ignore") or "{}")
            msg = (msg.get("error") or {}).get("message") if isinstance(msg.get("error"), dict) else msg.get("error") or msg.get("message")
        except Exception:
            msg = ""
        why = {401: "API 키가 올바르지 않음", 403: "이 키로는 사용할 수 없음", 404: "주소 또는 모델을 찾을 수 없음",
               429: "사용량 한도 초과 (잠시 후 다시 시도)"}.get(e.code, f"HTTP {e.code}")
        raise AIError(f"{why}" + (f" · {str(msg)[:80]}" if msg and e.code not in (401,) else ""))
    except (urllib.error.URLError, TimeoutError, OSError) as e:
        raise AIError(f"연결 실패 · {getattr(e, 'reason', e)}")
    except ValueError:
        raise AIError("응답을 읽을 수 없음")


_models_cache = {}
_lock = threading.Lock()


def models(conf, refresh=False):
    """이 키로 쓸 수 있는 글자 모델 ID 목록 (10분 동안은 다시 안 받음)"""
    ck = (provider(conf), base_url(conf), str((conf or {}).get("key") or "")[-6:])
    with _lock:
        hit = _models_cache.get(ck)
    if hit and not refresh and time.time() - hit[0] < 600:
        return hit[1]
    r = _request(conf, "/models", timeout=10)
    ids = []
    for m in (r.get("data") if isinstance(r, dict) else r) or []:
        mid = str((m or {}).get("id") or "") if isinstance(m, dict) else str(m)
        mid = mid[7:] if mid.startswith("models/") else mid
        if mid and not SKIP_MODEL.search(mid) and (m.get("active", True) if isinstance(m, dict) else True):
            ids.append(mid)
    with _lock:
        _models_cache[ck] = (time.time(), ids)
    return ids


def _size(mid):
    m = re.findall(r"(\d+(?:\.\d+)?)b\b", mid.lower())
    return max((float(x) for x in m), default=0.0)


def _ver(mid):
    return [float(x) for x in re.findall(r"\d+(?:\.\d+)?", mid)[:3]]


def best_model(prov, ids):
    """목록에서 이 일에 가장 좋은 모델 (추천) — 선호 이름 순서 → 같은 이름끼리는 버전이 큰 것 · 'lite' 는 뒤로"""
    ids = [i for i in ids if not SKIP_MODEL.search(i)]
    if not ids:
        return ""
    prefs = PREFER.get(prov) or []

    def score(i):
        low = i.lower()
        rank = next((n for n, p in enumerate(prefs) if p in low), len(prefs))
        lite = 1 if ("lite" in low or "mini" in low or "nano" in low) else 0
        preview = 1 if ("preview" in low or "exp" in low) else 0
        return (rank, lite, preview, [-v for v in _ver(low)], -_size(low))
    return sorted(ids, key=score)[0]


def pick_model(conf):
    m = str((conf or {}).get("model") or "").strip()
    if m:
        return m
    return best_model(provider(conf), models(conf))


def chat(conf, messages, timeout=12.0, max_tokens=300):
    model = pick_model(conf)
    if not model:
        raise AIError("쓸 수 있는 모델이 없음")
    body = {"model": model, "messages": messages, "temperature": 0, "max_tokens": max_tokens}
    t0 = time.time()
    r = _request(conf, "/chat/completions", body, timeout=timeout)
    try:
        text = r["choices"][0]["message"].get("content") or ""
    except (KeyError, IndexError, TypeError, AttributeError):
        raise AIError("응답 형식이 다름")
    return {"text": str(text), "model": model, "ms": int((time.time() - t0) * 1000)}


def parse_json(text):
    """AI 답에서 JSON 한 덩어리 꺼내기 (```json … ``` · 앞뒤 설명이 붙어도)"""
    t = re.sub(r"<think>.*?</think>", "", str(text or ""), flags=re.S)
    m = re.search(r"\{.*\}", t, re.S)
    if not m:
        return None
    try:
        return json.loads(m.group(0))
    except ValueError:
        try:
            return json.loads(re.sub(r",\s*([}\]])", r"\1", m.group(0)))
        except ValueError:
            return None


SYSTEM = (
    "You control an auto-clicker for Discord bot messages in a Roblox (Sol's RNG) community. "
    "Each alert hides a Roblox private-server link behind bot buttons or select menus. "
    "Decide the single next action that is most likely to reveal the link. "
    "Rules: follow instructions written in the messages (for example 'press this button first'); "
    "never pick buttons that report, delete, ban, unsubscribe, buy, pay, verify identity or change roles; "
    "do not repeat an action that already failed on the same message unless the bot asked to try again; "
    "if a message already contains the link (even written with spaces, [.] or other obfuscation), return it. "
    'Reply with JSON only: {"action":"press","board":B,"button":N} or '
    '{"action":"select","board":B,"menu":M,"option":O} or {"action":"link","url":"https://..."} or '
    '{"action":"wait","seconds":S} or {"action":"stop","reason":"short reason"}.'
)


def _board_text(i, b):
    lines = [f"Message {i} (from {b.get('author') or 'bot'}{', only you can see it' if b.get('ephemeral') else ''}):"]
    text = re.sub(r"\s+", " ", str(b.get("blob") or "")).strip()
    lines.append(f"  text: {text[:700] or '(no text)'}")
    for n, x in enumerate(b.get("buttons") or []):
        tag = " [disabled]" if x.get("disabled") else " [link button]" if x.get("url") else ""
        lines.append(f"  button {n}: {(x.get('label') or '').strip()} {x.get('emoji') or ''}{tag}".rstrip())
    for n, s in enumerate(b.get("selects") or []):
        opts = " | ".join(f"{k}: {o.get('label')}" for k, o in enumerate((s.get("options") or [])[:15]))
        lines.append(f"  menu {n} '{s.get('placeholder') or ''}': {opts}")
    return "\n".join(lines)


def decide(conf, boards, history, timeout=12.0):
    """보드(메시지) 목록 · 지금까지 한 일 → AI 의 다음 행동 dict (+ model · ms) · 실패하면 AIError"""
    parts = [_board_text(i, b) for i, b in enumerate(boards)]
    hist = "\n".join(f"- {h}" for h in history[-10:]) or "- nothing yet"
    user = "\n\n".join(parts) + f"\n\nWhat has been done so far:\n{hist}\n\nNext action as JSON:"
    r = chat(conf, [{"role": "system", "content": SYSTEM}, {"role": "user", "content": user}], timeout=timeout)
    act = parse_json(r["text"])
    if not isinstance(act, dict) or act.get("action") not in ("press", "select", "link", "wait", "stop"):
        raise AIError("AI 답을 이해하지 못함")
    act["_model"], act["_ms"] = r["model"], r["ms"]
    return act


def test(conf):
    """연결 확인: 모델 목록 → 추천 모델 → 짧은 질문 한 번"""
    ids = models(conf, refresh=True)
    rec = best_model(provider(conf), ids)
    use = str((conf or {}).get("model") or "").strip() or rec
    if not use:
        raise AIError("쓸 수 있는 모델이 없음")
    r = chat(dict(conf, model=use), [{"role": "user", "content": 'Reply with JSON only: {"ok":true}'}], timeout=15, max_tokens=20)
    return {"ok": True, "model": use, "recommended": rec, "models": ids[:80], "ms": r["ms"]}
