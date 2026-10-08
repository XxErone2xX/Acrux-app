## Acrux macro V2.0.4 업데이트
- 스나이핑 접속이 로블록스를 못 띄우고 끝나면, 복귀한 내 서버에서도 바이옴 웹후크가 꺼지고 매크로가 '스나이핑 중' 으로 멈춰 있던 문제 수정
- 레어 바이옴 자동 팝핑: 낚시 장소 · 판매 장소로 걷는 중이거나 이동 · 자동 보정이 도는 중이어도 먼저 멈추고 팝핑 · 전부 멈춘 게 확인된 뒤에만 클릭 (멈추지 않으면 '그냥 진행' 대신 취소)
- 자동 보정 중에 자동 낚시가 다시 켜지던 문제 수정
- 스나이핑한 서버에서 오토 팝핑 · 입장(Play 클릭) 중에 새 링크가 오면 그 서버를 나가던 문제 수정 → 새 링크는 건너뜀
- 매크로 복귀 때 내 서버 입장이 확인되지 않으면 2번까지 다시 복귀 · 그래도 안 되면 매크로를 멈춘 채로 두고, 직접 들어가거나 F3 으로 다시 켜면 이어감
- 매크로를 멈추거나 F7 을 누른 뒤에도 리셋 키(Esc · R · Enter)가 다른 창에 입력되던 문제 수정 · 리셋 전에 로블록스 창을 앞으로 가져옴
- 포션 자동 제작의 Auto Crafted 알림 확인(글자 읽기)이 매크로 전체를 몇 초씩 멈추던 문제 수정 · 응답 없는 글자 읽기가 계속 쌓이지 않게 함
- 오토 메모리 매치: 할 수 있으면 바로 하고, 'Watch AD' 칸이 있으면 광고를 최대한 봄 (광고 1번 = 쿨타임 3시간 ↓ · 광고로 쿨타임이 끝나면 또 메모리 매치) · 광고 칸이 없으면 남은 시간 뒤에 (최대 1시간 뒤) 다시 확인
- 오토 메모리 매치: 짝을 맞추면 기회가 안 줄어드는데 매번 줄어드는 걸로 세서 판을 일찍 끝내던 문제 수정
- 매크로 탭 오토 메모리 매치에 '본 광고' 수 표시

## Acrux macro V2.0.4 Update
- Fixed: when a snipe join never launched Roblox, biome webhooks stayed muted and the macro stayed paused as "sniping" even after returning to your own server
- Rare biome auto popping now stops walking to the fishing/sell spot, movement and auto calibration first, and clicks only once everything has actually stopped (cancels instead of "continuing anyway")
- Fixed auto fishing restarting during auto calibration
- Fixed leaving a sniped server when a new link arrived during auto popping or while joining (Play clicks) → the new link is skipped
- If a macro return can't confirm you joined your server, it retries up to 2 times · after that the macro stays paused and resumes once you join yourself or turn it back on with F3
- Fixed reset keys (Esc · R · Enter) being sent to other windows after stopping or pressing F7 · Roblox is brought to the front before resetting
- Fixed the Auto Crafted notice check (text reading) for potion auto craft freezing the whole macro for seconds · stuck text reads no longer pile up
- Auto memory match: plays right away when ready, and watches as many ads as possible while the 'Watch AD' bar is shown (each ad = −3 h cooldown · plays again once ads finish the cooldown) · without the ad bar it checks again when the cooldown ends (within 1 hour at most)
- Auto memory match: fixed ending the round early by counting a chance for every pair (matched pairs don't use a chance)
- Macro tab auto memory match now shows how many ads were watched
