## Acrux macro V1.15.0 업데이트
- 자동 낚시 구조를 새로 짬 (더 안정적으로)
  - 정해진 순서대로 움직이던 방식 → 매번 화면을 보고 지금 상태(Fish · 입질 대기 · 릴링 · 결과창)를 판단해서 그에 맞게 움직이는 방식
  - 클릭이 한 번 씹히거나 결과창이 늦게 떠도 다음 확인 때 알아서 바로잡음 (결과창이 남아 있으면 X 를 다시 누름)
  - ◇ 표시를 못 찾았을 때 내 위치를 실제보다 왼쪽으로 잘못 보던 문제 수정 (막대가 구간과 겹치면 생기던 오차 · 쓸데없는 클릭의 원인)
  - 한 화면만 튀는 위치 값은 버리고, 떨어지는 속도도 최근 몇 화면으로 계산해서 덜 흔들림
- 릴링이 끝나고 결과창 X 를 누르기까지 기다리는 시간 0.5초 늘림 (기본 0.8 → 1.3초 · 기본값을 쓰던 사람은 자동으로 바뀜)
- 세부 설정에 **릴링 기록 저장** 추가 — 낚시가 불안정할 때 켜고 몇 번 낚은 뒤 데이터 폴더의 fishing_log.csv 를 보내주면 원인 확인 가능

## Acrux macro V1.15.0 Update
- Rebuilt auto fishing (more stable)
  - Instead of following a fixed sequence, it now checks the screen every time, decides the current state (Fish · waiting for a bite · reeling · result) and acts on it
  - Recovers by itself if a click is missed or the result window shows up late (clicks X again if the result is still open)
  - Fixed the marker position being read too far left when the ◇ wasn't found (an error when the bar overlaps the zone · caused extra clicks)
  - Ignores single-frame jumps in the marker position, and computes the falling speed over the last few frames so it wobbles less
- The wait before clicking the result X after reeling is 0.5 s longer (default 0.8 → 1.3 s · applied automatically if you used the default)
- Added **Save reel log** to fine-tuning — if fishing is unstable, turn it on, fish a few times, and send fishing_log.csv from the data folder
