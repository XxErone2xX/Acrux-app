## Acrux macro V2.2.9 업데이트
- 메모리 최적화: 글자 읽기(RapidOCR)를 앱 안이 아니라 따로 띄운 작은 프로세스에서 돌림 — 필요할 때만 뜨고 45초 동안 안 쓰면 꺼져서 메모리를 전부 돌려받음 (예전엔 켜자마자 불러오고, 화면 전체를 한 번 읽으면 수백 MB 로 늘어난 채 그대로였음)
- 렉 줄이기: 글자 읽기가 CPU 코어를 다 쓰지 않게 스레드 수를 줄이고 우선순위를 낮춤 (사양 낮은 PC 에서 게임이 덜 버벅임)
- 렉 줄이기: 로블록스 창 찾기를 가볍게 (로블록스가 꺼져 있을 때 계속 찾던 것도 줄임) · 로블록스 로그 폴더 확인을 가볍게 (로그 파일이 많이 쌓여 있어도)
- 렉 줄이기: Acrux 창을 안 보고 있을 때 화면 갱신을 덜 자주 · 화면(Edge)에서 안 쓰는 기능을 더 끔
- 처음 글자를 읽을 때만 1~2초 더 걸릴 수 있음 (프로세스를 띄우는 시간)

## Acrux macro V2.2.9 Update
- Memory: text reading (RapidOCR) now runs in a small separate process instead of inside the app — it starts only when needed and closes after 45 seconds unused, giving all its memory back (before, it loaded at startup and stayed at hundreds of MB after reading a full screen once)
- Less lag: text reading no longer uses every CPU core and runs at lower priority (games stutter less on low-end PCs)
- Less lag: finding the Roblox window is lighter (including the constant searching while Roblox is closed) · checking the Roblox log folder is lighter (even with many log files)
- Less lag: the Acrux window refreshes less often while you are not looking at it · more unused browser features are turned off
- The first text read may take 1–2 seconds longer (starting the process)
