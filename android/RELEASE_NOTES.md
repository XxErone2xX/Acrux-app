## Acrux Mobile V0.10.7 업데이트
- **사용자 수 집계를 끌 수 없게 바꿈**
  - 설정 → 정보의 '사용자 수 집계에 참여' 스위치를 없앰 — 항상 참여
  - 보내는 건 그대로 '켜져 있다는 신호' 뿐 (설치마다 만든 무작위 번호 · 개인 정보 없음)

## Acrux Mobile V0.10.7 Update
- **The user count can no longer be turned off**
  - Removed the "Join the user count" switch from Settings → Info — always on
  - It still only sends an "app is on" signal (a random number made per install · no personal info)

## Acrux Mobile V0.10.6 업데이트
- **길 기록: 누를 때마다 옮겨지는 조이스틱 판을 따라감**
  - 로블록스 조이스틱은 누른 자리로 판이 옮겨짐 — 예전엔 원래 자리 근처만 봐서, 멀리 누르면 판을 놓치고 엉뚱한 방향을 읽었음
  - 이제 화면 왼쪽 아래를 넓게 보면서: 방금 판 자리 → 원래 자리 → 그래도 없으면 넓게 찾음
  - 잠깐 판이 안 보이면 (흰 구름 · 효과에 가려짐) 0.3초까지는 방금 방향 그대로 (예전엔 '멈춤' 으로 기록됐음)
  - 배경도 색이 옅을 때 (흰 구름 · 회색 바닥) 손잡이 위치가 끌려가던 것 막음
- V0.10.5 에서 빠르게 바꾼 재생 간격을 원래대로 되돌림 (0.06 ~ 0.1초) — 너무 짧으면 터치가 끊겨 조이스틱을 다시 누르게 될 수 있음

## Acrux Mobile V0.10.6 Update
- **Route recording follows the joystick ring that moves each time you press**
  - The Roblox joystick ring jumps to where you press — before, it only looked near the original spot, so pressing far away lost the ring and read wrong directions
  - Now it watches the whole bottom-left area: the last ring spot → the original spot → a wide search if still not found
  - If the ring is briefly hidden (white clouds · effects), the last direction is kept for up to 0.3 s (before, this was recorded as "stopped")
  - Stopped the knob position being pulled off when the background is also low-color (white clouds · gray floors)
- Restored the replay step timing changed in V0.10.5 (0.06-0.1 s) — steps that are too short can break the touch and re-press the joystick

## Acrux Mobile V0.10.5 업데이트
- **길 기록이 중간중간 튀던 것 줄임**
  - 손잡이 위치를 소수점 단위로 잼 (찾은 원 안의 회색 무게 중심) — 배경 무늬 때문에 1~2px 씩 튀던 게 줄어듦
    - 영상 기준: 안 누를 때 흔들림 0.21 → 0.17 이하 · 끝까지 밀 때 0.83 ~ 1.00 → 0.92 ~ 1.00
  - 방금 방향과 10도 안쪽으로만 달라지면 같은 방향으로 봄 (손가락은 그대로인데 읽은 값만 조금 튀는 것)
  - 앞뒤가 같은 방향인데 가운데만 잠깐(0.12초 미만) 튄 건 지움 — 빠르게 돌린 건 그대로 남김
- 길을 다시 걸을 때 조이스틱 방향을 더 자주 바꿈 (0.06 ~ 0.1초 → 0.035 ~ 0.07초 마다) — 빠르게 돌린 길도 더 비슷하게

## Acrux Mobile V0.10.5 Update
- **Reduced jumps in recorded routes**
  - Knob position is measured to sub-pixel precision (gray center of mass inside the found circle) — fewer 1-2px jumps caused by the background pattern
    - In the video: idle wobble 0.21 → 0.17 or less · full push 0.83-1.00 → 0.92-1.00
  - A change of less than 10° from the current direction counts as the same direction (the finger has not moved, only the reading wobbled)
  - A short blip (under 0.12 s) between two segments in the same direction is removed — fast turns are kept
- When replaying a route the joystick direction is updated more often (every 0.035-0.07 s instead of 0.06-0.1 s) — fast turns replay more closely

## Acrux Mobile V0.10.4 업데이트
- **조이스틱 · 점프 버튼 읽기를 실제 화면에 맞춰 다시 만듦 (보내 준 플레이 영상 기준)**
  - 조이스틱 찾기: 얇은 회색 테두리 원 (바깥 판) 을 찾음 — 예전엔 속이 찬 원으로 생각해서 손잡이를 판으로 잘못 찾았음
    - 영상의 안 누른 화면 전부에서 판 위치 · 크기를 맞게 찾음 (오차 2px 안)
    - 조이스틱을 안 누르고 있을 때 찾아 주세요
  - 길 기록: 로블록스 조이스틱은 누른 자리로 판이 따라 움직임 → 매 프레임 판 위치를 먼저 다시 찾고 손잡이를 그 기준으로 잼
  - 손잡이: 반투명 회색 원을 '색이 옅은 원' 으로 찾음 (풀 · 하늘 같은 배경과 상관없이)
  - 안 누를 때 손잡이가 조금 흔들려 보이는 건 무시 (반지름의 30% 미만은 멈춤)
  - 점프 버튼: 평소엔 버튼 안이 바깥보다 어둡고 (약 -70) 누르면 밝아짐 (약 +20) — 영상의 모든 점프를 잡고, 화면이 크게 바뀌어도 잘못 잡지 않음
  - 점프 버튼을 누르고 있으면 0.45초마다 한 번씩 점프로 기록 (로블록스는 누르고 있으면 계속 뜀)

## Acrux Mobile V0.10.4 Update
- **Rebuilt joystick and jump button reading to match the real screen (based on the gameplay video you sent)**
  - Find joystick: looks for the thin gray outline circle (outer ring) — it used to assume a filled circle and mistook the knob for the ring
    - Finds the ring position and size correctly in every untouched frame of the video (within 2px)
    - Please run it while not touching the joystick
  - Route recording: Roblox moves the joystick ring to where you press → every frame it finds the ring again first, then measures the knob from it
  - Knob: finds the translucent gray circle as a "low-color circle" (works on grass, sky or any background)
  - Small knob wobble while not touching is ignored (under 30% of the radius = stopped)
  - Jump button: normally the inside is darker than the outside (about -70) and it lights up when pressed (about +20) — catches every jump in the video with no false jumps even when the scene changes a lot
  - Holding the jump button records a jump every 0.45 s (Roblox keeps jumping while held)

## Acrux Mobile V0.10.3 업데이트
- **줌 아웃할 때 화면이 돌아가서 방향이 틀어지던 문제 수정**
  - 로블록스는 두 손가락 줌을 하는 동안 화면도 같이 돌려 버림 (손가락 방향을 바꿔도 막을 수 없음)
  - 그래서 줌 아웃을 **도감 열고 닫기(카메라 정렬) 전에** 함 → 돌아간 화면은 정렬이 캐릭터 뒤로 되돌리고, 줌 거리는 그대로
  - 기준 장소 순서: 재설정 → 줌 아웃 → 도감 열고 닫기 → 조이스틱 걷기 → 내려다보기

## Acrux Mobile V0.10.3 Update
- **Fixed the camera turning during zoom-out and throwing off directions**
  - Roblox also turns the camera during a two-finger zoom (changing the finger direction cannot prevent it)
  - So zoom-out now happens **before opening and closing the collection (camera align)** → the align turns the camera back behind the character, and the zoom distance stays
  - Base spot order: reset → zoom out → open and close the collection → joystick walk → look down

## Acrux Mobile V0.10.2 업데이트
- **길 기록이 조이스틱 위치 + 점프 버튼만 봄 (화면 터치 표시 · 개발자 옵션 필요 없음)**
  - 기록 중에 화면에서 읽는 건 두 가지뿐: 조이스틱 손잡이가 밀린 방향 · 점프 버튼이 눌렸는지 (누르면 버튼이 밝아짐)
  - ⚙ → **점프 버튼 (오른쪽 아래)** 위치를 한 번 지정 — 없으면 점프는 기록 안 함
  - 재생할 때 점프는 조이스틱 손가락을 뗀 채로 따로 누르지 않고 같은 터치에 섞어서 보냄 (걷던 방향이 안 끊김)
  - 길 정보에 점프 횟수도 보임

## Acrux Mobile V0.10.2 Update
- **Route recording now only reads the joystick position and the jump button (no "show taps" or developer options needed)**
  - While recording it reads just two things from the screen: which way the joystick knob is pushed · whether the jump button is pressed (it lights up when pressed)
  - Set ⚙ → **Jump button (bottom right)** once — without it, jumps are not recorded
  - On replay the jump tap is sent together with the joystick finger instead of on its own (walking direction is not interrupted)
  - Route info also shows the jump count

## Acrux Mobile V0.10.1 업데이트
- **줌 아웃할 때 화면이 돌아가던 문제 수정**
  - 두 손가락을 좌우 대신 **위아래** 로 놓고 오므림 — 손가락이 조금 어긋나도 화면이 좌우로 안 돎
  - 두 손가락을 먼저 0.12초 가만히 댄 뒤 똑같이 오므림 (한 손가락이 먼저 움직이면 로블록스가 화면 돌리기로 봤음)
  - 순서를 **줌 아웃 → 내려다보기** 로 바꿈 — 줌 아웃 중에 위아래로 조금 기운 건 내려다보기가 다시 맞춤

## Acrux Mobile V0.10.1 Update
- **Fixed the camera turning while zooming out**
  - The two fingers are now placed **top and bottom** instead of left and right — a slight mismatch no longer turns the camera sideways
  - Both fingers rest still for 0.12 s first, then pinch together (Roblox took one finger moving first as a camera turn)
  - Order changed to **zoom out → look down** — any small up/down tilt from zooming is fixed by the look-down

## Acrux Mobile V0.10.0 업데이트
- **Shizuku 를 완전히 뺌 — 모든 기능이 화면 터치로 (루팅 · Shizuku · 무선 디버깅 필요 없음)**
  - 필요한 건 접근성 권한 · 화면 캡처 둘뿐
  - 로블록스 설정 → 이동 방식을 **조이스틱(클래식)** 으로
- **재설정 (터치)**
  - 로블록스 메뉴 버튼 → 캐릭터 재설정 → 재설정 확인 을 차례로 탭
  - ⚙ → 공통 위치에서 세 버튼 위치를 한 번 지정
- **기준 장소 (터치)**
  - 재설정 → 도감 열고 닫기 → 조이스틱으로 W+A 3.5초 → A 3초 → A+W 2초 → 0.5초 쉼 → D 1초
  - 내려다보기: 화면을 위에서 아래로 끌기 (기본 3번)
  - 줌 아웃: 두 손가락 오므리기 (기본 4번)
- **새 이동 방식: 길 기록**
  - 예전의 '지점 찍기 · 시간 재기 (Click to Move)' 대신, 기준 장소에서 목적지까지 **직접 걸어간 길을 저장해서 그대로 다시 걸음**
  - ⚙ → 낚시 장소 길 / 판매 장소 길 → **● 길 기록**: 기준 장소로 자동으로 간 뒤 '지금부터 기록' → 조이스틱으로 걸어가서 [도착]
  - 기록하는 동안 화면에서 조이스틱 손잡이 위치를 매 프레임 읽음
  - 예전에 저장한 지점은 안 씀 — 길을 새로 기록해 주세요
- **판매 (터치)**
  - E 키 대신 판매 장소에서 뜨는 **대화 버튼** 을 탭 — ⚙ → 판매 → 대화 버튼 (E) 위치를 판매 장소에 서서 지정
  - Sell Fish · Sell All · 확인 Sell 은 탭 → 짧은 탭 → 길게 순서로 다시 시도
- 앱 홈의 Shizuku 칸 · 키 테스트를 없앰

## Acrux Mobile V0.10.0 Update
- **Shizuku removed completely — everything works by screen touch (no root · Shizuku · wireless debugging)**
  - Only accessibility and screen capture are needed
  - Set Roblox settings → Movement mode to **Thumbstick (classic)**
- **Reset (touch)**
  - Taps the Roblox menu button → Reset Character → reset confirm in order
  - Set the three button positions once in ⚙ → common positions
- **Base spot (touch)**
  - Reset → open and close the collection → joystick W+A 3.5 s → A 3 s → A+W 2 s → 0.5 s pause → D 1 s
  - Look down: drags the screen from top to bottom (3 times by default)
  - Zoom out: two-finger pinch (4 times by default)
- **New movement: recorded routes**
  - Instead of the old "set points · measure time (Click to Move)", it **saves the route you walked yourself from the base spot and walks it again**
  - ⚙ → Fishing spot route / Sell spot route → **● Record route**: goes to the base spot automatically, then "recording from now" → walk there with the joystick and tap [Arrived]
  - While recording it reads the joystick knob position from the screen every frame
  - Old saved points are no longer used — please record the routes again
- **Selling (touch)**
  - Instead of the E key it taps the **talk button** shown at the sell spot — set ⚙ → Sell → Talk button (E) while standing at the sell spot
  - Sell Fish · Sell All · confirm Sell retry as tap → short tap → long press
- Removed the Shizuku row and key test from the app home

## Acrux Mobile V0.9.11 업데이트
- **기준 장소로 걷는 방법 변경**
  - 재설정 → 화면 맞추기 (도감 열고 닫기) 뒤에
  - W+A 3.5초 → A 3초 → A+W 2초 → 0.5초 쉼 → D 1초
  - 이동 · 판매 탭 → 기준 장소 (초) 에서 시간을 바꿀 수 있음 (예전에 바꾼 W · W+A · A · S+D 값은 새 기본값으로)

## Acrux Mobile V0.9.11 Update
- **Changed how it walks to the base spot**
  - After reset → camera align (open and close the collection)
  - W+A 3.5 s → A 3 s → A+W 2 s → 0.5 s pause → D 1 s
  - Times can be changed in Move · Sell tab → Base spot (s) (old W · W+A · A · S+D values go back to the new defaults)

## Acrux Mobile V0.9.10 업데이트
- **언어 선택 · 후원하기를 설정 탭 → 홈 탭으로 옮김**
  - 언어: 홈 맨 위
  - 후원하기: 홈 맨 아래

## Acrux Mobile V0.9.10 Update
- **Moved language selection and Support from the Settings tab to the Home tab**
  - Language: top of Home
  - Support: bottom of Home

## Acrux Mobile V0.9.9 업데이트
- **W · A · S · D 를 로블록스 조이스틱으로 (Shizuku 없이 터치로)**
  - 로블록스 설정 → 이동 방식을 **조이스틱**(클래식)으로 바꿔 주세요
  - ⚙ → **조이스틱 찾기**: 화면 왼쪽 아래의 조이스틱을 찾아서 위치 · 크기를 저장 (찾은 자리에 초록 원이 잠깐 뜸)
  - 못 찾으면 ⚙ → **조이스틱 직접 지정**: 가운데 → 바깥 판 가장자리를 차례로 누름
  - ⚙ → **조이스틱 테스트**: W · A · S · D 를 0.8초씩 (앞 · 왼쪽 · 뒤 · 오른쪽)
  - 누르는 방법: 조이스틱 가운데를 먼저 누른 뒤 그 방향으로 끌어서 계속 누르고 있음 (W+A 는 왼쪽 위 대각선)
  - 기준 장소로 걷기 (W → W+A → A → S+D) 도 조이스틱으로 함
  - 아직 터치로 못 하는 것 (Esc · Enter · E · 화면 돌리기 · 줌) 은 Shizuku 가 있으면 Shizuku 로 · 없으면 건너뜀 — 다음 버전에서 터치로 바꿈

## Acrux Mobile V0.9.9 Update
- **W · A · S · D now push the Roblox joystick (by touch, no Shizuku)**
  - Set Roblox settings → Movement mode to **Thumbstick** (classic)
  - ⚙ → **Find joystick**: finds the joystick at the bottom left and saves its position and size (a green circle shows where it was found)
  - If it is not found, ⚙ → **Set joystick by hand**: tap the center, then the outer edge
  - ⚙ → **Joystick test**: W · A · S · D for 0.8 s each (forward · left · back · right)
  - How it presses: touches the joystick center first, then drags toward the direction and keeps holding (W+A is up-left diagonal)
  - Walking to the base spot (W → W+A → A → S+D) also uses the joystick
  - Things not done by touch yet (Esc · Enter · E · camera turn · zoom) use Shizuku if connected, otherwise they are skipped — moving to touch in the next version

## Acrux Mobile V0.9.8 업데이트
- **리셋 방법 변경 — R 키 대신 재설정 버튼을 눌러서 리셋**
  - 모바일 로블록스는 Esc 메뉴에서 R 키가 안 먹어서 재설정 탭으로 안 넘어갔음
  - 이제: Esc → 재설정 버튼 탭 → Enter 3번 (0.4초 간격)
  - ⚙ → 공통 위치에 **재설정 버튼 (Esc 메뉴)** 추가 — Esc 메뉴를 열어 둔 채로 재설정 버튼 위치를 지정
  - 재설정 · 도감 버튼은 마우스 대신 손가락 탭으로 누름

## Acrux Mobile V0.9.8 Update
- **Reset now presses the Reset button instead of the R key**
  - On mobile Roblox the R key does nothing in the Esc menu, so it never got to the reset tab
  - Now: Esc → tap the Reset button → Enter 3 times (0.4 s apart)
  - Added **Reset button (Esc menu)** to ⚙ → common positions — open the Esc menu and set where the Reset button is
  - The Reset and collection buttons are pressed with a finger tap instead of the mouse

## Acrux Mobile V0.9.7 업데이트
- **도감 열고 닫기(카메라 정렬)를 다시 넣음 — 채팅 끄고 켜기만 뺌**
  - 기준 장소: 리셋 → 도감 열고 닫기 → 걷기 → 내려다보기
  - 판매: 판매 장소에 도착하면 도감 열고 닫은 뒤 E
  - 이제 / 키 · 채팅 버튼 · Enter 는 안 누름
  - 공통 위치는 2개만 (도감 버튼 · 도감 Exit) — 채팅 버튼은 지정 안 해도 됨

## Acrux Mobile V0.9.7 Update
- **Brought back the collection open-close (camera align) — only the chat toggle is removed**
  - Base spot: reset → open and close the collection → walk → look down
  - Selling: on reaching the sell spot, open and close the collection, then E
  - The / key, chat button and Enter are no longer pressed
  - Only 2 common positions (collection button · collection Exit) — the chat button is no longer needed

## Acrux Mobile V0.9.6 업데이트
- **채팅 · 도감 열고 닫기(카메라 정렬)를 뺌**
  - 기준 장소로 갈 때 이제 리셋 → 걷기 → 내려다보기만 함
  - 공통 위치 3개(채팅 버튼 · 도감 버튼 · 도감 Exit)를 지정하지 않아도 됨
- **리셋 (Esc → R → Enter) 이 실제 실행에서 씹히던 것 고침**
  - 키를 너무 짧게(0.04초) 눌러서 느린 기기에선 게임이 못 받을 때가 있었음
  - 이제 키마다 0.12초 누르고 0.8초 간격으로 보냄
  - 다른 키(E 등)도 0.08초 누름
  - 떠 있는 창에 지금 누르는 키(ESC · R · ENTER)가 보임
- **시작하면 1초 기다린 뒤 움직임**
  - [시작]을 누른 손가락이 떨어지고 게임 화면으로 돌아온 뒤에 키 · 클릭을 보냄

## Acrux Mobile V0.9.6 Update
- **Removed the chat / collection open-close step (camera align)**
  - Going to the base spot is now just reset → walk → look down
  - The 3 common positions (chat button · collection button · collection Exit) are no longer needed
- **Fixed the reset (Esc → R → Enter) being missed in real runs**
  - Keys were pressed too briefly (0.04 s), so slow devices sometimes missed them
  - Each key is now held 0.12 s, with 0.8 s between keys
  - Other keys (E etc.) are held 0.08 s
  - The floating window shows the key being pressed (ESC · R · ENTER)
- **Waits 1 second after starting before moving**
  - Keys and clicks are sent only after the finger that pressed [Start] is lifted and the game is back in front

## Acrux Mobile V0.9.5 업데이트
- 기준 장소로 갈 때 리셋 (Esc → R → Enter) 을 건너뛰던 경우 수정
  - '방금 리셋함 — 리셋 건너뜀' 상태가 실행이 끝나도 남아서, 그 사이 손으로 움직인 뒤 다시 실행하면 리셋 없이 엉뚱한 곳에서 걷기 시작했음
  - 이제 새로 실행할 때마다 리셋을 꼭 함 (한 실행 안에서 방금 리셋하고 안 움직였을 때만 건너뜀 · PC 와 같음)

## Acrux Mobile V0.9.5 Update
- Fixed the reset (Esc → R → Enter) being skipped when going to the base spot
  - The "just reset — skipping the reset" state stayed after a run ended, so moving by hand and starting again began walking from the wrong place without a reset
  - Every new run now always resets (it is only skipped within one run right after a reset with no movement · same as PC)

## Acrux Mobile V0.9.4 업데이트
- **'판매 한 번 해 보기' 등을 눌러도 아무것도 안 하던 것처럼 보이던 문제 수정**
  - 필요한 것 (Shizuku · 판매 자동 보정 · 판매 장소 · 낚시 장소 지점 · 공통 위치) 이 없으면 시작을 안 하거나 바로 멈췄는데, 그 이유가 앱의 기록 칸에만 남아서 게임 화면에선 아무 일도 없는 것처럼 보였음
  - 이제 이유를 **떠 있는 Acrux 창 · 화면 알림** 으로 바로 보여 줌 (예: "판매 자동 보정 필요: Sell All 버튼 …")
  - 실행 중에 멈추면 떠 있는 창에 '멈춤 — 이유' 가 남음
  - 판매 한 번: 필요한 위치 · 지점을 시작 전에 먼저 확인

## Acrux Mobile V0.9.4 Update
- **Fixed "Try selling once" and similar buttons looking like they did nothing**
  - When something needed was missing (Shizuku · sell auto calibration · sell spot · fishing spot points · common positions), it did not start or stopped right away, but the reason only went to the app's Log tab, so nothing seemed to happen in game
  - The reason now shows right away on the **floating Acrux window and as an on-screen notice** (e.g. "Sell auto calibration needed: Sell All button …")
  - When a run stops, the floating window keeps showing "Stopped — reason"
  - Try selling once: the needed positions and points are checked before starting

## Acrux Mobile V0.9.3 업데이트
- **[보정] 을 눌렀는데 [시작] 이 눌리던 문제 수정**
  - 매크로가 누르는 터치 · 클릭은 화면 맨 위 창이 받음 → Acrux 창이 Fish 버튼 위에 있으면 [보정] 이 누른 Fish 가 Acrux 창의 [시작] 으로 들어가서 보정이 멈췄음
  - 이제 매크로가 Acrux 창 위를 누를 때는 그 순간만 창이 터치를 통과시킴 (손가락 탭 · Shizuku 마우스 둘 다) · 사람이 누르는 건 그대로
- Acrux 창이 낚시 창 쪽을 가리고 [보정] · [시작] 을 누르면 옮기라고 알려 줌 (화면 캡처에 같이 찍혀서 인식이 틀릴 수 있음)

## Acrux Mobile V0.9.3 Update
- **Fixed [Start] getting pressed when tapping [Calibrate]**
  - The macro's taps and clicks go to the topmost window → when the Acrux window sat over the Fish button, the Fish tap from [Calibrate] landed on the Acrux window's [Start] and stopped calibration
  - When the macro presses inside the Acrux window, the window now lets that touch through for a moment (finger taps and Shizuku mouse) · your own taps work as before
- Pressing [Calibrate] or [Start] while the Acrux window covers the fishing window area now tells you to move it (it shows up in the screen capture and can break recognition)

## Acrux Mobile V0.9.2 업데이트
- **대화창 [Sell Fish] · 상점 [Sell All] 이 안 눌리던 문제 수정**
  - 모바일은 대화 선택지가 Roll 버튼 위에 겹쳐 그려져서, 글자 읽기가 'Roll' 글자와 헷갈려 버튼을 못 찾거나 엉뚱한 자리를 눌렀음
  - 이제 버튼을 **테두리 색으로** 찾음: [Open Shop] 초록 · [Sell Fish] 파랑 · [Leave] 빨강 줄 → 가운데 파랑 / [Sell] 초록 바로 오른쪽 파랑 = Sell All
  - 누르기 직전에 화면에서 다시 찾아서 누름 (저장한 자리가 어긋나 있어도)
  - 누르기: **손가락 탭 먼저** → 마우스 (호버 후 클릭) → 손가락 길게 · 상점이 열렸는지 · 확인창이 떴는지 보면서 다시 누름
  - 첫 물고기 칸 · 확인 Sell · 상점 닫기도 손가락 탭 먼저
  - 스크린샷 4장을 164가지로 변형해서 시험 (해상도 0.3 ~ 1.6배 · 늘림 · 검은 띠 · 잘림 · 저화질 · 밝기 · 조합) → 162 통과 · 낚시 화면 252장에서 잘못 찾은 것 0

## Acrux Mobile V0.9.2 Update
- **Fixed [Sell Fish] in the dialog and [Sell All] in the shop not being pressed**
  - On mobile the dialog choices are drawn over the Roll button, so text reading mixed them up with the "Roll" text and missed the button or pressed the wrong spot
  - Buttons are now found by **border color**: [Open Shop] green · [Sell Fish] blue · [Leave] red row → the blue in the middle / the blue right after the green [Sell] = Sell All
  - They are found again on screen right before each press (even if the saved spot is off)
  - Pressing: **finger tap first** → mouse (hover then click) → long finger tap · retried while checking that the shop opened / the confirm window appeared
  - The first fish slot, confirm Sell and shop close also use a finger tap first
  - Tested on 164 variants of 4 screenshots (0.3 to 1.6× resolution · stretch · black bars · crop · low quality · brightness · combos) → 162 pass · 0 false hits on 252 fishing screens

## Acrux Mobile V0.9.1 업데이트
- **Shizuku 로 보낸 W · A · S · D 를 로블록스가 무시하던 문제 수정**
  - 키를 '가상 키보드'(장치 없음) 신호로 넣었는데, 로블록스 모바일은 진짜 키보드가 연결돼 있어야 키보드 이동을 켬
  - 이제 Shizuku 로 진짜 USB 키보드 장치(Acrux Keyboard)를 만들어서 보냄 (블루투스 키보드와 같은 커널 기능) → 안드로이드 · 로블록스가 키보드를 꽂은 것과 똑같이 봄
  - 키보드 장치를 못 만드는 기종에선 예전 방식으로 (누르고 있는 키는 진짜 키보드처럼 자동 반복)
  - 연결할 때 · 키 테스트 때 기록에 어떤 방식으로 보내는지 나옴
- 업데이트 뒤 앱 → ③ Shizuku [다시] 를 한 번 눌러 주세요 (입력 서비스가 새로 바뀜)

## Acrux Mobile V0.9.1 Update
- **Fixed Roblox ignoring W · A · S · D sent through Shizuku**
  - Keys were injected as a "virtual keyboard" (no device), but Roblox mobile only turns on keyboard movement when a real keyboard is connected
  - Shizuku now creates a real USB keyboard device (Acrux Keyboard) and sends keys through it (the same kernel feature Bluetooth keyboards use) → Android and Roblox see it exactly like a plugged-in keyboard
  - On devices where the keyboard device cannot be created, the old method is used (held keys auto-repeat like a real keyboard)
  - The log shows which method is used when connecting and during the key test
- After updating, tap App → ③ Shizuku [Again] once (the input service changed)

## Acrux Mobile V0.9.0 업데이트
- 낚시 통계 성공률: 쓰레기도 잡은 것으로 계산 → (성공 + 쓰레기) ÷ 전체 (쓰레기 때문에 성공률이 낮아지지 않음)

## Acrux Mobile V0.9.0 Update
- Fishing stats success rate: junk now counts as caught → (success + junk) ÷ total (junk no longer lowers the success rate)

## Acrux Mobile V0.8.9 업데이트
- 새 앱 아이콘 (Acrux Mobile 로고) · 런처가 원형 · 둥근 사각형으로 잘라도 로고가 안 잘리게 (적응형 아이콘)

## Acrux Mobile V0.8.9 Update
- New app icon (Acrux Mobile logo) · the logo is not cut off when the launcher crops icons to circles or rounded squares (adaptive icon)

## Acrux Mobile V0.8.8 업데이트
- 낚시 통계에서 판매 횟수 · 판 종류 · 실행 시간 제거 (성공 · 쓰레기 · 실패 · 성공률만)

## Acrux Mobile V0.8.8 Update
- Removed sell trips · kinds sold · run time from the fishing stats (only success · junk · fail · success rate)

## Acrux Mobile V0.8.7 업데이트
- **안드로이드 8 ~ 12 기종에서 앱이 꺼지던 문제 수정**
  - V0.7.0 의 자동 업데이트가 안드로이드 13 이상에만 있는 기능을 써서, 그 아래 버전은 앱을 켜고 업데이트를 확인하는 순간 꺼졌음
  - 빌드할 때마다 '옛날 안드로이드에 없는 기능'을 자동으로 검사 (있으면 빌드가 멈춤 → 다시는 안 섞이게)
- **메모리가 작은 기종에서 꺼지던 문제 줄임**: 창 찾기에 쓰는 메모리를 크게 줄임 · 앱이 쓸 수 있는 메모리를 늘림 · 메모리가 모자라면 꺼지지 않고 멈춤 + 기록
- 화면 캡처 이미지가 기종마다 다르게 생겨도 (줄 끝 여백 · 짧은 버퍼) 안 꺼짐
- 업데이트 확인 · 사용자 수 · 매크로 작업에서 나는 모든 오류를 잡아서 기록 (앱이 통째로 꺼지지 않게)
- **크래시 기록**: 그래도 앱이 꺼지면 다음에 켰을 때 홈에 알림 → 기록 칸에서 [복사] 해서 보내면 원인을 바로 앎 (기종 · 안드로이드 버전 · 오류 내용 · 개인 정보 없음)

## Acrux Mobile V0.8.7 Update
- **Fixed the app closing on Android 8 to 12 devices**
  - The auto updater from V0.7.0 used a feature that only exists on Android 13+, so lower versions closed as soon as the app checked for updates
  - Every build now checks automatically for features missing on older Android (the build stops if any are found → they cannot slip in again)
- **Fewer closes on low-memory devices**: window search uses much less memory · the app may use more memory · when memory runs out it stops and logs instead of closing
- No close when screen capture images differ per device (row padding · short buffers)
- All errors in the update check, user count and macro work are caught and logged (the whole app no longer closes)
- **Crash log**: if the app still closes, Home shows a notice on the next launch → [Copy] it in the Log tab and send it to find the cause right away (device · Android version · error · no personal info)

## Acrux Mobile V0.8.6 업데이트
- **낚시 통계가 사라지던 문제 수정**
  - 통계가 '지금 낚시 중인 것'에만 있어서 [보정] · 판매 · 이동 · 키 테스트를 하거나 앱을 다시 켜면 0 으로 돌아갔음
  - 이제 따로 저장됨: **이번 실행** (앱을 켠 뒤) · **전체** (계속 쌓임 · 앱을 꺼도 남음)
- 낚시 칸 → **낚시 통계**: 성공 · 쓰레기 · 실패 · 성공률 · 판매 횟수 · 판 종류 · 실행 시간 · [이번 실행 초기화] · [전체 초기화]
- 홈 → 상태에도 이번 실행의 성공 · 쓰레기 · 실패가 늘 보임

## Acrux Mobile V0.8.6 Update
- **Fixed fishing stats disappearing**
  - The stats only lived in the current fishing run, so [Calibrate], selling, movement or key tests, or reopening the app reset them to 0
  - They are now saved separately: **This run** (since the app opened) · **All time** (keeps adding up · stays after closing the app)
- Fishing tab → **Fishing stats**: success · junk · fail · success rate · sell trips · kinds sold · run time · [Reset this run] · [Reset all time]
- Home → Status always shows this run's success · junk · fail

## Acrux Mobile V0.8.5 업데이트
- 떠 있는 Acrux 창에서 사용자 수 (● 숫자) 제거 — 앱 맨 위 Now Active 에만 보임

## Acrux Mobile V0.8.5 Update
- Removed the user count (● number) from the floating Acrux window — it now only shows as Now Active at the top of the app

## Acrux Mobile V0.8.4 업데이트
- **현재 온라인 (Now Active)**: 앱 맨 위 제목 아래 · 떠 있는 Acrux 창 제목 옆 (● 숫자)
  - PC 버전과 같은 서버 → PC + 모바일을 합친 사용자 수 · 1분마다 새로 읽음 · 못 읽으면 --
- 설정 → 정보: '사용자 수 집계에 참여' (기본 켜짐) — 켜져 있다는 신호만 보냄 (무작위 번호 · 개인 정보 없음) · 꺼도 사용자 수는 보임

## Acrux Mobile V0.8.4 Update
- **Now Active**: under the title at the top of the app · next to the floating Acrux window title (● number)
  - Same server as the PC version → combined PC + mobile user count · refreshed every minute · -- when it cannot be read
- Settings → About: "Join the user count" (on by default) — only sends a signal that Acrux is open (a random number · no personal info) · the user count still shows when off

## Acrux Mobile V0.8.3 업데이트
- **UI 끄기 · 켜기 버튼**: 앱 맨 위 제목 오른쪽 (설정 → 조작 창에도 같은 버튼)
  - 매크로를 안 돌릴 때 떠 있는 Acrux 창을 통째로 끔 · 다시 누르면 켬 (껐다 켜도 그대로 유지)
  - 매크로가 돌고 있을 때 끄면 매크로도 멈춤 (창이 없으면 게임 안에서 멈출 수 없어서)
- 떠 있는 창의 [—] · [+] 접기 버튼 제거

## Acrux Mobile V0.8.3 Update
- **UI off · on button**: at the top right of the app next to the title (the same button is in Settings → Control window)
  - Turns the whole floating Acrux window off when you are not running the macro · tap again to turn it on (kept across restarts)
  - Turning it off while the macro is running also stops the macro (there would be no way to stop it in game without the window)
- Removed the [—] · [+] fold button on the floating window

## Acrux Mobile V0.8.2 업데이트
- 설정 → 정보: GitHub · 다운로드 버튼 제거 (후원하기만 남김)

## Acrux Mobile V0.8.2 Update
- Settings → About: removed the GitHub · Download button (only Support me remains)

## Acrux Mobile V0.8.1 업데이트
- **화면 정리 (PC 와 같은 모양)**
  - 칸 이름: 글자만 · 고른 칸은 밝은 글자 + 아래 파란 줄 · 칸 이름 아래 얇은 선
  - 버튼: 테두리 · ㄱ 없이 꽉 채운 색 (파랑 · 회색 · 옅은 색)
  - 바로 가기 · 언어: 작은 회색 칸 · 고른 것만 파란 테두리
  - ㄱ 장식은 카드에만 · 입력칸은 바탕색 + 얇은 선 · 스위치는 파랑
- **조작 창 끄기**
  - 창의 [—]: 제목 줄만 남기고 접기 · [+] 로 펼치기
  - 설정 → 조작 창 → '조작 창 보이기': 완전히 끄기
  - 'Acrux 앱을 볼 때는 숨기기' (기본 켜짐): 앱 화면을 보고 있을 땐 조작 창이 안 가림

## Acrux Mobile V0.8.1 Update
- **Cleaner screen (same look as PC)**
  - Tab names: text only · the selected tab has bright text + a blue underline · thin line under the tab names
  - Buttons: solid colors without border or ㄱ corners (blue · gray · light tint)
  - Jump bar · language: small gray boxes · only the selected one has a blue border
  - ㄱ corners only on cards · inputs have a background color + thin line · switches are blue
- **Turn the control window off**
  - [—] on the window: fold it down to the title row · [+] to unfold
  - Settings → Control window → "Show the control window": turn it off completely
  - "Hide it while the Acrux app is open" (on by default): the control window no longer covers the app screen

## Acrux Mobile V0.8.0 업데이트
- **언어 선택 (PC 와 같음)**: 한국어 · English · 日本語 — 설정 → 언어 · 떠 있는 Acrux 창 · 상태 · 기록도 같이 바뀜
  - 처음 켜면 폰 언어로 (한국어 · 일본어 · 그 밖엔 영어)
  - PC 버전 번역 사전을 그대로 씀 (빌드할 때마다 PC 사전에서 다시 가져옴) + 모바일 전용 사전
- **칸 나누기 (PC 와 같음)**: 홈 · 낚시 · 이동 · 판매 · 기록 · 설정
  - 위쪽 칸 이름을 누르거나 화면을 좌우로 밀어서 이동 · 마지막에 본 칸을 기억
  - 묶음이 많은 칸은 위쪽에 바로 가기 (누르면 부드럽게 이동 · 지금 보는 묶음 표시)
  - 새 버전이 있으면 홈 맨 위에 알림 (누르면 바로 업데이트)
- **떠 있는 Acrux 창**
  - 제목 ACRUX MACRO · 다른 버튼처럼 얇은 테두리 + 네 모서리 ㄱ
  - 어디를 잡고 끌어도 움직임 (짧게 누르면 버튼 · ⚙ 메뉴 안에서 위아래로 밀면 메뉴 스크롤)
  - 설정 → 조작 창: 투명도 · 위치 되돌리기
- 설정 → 정보: GitHub · 다운로드 · 후원하기
- 기록 칸: 최근 300줄 · 글자 선택(복사) · 기록 지우기

## Acrux Mobile V0.8.0 Update
- **Language selection (same as PC)**: 한국어 · English · 日本語 — Settings → Language · the floating Acrux window, status and log change too
  - The first launch follows the phone language (Korean · Japanese · otherwise English)
  - Uses the PC version's translation dictionary as is (pulled from the PC dictionary on every build) + a mobile dictionary
- **Tabs (same as PC)**: Home · Fishing · Movement · Selling · Log · Settings
  - Tap a tab name at the top or swipe left/right · the last tab is remembered
  - Tabs with many groups get a jump bar at the top (smooth scroll · shows the current group)
  - When a new version is available, a notice appears at the top of Home (tap to update)
- **Floating Acrux window**
  - Title ACRUX MACRO · thin border with ㄱ corners like the other buttons
  - Drag it from anywhere (a short tap still presses a button · swiping up/down inside the ⚙ menu scrolls the menu)
  - Settings → Control window: opacity · reset position
- Settings → About: GitHub · Download · Support me
- Log tab: last 300 lines · selectable text (copy) · clear log

## Acrux Mobile V0.7.0 업데이트
- **자동 업데이트**: 앱을 켜면 GitHub 에서 새 버전을 확인 → 있으면 받아서 바로 설치 화면을 띄움 (설정 · 위치는 그대로)
  - 앱 맨 위 '업데이트' 칸: 지금 버전 · 새 버전 · 바뀐 점 · [확인] / [업데이트] · '앱을 켤 때 자동으로 업데이트' 끄고 켜기
  - 받은 파일은 sha256 으로 확인한 뒤에만 설치
  - 처음 한 번은 'Acrux Mobile 에서 앱 설치 허용'을 켜야 함 (자동으로 그 설정 화면을 띄움)
- **서명 키 고정**: 이제 새 버전을 지우지 않고 덮어쓰기 설치 · 설정이 유지됨
  - 이번 V0.7.0 만 한 번 지우고 새로 설치해야 함 (그 전 버전은 빌드할 때마다 서명이 달랐음)

## Acrux Mobile V0.7.0 Update
- **Auto update**: when the app opens it checks GitHub for a new version → if there is one, it downloads it and shows the install screen right away (settings and positions are kept)
  - "Update" box at the top of the app: current version · new version · what changed · [Check] / [Update] · toggle "Update automatically when the app opens"
  - The downloaded file is installed only after its sha256 matches
  - The first time, "Allow app installs from Acrux Mobile" must be turned on (the settings screen opens automatically)
- **Fixed signing key**: new versions now install over the old one without uninstalling · settings are kept
  - Only this V0.7.0 needs one uninstall and a fresh install (earlier builds had a different signature every time)

## Acrux Mobile V0.6.5 업데이트
- **쓰레기(Fish Caught...?)가 잡히면 결과창을 못 닫고 멈추던 문제 수정** (PC V2.5.2 와 같음)
  - 결과창(흰 꺾쇠 창) 자체를 먼저 찾아서 그 창의 오른쪽 위 X 를 누름 · 바탕이 밝거나 크기 · 위치가 달라도 됨
  - X 가 안 보이면 창 크기 비율로 계산한 자리를 누름

## Acrux Mobile V0.6.5 Update
- **Fixed fishing stopping because the result window could not be closed after catching junk (Fish Caught...?)** (same as PC V2.5.2)
  - The result window itself (white cornered window) is found first and its top-right X is tapped · works with a bright background or a different size / position
  - If the X is not visible, the spot calculated from the window size is tapped

## Acrux Mobile V0.6.4 업데이트
- **여러 화면 비율 · 해상도에서 인식 강화** (스크린샷 6장을 246가지로 변형해서 시험 · 125 → 231 통과)
  - 해상도 0.5 ~ 1.6배 · 노치 · 위아래 · 양옆 검은 띠 · 초광각 · 정사각형 · 태블릿 비율 · 가장자리 잘림 · 저화질 JPEG · 흐림 · 가로세로 늘림
- **보라색 구간 (Purple zone) 을 못 보던 문제 수정**: 모바일 바 테두리가 두꺼워서 (3 ~ 4줄) 바 높이를 4px 로 잘못 잡았음
- 바 테두리가 ◇ · 글자에 가려 끊기거나 흐릿해도 바 전체 폭을 잡음
- 창 찾기
  - 화면 가운데에 뜨는 창만 인정 (순위표 · Roll · Quick roll 버튼 테두리를 창으로 착각하지 않음)
  - 미니게임 창은 안에 릴링 바가 있고 Fish / Exit 버튼이 없어야 인정 (낚시 창과 헷갈리지 않음)
  - 비율이 다른 화면: 모양 비율을 넓게 보고 안의 버튼 · 바 · X 로 확인
  - 꺾쇠가 어둡게 찍히는 화면: 흰색 기준을 낮춰서 한 번 더 찾음

## Acrux Mobile V0.6.4 Update
- **Stronger recognition across screen ratios and resolutions** (6 screenshots turned into 246 variants · passing went from 125 to 231)
  - 0.5 to 1.6× resolution · notch · black bars top/bottom and left/right · ultra-wide · square · tablet ratio · cropped edges · low-quality JPEG · blur · stretched width or height
- **Fixed the purple zone not being seen**: the mobile bar border is thick (3 to 4 rows), so the bar height was taken as 4 px
- The full bar width is found even when its border is broken by ◇ or text, or blurry
- Window search
  - Only windows centered on screen count (the leaderboard and the Roll / Quick roll button borders are no longer taken for windows)
  - The minigame window must contain a reel bar and no Fish / Exit button (no mix-up with the fishing window)
  - Screens with other ratios: the shape ratio is checked loosely and confirmed by the button, bar or X inside
  - Screens where the corners come out darker: searched again with a lower white threshold

## Acrux Mobile V0.6.3 업데이트
- **릴링 중에 Roll 버튼을 누르던 문제 수정**
  - 릴링 클릭 자리가 PC 처럼 미니게임 창 아래 빈 화면이었는데, 모바일은 UI 가 커서 그 자리가 Roll 버튼 바로 위 (몇 px 차이)였음
  - 이제 미니게임 창 위쪽 빈 화면 (캐릭터 쪽)을 누름 · 세부 설정의 '릴링 때 미니게임 창 위쪽을 누름' 으로 끄고 켤 수 있음
- **오른쪽 위 Player · Rolls 순위표를 낚시 창으로 잘못 잡던 문제 수정**
  - 순위표도 꺾쇠 창이고 모양 비율이 낚시 창과 비슷함 → 이제 안에 Fish / Exit 버튼이 보여야 낚시 창으로 인정

## Acrux Mobile V0.6.3 Update
- **Fixed the Roll button being pressed while reeling**
  - The reel click spot was the empty screen under the minigame window like on PC, but on mobile the UI is larger and that spot was right on top of the Roll button (a few px away)
  - It now clicks the empty screen above the minigame window (character side) · toggle it with "Click above the minigame window while reeling" in the detailed settings
- **Fixed the Player · Rolls leaderboard at the top right being taken for the fishing window**
  - The leaderboard is also a cornered window with a similar shape → now a window only counts as the fishing window when a Fish / Exit button is visible inside it

## Acrux Mobile V0.6.2 업데이트
- **Sell All 이 처음에 안 눌리던 문제 수정**
  - 상점을 연 뒤 처음 누르는 Sell All 이 씹히면 바로 '팔 물고기 없음'으로 보고 판매를 끝냈음
  - 이제 확인창이 안 뜨면 최대 3번 다시 누름: 호버 후 클릭 → 오래 호버 후 클릭 → 손가락 탭
  - 3번 다 안 떠야 팔 물고기가 없는 것으로 봄 · 확인 Sell 버튼도 마지막엔 손가락 탭으로
  - 판매 자동 보정에서 Sell All 을 누를 때도 같음

## Acrux Mobile V0.6.2 Update
- **Fixed Sell All not being clicked the first time**
  - If the first Sell All after opening the shop was swallowed, selling ended right away as "no fish to sell"
  - Now it presses again up to 3 times when the confirm window does not appear: hover then click → long hover then click → finger tap
  - Only when all 3 fail is it treated as no fish · the confirm Sell button also falls back to a finger tap on the last try
  - The same applies to Sell All during sell auto calibration

## Acrux Mobile V0.6.1 업데이트
- **모바일에서 낚시 창 · 미니게임 창을 못 찾던 문제 수정**
  - 모바일 화면은 창 꺾쇠(흰색) 바깥에 회색 테두리가 한 겹 더 있고, 그 안에 거의 흰색인 점들이 섞여 있음
  - 그 점들이 꺾쇠 모서리에 붙어서 창 모서리로 안 보였음 → 더 밝은 기준으로 한 번 더 찾게 함
  - 보정 중 "릴링 중"에서 멈춰 있던 경우가 이것 때문

## Acrux Mobile V0.6.1 Update
- **Fixed the fishing window and minigame window not being found on mobile**
  - On mobile the white window corners have an extra gray outline with almost-white specks in it
  - Those specks touched the corners so they no longer looked like window corners → frames are now searched again with a brighter threshold
  - This is why calibration got stuck at "Reeling"

## Acrux Mobile V0.6.0 업데이트
- **다시 Shizuku 방식으로**: 키보드 방식은 로블록스가 키를 안 받아서 뺌 · 키(Esc · R · Enter · W · A · S · D · E · O · /)와 마우스(우클릭 Click to Move · Sell All 호버 후 클릭 · 우클릭 드래그)를 Shizuku 로 PC 와 똑같이 보냄
  - 앱 → 준비 → ③ Shizuku [연결] (Shizuku 설치 → 무선 디버깅으로 시작 → 허용)
  - 자동 낚시만 쓸 거면 Shizuku 없이도 됨 (화면 터치)
- Acrux 창 ⚙ → 키 테스트: W 를 1초 눌러서 Shizuku 키 입력이 되는지 바로 확인
- Acrux 키보드 · 접근성 입력기 제거

## Acrux Mobile V0.6.0 Update
- **Back to Shizuku**: removed the keyboard method because Roblox did not accept its keys · keys (Esc · R · Enter · W · A · S · D · E · O · /) and mouse (right-click Click to Move · Sell All hover then click · right-drag) go through Shizuku exactly like on PC
  - App → Setup → ③ Shizuku [Connect] (install Shizuku → start via wireless debugging → allow)
  - Auto fishing alone still works without Shizuku (screen taps)
- Acrux window ⚙ → Key test: holds W for 1 s to check right away whether Shizuku key input works
- Removed Acrux Keyboard and the accessibility input method

## Acrux Mobile V0.5.0 업데이트
- **이동 · 판매 입력을 키보드 방식 하나로 정리**: 키(Esc · R · Enter · W · A · S · D · E · O · / · Space)를 키보드로 진짜 키 신호로 보냄
  - Acrux 키보드 추가: 설정에서 켜고 지금 키보드로 선택하면 이 키보드가 로블록스에 키를 보냄
  - 안드로이드 13 이상은 접근성 입력기로도 한 번 더 시도함 (키보드를 안 바꿔도 됨)
  - 클릭은 키보드로 못 보내서 화면 탭 (우클릭 Click to Move · Sell All 등은 0.12초 꾹 눌렀다 뗌) · 우클릭 드래그는 화면 끌기
  - Acrux 창 ⚙ → 키 테스트: W 를 1초 눌러서 키보드 방식이 되는지 바로 확인
- Shizuku · 터치 모드(조이스틱 · 메뉴 리셋 · 핀치) 제거

## Acrux Mobile V0.5.0 Update
- **Movement and selling input reduced to the keyboard method only**: keys (Esc · R · Enter · W · A · S · D · E · O · / · Space) are sent as real key signals through a keyboard
  - Added Acrux Keyboard: enable it in settings and pick it as the current keyboard so it sends keys to Roblox
  - On Android 13+ the accessibility input method is tried as well (no keyboard switch needed)
  - Clicks cannot go through a keyboard, so they are screen taps (right-click Click to Move, Sell All and so on are held for 0.12 s) · right-drag becomes a screen drag
  - Acrux window ⚙ → Key test: holds W for 1 s to check right away whether the keyboard method works
- Removed Shizuku and touch mode (joystick, menu reset, pinch)

## Acrux Mobile V0.4.0 업데이트
- **Shizuku 없이도 이동 · 판매 가능 (터치 모드)**: Shizuku 가 연결 안 돼 있으면 PC 키 · 마우스를 로블록스 모바일 조작으로 바꿔서 누름
  - W A S D → 조이스틱을 그 방향으로 밀고 있기 (W+A 같은 대각선도 · 키를 바꿀 때 손가락을 안 뗌)
  - Esc → R → Enter → 메뉴 버튼 → Reset Character → Reset (글자로 찾음)
  - 우클릭 → Tap to Move · E → 상호작용 버튼 · Space → 점프 버튼
  - O + 우클릭 드래그 → 화면 끌어 내려다보기 + 두 손가락 줌 아웃
  - Sell All 같은 버튼은 0.12초 동안 눌렀다 뗌 (짧게 누르면 호버만 되고 안 눌리던 문제)
  - 조이스틱 · 메뉴 · 점프 버튼은 기본 자리가 있음 · 상호작용 버튼은 Acrux 창 ⚙ → 터치 조작에서 지정
  - 앱에서 탭 누르는 시간 · 내려다보기 배율 조절
- **키보드 앱 방식 키 (실험 · 안드로이드 13+)**: 켜면 키를 접근성 입력기로 먼저 보내 봄 · 로블록스가 받아 주면 조이스틱 대신 진짜 키로 움직임
- Shizuku 가 연결돼 있으면 지금처럼 PC 와 똑같은 키보드 · 마우스로 움직임

## Acrux Mobile V0.4.0 Update
- **Movement and selling without Shizuku (touch mode)**: when Shizuku is not connected, PC keys and mouse are turned into Roblox mobile controls
  - W A S D → hold the joystick in that direction (diagonals like W+A too · the finger stays down when keys change)
  - Esc → R → Enter → menu button → Reset Character → Reset (found by text)
  - Right-click → Tap to Move · E → interact button · Space → jump button
  - O + right-drag → drag the screen to look down + two-finger zoom out
  - Buttons like Sell All are held for 0.12 s (short taps only hovered and did not click)
  - Joystick, menu and jump buttons have default spots · set the interact button in the Acrux window ⚙ → Touch controls
  - Tap hold time and look-down scale are adjustable in the app
- **Keyboard-app style keys (experimental · Android 13+)**: when on, keys are first sent through the accessibility input method · if Roblox accepts them, it moves with real keys instead of the joystick
- With Shizuku connected it still moves with the same keyboard and mouse as on PC

## Acrux Mobile V0.3.0 업데이트
- **PC 와 똑같은 입력**: Shizuku 로 진짜 키보드 · 마우스 신호를 보냄 (Esc · R · Enter · W · A · S · D · E · O · / · 우클릭)
  - 앱 → 준비 → ③ Shizuku [연결] (Shizuku 설치 → 무선 디버깅으로 시작 → 허용)
  - 자동 낚시만 쓸 거면 Shizuku 없이도 됨 (그때는 화면 터치)
- **Sell All 이 안 눌리던 문제 수정**: PC 처럼 버튼 위에 마우스를 먼저 올리고 (호버) 0.05초 뒤 클릭
- **이동을 PC 버전 그대로**: 리셋 (Esc → R → Enter) → 카메라 정렬 (/ → 채팅 → 도감 → 도감 Exit) → W → W+A → A (끝에 W) → S+D → O 누른 채 우클릭 드래그로 내려다보기 · 줌 아웃
  - 낚시 장소 · 판매 장소 = Click to Move (우클릭) 지점 + 걸리는 시간 · PC 처럼 지점 추가 → 걸어가며 [도착] 으로 시간 재기 · 시간 다시 재기 · 마지막 지점 지우기
  - 이동 시간 (리셋 후 대기 · W · W+A · A · A 끝 W · S+D · 내려다보기 px · O 줌 · 걷기 여유) 을 앱에서 바꿀 수 있음
  - 조이스틱 · 핀치 · 경로 글 편집은 없앰
- **판매도 PC 버전 그대로**: 판매 장소 → E → 대화 넘기기 → Sell Fish → (첫 칸 → Sell All → 확인) 반복 → 상점 닫기 → 낚시 장소
  - 판매 자동 보정: NPC 앞에서 시작하고 E 는 직접 누르기 (PC 와 같음)
- Acrux 창 ⚙ 메뉴: Shizuku 연결 · 공통 위치 (채팅 버튼 · 도감 버튼 · 도감 Exit) · 낚시 장소 / 판매 장소 지점 · 판매 자동 보정 · 판매 한 번 · 이동 테스트

## Acrux Mobile V0.3.0 Update
- **Same input as PC**: sends real keyboard and mouse signals through Shizuku (Esc · R · Enter · W · A · S · D · E · O · / · right-click)
  - App → Setup → ③ Shizuku [Connect] (install Shizuku → start via wireless debugging → allow)
  - Auto fishing alone still works without Shizuku (screen taps)
- **Fixed Sell All not being clicked**: like on PC, the mouse hovers the button first and clicks 0.05 s later
- **Movement now matches the PC version**: reset (Esc → R → Enter) → camera align (/ → chat → collection → collection Exit) → W → W+A → A (W at the end) → S+D → hold O and right-drag to look down · zoom out
  - Fishing / sell spot = Click to Move (right-click) points + walk time · add points like on PC and time them with [Arrived] · re-measure · remove the last point
  - Movement times (wait after reset · W · W+A · A · W at end of A · S+D · look-down px · O zoom · walk margin) are editable in the app
  - Removed the joystick, pinch and text route editor
- **Selling now matches the PC version**: sell spot → E → skip dialog → Sell Fish → (first slot → Sell All → confirm) repeated → close shop → fishing spot
  - Sell auto calibration: start in front of the NPC and press E yourself (same as PC)
- Acrux window ⚙ menu: Shizuku connect · common positions (chat button · collection button · collection Exit) · fishing / sell spot points · sell auto calibration · sell once · movement tests

## Acrux Mobile V0.2.0 업데이트
- **이동** 추가 (PC 버전처럼 기준 장소 → 장소): 시작하면 낚시 장소로 가서 낚시 · 30초 동안 낚시 화면이 안 보이면 다시 이동 (2번까지)
  - 경로 = 한 줄에 동작 하나: 리셋 (메뉴 → Reset Character → Reset · 글자로 찾음) · 대기 · 카메라정렬 · 이동(조이스틱 W A S D WA … · 쉼표로 이어서) · 화면내리기 · 화면돌리기 · 줌아웃 · 줌인 · 점프 · 터치 · 꾹 · 터치이동(Tap to Move) · 상호작용(E 대신)
  - 기준 장소 기본 경로는 PC 와 같은 순서 (리셋 → 카메라 정렬 → W 1초 → W+A 7초 → A 1초 → S+D → 내려다보기 → 줌 아웃)
  - 앱에서 경로 편집 · 잘못 적은 줄은 바로 알려 줌 · 필요한 위치가 빠지면 알려 줌
- **판매** 추가 (PC 판매와 같은 순서): 인벤토리가 가득 차면 기준 장소 → 판매 장소 → 상호작용 → 대화 넘기기 → Sell Fish → (첫 칸 → Sell All → 확인 Sell) 반복 → 상점 닫기 → 낚시 장소
  - 다 팔았는지는 확인창(초록 Sell)으로 · 두 번 연속 팔 물고기가 없으면 낚시 자리 문제로 보고 멈춤
  - 판매 보정: NPC 앞에서 상호작용 버튼을 누르면 대화창 · Sell Fish · 상점 · 확인창 위치를 글자로 찾아 맞춤 (팔지는 않음)
  - 글자 읽기는 폰 안에서만 (구글 ML Kit · 인터넷 안 씀)
- Acrux 창 ⚙ 메뉴: 위치 지정 (로블록스 화면에서 누른 곳) · 지점 추가 (Tap to Move 로 걸어가며 시간 재기 → 경로에 자동 추가) · 판매 보정 · 판매 해 보기 · 이동 테스트

## Acrux Mobile V0.2.0 Update
- Added **movement** (like the PC version, base spot → place): goes to the fishing spot when starting · moves again if the fishing screen is not seen for 30 s (up to 2 times)
  - Route = one action per line: reset (menu → Reset Character → Reset · found by text) · wait · camera align · move (joystick W A S D WA … · chain with commas) · tilt down · rotate · zoom out · zoom in · jump · tap · hold · tap-to-move · interact (instead of E)
  - The default base route follows the PC order (reset → camera align → W 1 s → W+A 7 s → A 1 s → S+D → look down → zoom out)
  - Edit routes in the app · wrong lines and missing positions are reported right away
- Added **selling** (same order as the PC version): when the inventory is full → base spot → sell spot → interact → skip dialog → Sell Fish → (first slot → Sell All → confirm Sell) repeated → close shop → fishing spot
  - Detects when everything is sold via the confirm window (green Sell) · stops if there is nothing to sell twice in a row
  - Sell calibration: interact with the NPC and it finds the dialog, Sell Fish, shop and confirm positions by text (does not sell)
  - Text reading runs on the phone only (Google ML Kit · no internet)
- Acrux window ⚙ menu: set positions (tap on the Roblox screen) · add points (walk with tap-to-move and time it → added to the route) · sell calibration · try selling · movement tests
