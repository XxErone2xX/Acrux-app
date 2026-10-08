## Acrux macro V1.34.0 업데이트
- 매크로 기능을 상시 · 단일성 · 긴급 기능으로 나눠서 정리
  - 긴급: 레어 바이옴 자동 팝핑 — 다른 기능보다 먼저
  - 상시: 자동 낚시 — 다른 기능이 자리를 쓰는 동안은 비켜 줌
  - 단일성: 상인 자동 구매 · 포션 자동 제작 · 오토 메모리 매치 · 오토 아이템 사용 — 한 번에 하나만, 먼저 온 순서대로
  - 여러 기능이 동시에 자동 낚시를 멈춰 달라고 해도, 전부 끝나야 낚시를 이어가도록 수정
- 자동 낚시가 막히면(낚시 화면 없음 · 팔 물고기 없음) F3 없이도 잠깐 쉬었다가 리셋하고 낚시 장소로 다시 가서 시도
  - 쉬는 시간은 30초부터 실패할수록 늘어남 (최대 5분) · 낚시가 잘 되면 처음부터
- 오토 팝핑 후 바이옴이 끝나길 기다리는 시간을 최대 20분으로 제한 (넘으면 매크로 복귀)

## Acrux macro V1.34.0 Update
- Macro features are now organized into always-on · single-run · emergency features
  - Emergency: rare biome auto popping — runs before anything else
  - Always-on: auto fishing — steps aside while another feature uses the spot
  - Single-run: merchant auto buy · potion auto craft · auto memory match · auto item use — one at a time, first come first served
  - Even if several features ask auto fishing to pause at once, fishing resumes only after all of them are done
- When auto fishing gets stuck (no fishing screen · no fish to sell), it now waits briefly, resets, goes back to the fishing spot and retries without needing F3
  - The wait starts at 30 seconds and grows with each failure (up to 5 minutes) · resets once fishing works again
- Waiting for the biome to end after auto popping is now capped at 20 minutes (then returns to your server)
