## Acrux macro V1.26.9 업데이트
- 자동 낚시 기록에서 성공이 쓰레기로 세지던 문제 수정
  - 결과창 바탕이 반투명이라 밝은 곳에선 바탕이 회색으로 보여서, 하늘색 "Fish Caught!" 글자가 회색(쓰레기)에 묻혔음
  - 이제 제목 글자 색만 보고, 하늘색 · 빨강 글자가 보이면 그걸 우선함
  - 결과창이 뜨는 중(글자가 흐릴 때)엔 쓰레기로 정하지 않고 다 뜰 때까지 잠깐 기다림

## Acrux macro V1.26.9 Update
- Fixed successful catches being counted as junk in the auto fishing record
  - The result window background is see-through, so in bright places it looked gray and the light-blue "Fish Caught!" text got outvoted as gray (junk)
  - Now only the title text color is used, and light-blue or red text wins when it's there
  - While the result window is still fading in (text faint), it waits a moment instead of deciding junk
