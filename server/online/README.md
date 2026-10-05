# Acrux 사용자 수 서버

Acrux 가 켜져 있는 사람 수를 세는 작은 서버예요. Cloudflare Workers 무료 요금제로 돌아가요.

## 올리는 방법 (한 번만)

1. https://dash.cloudflare.com 에서 계정을 만들고 로그인
2. 이 폴더(`server/online`)에서:
   ```
   npx wrangler login
   npx wrangler deploy
   ```
3. 끝나면 `https://acrux-online.<계정이름>.workers.dev` 같은 주소가 나옴
4. 그 주소를 `src/app.py` 의 `ONLINE_URL` 에 넣고 새 버전을 올리면 앱에 사용자 수가 뜸

## 동작

- `POST /ping {"id": "<32자리 무작위 번호>"}` → `{"online": 사용자 수, "next": 다음 신호까지 초}`
- `GET /count` → `{"online": 사용자 수}` (집계에 참여하지 않는 사람용 · 세기만 함)
- 최근 6분 안에 신호를 보낸 번호 수를 셈 · 무작위 번호 말고는 저장하지 않음

## 무료 한도

Workers · Durable Objects 무료 요금제는 하루 10만 요청까지예요. 5분마다 신호를 보내면 동시에 약 300명까지 넉넉해요.
사람이 더 많아지면 `worker.js` 의 `NEXT_SEC` 를 늘리면 돼요 (앱은 서버가 알려준 간격을 따름).
