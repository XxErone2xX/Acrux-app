## Acrux macro V1.17.1 업데이트
- 위치 지정 창이 가끔 안 뜨고, 그 뒤로 다른 위치 지정도 안 되던 문제 수정
  - 원인: 선택 창이 로블록스 뒤에 숨는 등 안 보여도 최대 2분 30초 동안 기다리느라 다른 위치 지정이 막힘
  - 선택 창을 확실히 맨 앞으로 띄우고, 6초 안에 안 뜨면 자동으로 한 번 더 띄움 · 그래도 안 되면 바로 실패로 끝냄
  - 위치 지정 중에 버튼을 한 번 더 누르면 취소

## Acrux macro V1.17.1 Update
- Fixed the position picker window sometimes not showing, which then blocked all other position picking
  - Cause: even when the picker window was hidden (e.g. behind Roblox), it kept waiting up to 2 min 30 s, blocking every other pick
  - The picker now forces itself to the front, reopens once if it doesn't show within 6 s, and otherwise ends right away
  - Pressing the button again while picking cancels it
