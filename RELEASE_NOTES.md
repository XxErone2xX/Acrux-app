## Acrux macro V1.20.1 업데이트
- OCR 을 예전 방식으로 되돌림 (켤 때 미리 불러둠)
- **결과창에서 X 를 안 누르고 멈추던 버그** 수정
  - 결과창이 뜨면 낚시 창이 사라지는데, 그 자리 뒤의 파랑 · 빨강 줄무늬 계단 같은 배경을 Fish/Exit 버튼으로 잘못 봤음
  - 그래서 X 를 멈추거나, Fish 를 계속 누르다 '인벤토리 가득'으로 낚시가 멈췄음 → 이제 어두운 바탕의 진짜 버튼만 버튼으로 봄
- **미니게임 구간을 놓쳐서 엉뚱하게 누르던 문제** 수정 (청록 · 갈색 구간 물고기)
  - 미니게임마다 막대 색을 배워서, 바 안에서 막대 색도 빈 칸도 숫자도 아닌 곳을 전부 구간으로 봄
  - 막대와 겹친 부분(밝은 하늘색 · 회갈색)도 구간으로 읽어서 구간 오른쪽으로 넘어가게 누르지 않음

## Acrux macro V1.20.1 Update
- OCR is back to the previous behavior (preloaded at startup)
- Fixed **the result window not being closed with X**
  - When the result window opens the fishing panel disappears, and scenery behind the Fish button spot (like blue/red striped stairs) was read as the Fish/Exit button
  - That stopped the X clicks, or kept clicking Fish until fishing stopped as "inventory full"; only a real button on a dark background counts now
- Fixed **the minigame losing the zone and clicking at the wrong time** (teal or brown zones)
  - The fill color is learned each minigame, and everything in the bar that is not the fill, the empty background or the digits is read as the zone
  - The part overlapped by the fill (light cyan, grayish brown) now counts as zone, so it no longer clicks the marker past the zone
