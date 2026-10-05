// Acrux 사용자 수 서버 — Cloudflare 대시보드에 그대로 붙여넣는 버전 (D1 데이터베이스를 DB 라는 이름으로 연결)
// POST /ping {"id": 32자리 무작위 번호} → {"online": 사용자 수, "next": 다음 신호까지 초}
// GET /count → {"online": 사용자 수} · 무작위 번호와 마지막 신호 시각 말고는 저장하지 않음
const ACTIVE_MS = 6 * 60 * 1000;   // 6분 안에 신호가 있으면 사용 중
const NEXT_SEC = 300;              // 앱이 다음 신호를 보낼 때까지 (초)
const ID_RE = /^[0-9a-f]{32}$/;
let ready = false, cache = { n: 0, at: 0 }, pruned = 0;

export default {
  async fetch(req, env) {
    const url = new URL(req.url), now = Date.now();
    if (!ready) {
      await env.DB.prepare("CREATE TABLE IF NOT EXISTS seen (id TEXT PRIMARY KEY, t INTEGER)").run();
      await env.DB.prepare("CREATE INDEX IF NOT EXISTS seen_t ON seen (t)").run();
      ready = true;
    }
    if (url.pathname === "/ping" && req.method === "POST") {
      let id = "";
      try { id = String((await req.json()).id || ""); } catch (e) {}
      if (!ID_RE.test(id)) return json({ error: "bad id" }, 400);
      await env.DB.prepare("INSERT INTO seen (id, t) VALUES (?1, ?2) ON CONFLICT(id) DO UPDATE SET t = ?2").bind(id, now).run();
    } else if (!(url.pathname === "/count" && req.method === "GET")) {
      return json({ error: "not found" }, 404);
    }
    if (now - pruned > 10 * 60 * 1000) {          // 오래된 번호는 10분마다 지움
      pruned = now;
      await env.DB.prepare("DELETE FROM seen WHERE t < ?1").bind(now - ACTIVE_MS).run();
    }
    if (now - cache.at > 60 * 1000) {             // 세는 건 1분에 한 번만 (무료 한도 아끼기)
      const r = await env.DB.prepare("SELECT COUNT(*) AS n FROM seen WHERE t > ?1").bind(now - ACTIVE_MS).first();
      cache = { n: r ? r.n : 0, at: now };
    }
    return json({ online: cache.n, next: NEXT_SEC });
  },
};

function json(obj, status = 200) {
  return new Response(JSON.stringify(obj), {
    status, headers: { "content-type": "application/json", "cache-control": "no-store" },
  });
}
