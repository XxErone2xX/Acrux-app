## Acrux macro V1.15.1 업데이트
- 입질 후 미니게임으로 넘어갈 때 결과창 X 를 한 번 누르던 버그 수정
  - 넘어가는 순간엔 버튼도 릴링 바도 안 보여서, 결과창 제목 자리에 보이는 하늘 · 배경을 결과창으로 잘못 보던 문제
  - 이제 결과창은 릴링이 끝난 뒤에만 다룸
- 낚은 뒤 결과창 닫는 방식 변경 (최대한 빠르게)
  - 물고기를 낚은 게 확인되면 Fish 버튼이 다시 보일 때까지 결과창 X 를 0.1초마다 미리 눌러 둠 → 결과창이 뜨자마자 닫힘
  - 결과(성공 / 쓰레기 / 실패)는 X 를 누르기 직전마다 제목 색으로 확인해서 기록
  - 그래서 세부 설정의 '결과창 대기'는 없앰

## Acrux macro V1.15.1 Update
- Fixed the result X being clicked once when a bite turns into the minigame
  - During the switch neither the button nor the reel bar is visible, and the sky · background at the result title spot was mistaken for the result window
  - The result window is now only handled after reeling ends
- New way to close the result after a catch (as fast as possible)
  - Once a catch is confirmed, it clicks the result X every 0.1 s until the Fish button is back → the result closes the moment it appears
  - The result (success / junk / fail) is read from the title color right before each X click
  - 'Result window wait' was removed from fine-tuning
