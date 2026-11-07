## Acrux macro V2.5.2 업데이트
- **쓰레기(Fish Caught...?)가 잡히면 결과창을 못 닫고 멈추던 문제 수정**
  - 쓰레기 결과창은 실패 · 성공 창과 크기 · 바탕이 달라서, 보정한 X 자리도 · 화면에서 X 찾기도 빗나갔음
  - 바탕이 밝으면 X 찾기가 게임 화면 속 엉뚱한 자리를 X 로 잡아서 그 자리만 계속 누르기도 했음
  - 이제 결과창(흰 꺾쇠 창) 자체를 먼저 찾아서 그 창의 오른쪽 위 X 를 누름 · X 가 안 보이면 창 크기 비율로 계산한 자리
- 창 테두리가 흐리게 번져 보일 때도 창을 찾음

## Acrux macro V2.5.2 Update
- **Fixed fishing stopping because the result window could not be closed after catching junk (Fish Caught...?)**
  - The junk result window has a different size and background than the fail / success windows, so both the calibrated X spot and the on-screen X search missed it
  - With a bright background the X search could also pick a wrong spot in the game view and keep clicking it
  - The result window itself (white cornered window) is now found first and its top-right X is clicked · if the X is not visible, the spot is calculated from the window size
- Windows are found even when their border looks blurred
