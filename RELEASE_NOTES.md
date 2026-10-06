## Acrux macro V1.28.0 업데이트
- **오토 아이템 사용 추가** (매크로 기능 설정 → 오토 아이템 사용 · 자동 낚시 바로 아래)
  - 매크로가 켜져 있는 동안 쿨타임마다 인벤토리에서 아이템을 1개씩 사용: **Strange Controller** 10.5분 · **Biome Randomizer** 18분 (각각 켜기 · 끄기 · 간격 바꾸기)
  - Inventory → Items → 아이템 검색 → 이름 확인(OCR) → 1개 사용 → Inventory 닫기 · 위치는 통합 위치의 인벤토리 위치 · OCR 영역을 씀
  - 자동 낚시 중이면 낚시가 끝난 자리에서 잠깐 비켜줬다가 이어감 · 이동 · 판매 중엔 끝난 뒤에 사용
  - 아이템을 못 찾으면 1분 뒤 다시 시도 · 스나이핑 중엔 쉬고 내 서버로 돌아오면 다시 시작
  - [지금 사용 테스트] 로 쿨타임과 상관없이 바로 한 번 써 볼 수 있음

## Acrux macro V1.28.0 Update
- **Added Auto item use** (Macro feature settings → Auto item use · right below Auto fishing)
  - While the macro is on, uses one item from the inventory each cooldown: **Strange Controller** every 10.5 min · **Biome Randomizer** every 18 min (each can be turned on/off and its interval changed)
  - Inventory → Items → search the item → check the name (OCR) → use 1 → close Inventory · uses the inventory positions and OCR area from Shared positions
  - During auto fishing, fishing steps aside at a safe point and then continues · during movement or selling, it waits until they finish
  - If the item isn't found it retries in 1 minute · it rests while sniping and starts again back in your server
  - [Use now (test)] uses it once right away, ignoring the cooldown
