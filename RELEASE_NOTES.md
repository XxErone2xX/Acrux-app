## Acrux macro V1.26.11 업데이트
- 자동 낚시 기록에서 성공이 쓰레기로 세지던 진짜 원인 수정
  - 보통 물고기의 성공 제목 "Fish Caught!" 은 흰색인데, 흰 글자 가장자리가 회색으로 번진 걸 쓰레기(회색 제목)로 보고 있었음
  - 이제 흰색 · 밝은 색 제목 = 성공 · 회색 제목 = 쓰레기 · 빨간 제목 = 실패
  - 미니게임 창의 빨간 남은 시간(0.0)을 실패로 잘못 보던 것도 막음 (제목처럼 넓고 큰 글자만 봄)

## Acrux macro V1.26.11 Update
- Fixed the real cause of successful catches being counted as junk in the auto fishing record
  - The success title "Fish Caught!" is white for normal fish, and the gray edges of the white letters were being read as the gray junk title
  - Now a white or bright-colored title = success · gray title = junk · red title = fail
  - The minigame's red time left (0.0) is no longer mistaken for a fail (only wide, large title-like text counts)
