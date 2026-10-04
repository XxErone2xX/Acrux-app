## Acrux macro V1.17.0 업데이트
- 자동 낚시 릴링 안정화 (보내주신 릴링 기록으로 원인 확인)
  - 내 위치가 구간 안에 있으면 막대와 겹쳐서 구간이 잘리거나 안 보이는 화면이 20% 쯤 있었고, 그때 클릭을 안 해서 구간 아래로 빠졌음
  - 이제 구간 폭 · 위치를 기억해서, 잘리거나 잠깐 안 보여도 그대로 따라감
  - 가운데 남은 시간 숫자를 구간으로 잘못 보던 문제 수정
  - 기록에서 잰 실제 움직임(클릭 한 번에 36px 위로 · 1초에 116px 아래로)으로 시험: 구간 안에 있는 시간 약 78% → 99%
- **알림 영역** 추가 (매크로 기준 위치 설정 → 자동 낚시)
  - 오른쪽에 뜨는 "Cannot Fish (인벤토리 공간 부족)" 알림을 보고 바로 인벤토리 가득으로 판단 (Fish 를 3번 다시 눌러 보지 않음)
  - 빨간 알림이 보이면 글자(OCR)로 한 번 더 확인 · 다른 기능에서도 같은 방식으로 알림 감지에 쓸 예정

## Acrux macro V1.17.0 Update
- More stable auto fishing reeling (cause found from your reel log)
  - When the marker was inside the zone, the fill overlap made the zone look cut off or disappear in about 20% of frames, and it didn't click then, so the marker fell out of the zone
  - It now remembers the zone's width and position and keeps following it even when it's cut off or briefly hidden
  - Fixed the countdown number in the middle being mistaken for the zone
  - Tested with the real movement measured from the log (one click = 36 px up · falls 116 px per second): time in the zone went from about 78% to 99%
- Added a **Notification area** (Macro base position settings → Auto fishing)
  - The "Cannot Fish (not enough inventory space)" notification on the right now means inventory full right away (no more pressing Fish 3 times)
  - When a red notification shows, the text is double-checked with OCR · other features will use the same notification detection
