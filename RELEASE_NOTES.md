## Acrux macro V1.17.2 업데이트
- '낚시 미니게임 창 영역' 지정할 때만 위치 지정 창이 안 뜨던 문제 수정
  - 원인 1: 자동 낚시가 돌고 있으면 미니게임 중에 로블록스를 계속 앞으로 끌어와서 위치 지정 창이 그 뒤로 숨음
  - 원인 2: 위치 지정 창이 뜨는 1~3초 사이 미니게임이 끝나 버릴 수 있음
  - 위치 지정 창이 떠 있는 동안 자동 낚시는 로블록스를 앞으로 끌어오지 않고, 클릭 · 마우스 이동도 멈춤
  - 미니게임이 감지된 순간의 화면을 바로 찍어서 위치 지정 창에 그대로 보여줌

## Acrux macro V1.17.2 Update
- Fixed the picker window not showing only when setting the 'Fishing minigame window area'
  - Cause 1: while auto fishing runs, it kept pulling Roblox to the front during the minigame, hiding the picker behind it
  - Cause 2: the minigame could end in the 1–3 s it takes the picker to open
  - While a picker is open, auto fishing no longer brings Roblox to the front and holds its clicks and mouse moves
  - The screen is captured the moment the minigame is detected and shown in the picker as-is
