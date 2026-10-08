## Acrux macro V2.0.10 업데이트
- 물고기 판매 루프 수정: Sell All 뒤 확인창(Sell · Cancel)이 떠도 '물고기 없음' 으로 보고 상점을 닫던 문제
  - 확인창을 초록 픽셀(0.8초)만 보던 것 → 초록 픽셀 + 글자(Sell · Cancel)로 2.5초 동안 확인 · 픽셀 기준도 넉넉하게
  - 확인창 Sell 을 누른 뒤 창이 닫혔는지 확인하고, 안 닫혔으면 다시 누름 (최대 3번)
  - Sell All · Sell 버튼은 마우스를 근처로 옮겼다가 누름 (클릭이 씹히지 않게)

## Acrux macro V2.0.10 Update
- Fixed the fish selling loop closing the shop as "no fish" even though the Sell · Cancel confirm window appeared
  - The confirm window was checked only by green pixels for 0.8 s → now green pixels + the Sell · Cancel text for up to 2.5 s, with a looser pixel check
  - After pressing Sell it checks that the confirm window closed, and presses again if not (up to 3 times)
  - Sell All and Sell are pressed after moving the mouse next to them first (so clicks don't get dropped)
