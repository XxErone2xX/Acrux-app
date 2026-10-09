## Acrux macro V2.0.15 업데이트
- 오토 메모리 매치: 시작을 누른 뒤 카드 판을 못 찾고 리셋해버려서(진행 중인 판이 날아가고) 다른 기능까지 꼬이던 문제 수정
  - 카드 판을 글자(Memory Match · CHANCES)로 못 읽어도, 알림 창이 사라졌고 그 자리(알림 창 위치로 계산 · 카드 판 자동 보정값)에 카드 20장 뒷면이 보이면 카드 판으로 봄
  - 그래도 못 찾으면 바로 리셋하지 않고 15초 더 확인
  - 카드 뒷면(어두운 바탕 + 흰 별)인지 같이 확인해서 게임 화면을 판으로 잘못 보지 않게 함

## Acrux macro V2.0.15 Update
- Auto memory match: fixed the board not being found after pressing Start, which reset the character (throwing away the running round) and tangled other features
  - Even if the board's text (Memory Match · CHANCES) can't be read, it counts as the board once the notification is gone and 20 face-down cards are visible where the board should be (worked out from the notification position, or the card board calibration)
  - If it still can't find it, it checks for 15 more seconds instead of resetting right away
  - Face-down cards (dark card + white star) are checked so the game world isn't mistaken for the board
