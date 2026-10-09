## Acrux macro V2.0.17 업데이트
- 오토 메모리 매치: 판이 끝났는데 Close 를 안 누르고 계속 카드를 누르던 문제 수정
  - 게임이 끝나 남은 카드가 전부 앞면으로 공개되면 바로 판 끝으로 봄 (남은 기회 숫자를 못 읽거나 세던 값이 어긋나도)
  - 남은 기회가 0 으로 두 번 연속 읽히면 믿음
  - Close 를 누른 뒤 카드 판이 닫혔는지 확인하고, 안 닫혔으면 다시 누름 (최대 3번)

## Acrux macro V2.0.17 Update
- Auto memory match: fixed it continuing to press cards after the round ended instead of pressing Close
  - When the game ends and every remaining card is revealed face up, the round counts as over right away (even if the chances number can't be read or the count drifted)
  - A chances reading of 0 is trusted when seen twice in a row
  - After pressing Close it checks the board closed and presses again if not (up to 3 times)
