## Acrux macro V3.2.3 업데이트
- 길 기록이 켜지면 로블록스 창이 뒤로 밀리던 문제 수정
  - 원인: "길 기록 중" 안내 띠가 뜨는 순간 로블록스에서 포커스를 가져갔음 [ 띠를 띄운 뒤에 '포커스 안 가져감' 설정을 했음 ]
  - 안내 띠 · 현재 상태 창은 설정을 먼저 하고 띄움 · 그래도 포커스를 가져가면 바로 로블록스로 돌려 놓음
  - 길 기록 중 첫 키를 누르기 전까지 로블록스가 맨 앞이 아니면 다시 맨 앞으로

## Acrux macro V3.2.3 Update
- Fixed the Roblox window being pushed back when route recording starts
  - Cause: the "Recording route" banner took focus from Roblox the moment it appeared [ the no-focus setting was applied after showing it ]
  - The banner · status panel now get that setting before they show · if they still take focus, it goes straight back to Roblox
  - Until the first key is pressed while recording, Roblox is brought back to the front if it isn't
