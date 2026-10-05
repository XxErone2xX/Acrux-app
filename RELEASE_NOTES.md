## Acrux macro V1.23.0 업데이트
- **자동 낚시 물고기 판매 추가** (매크로 기준 위치 설정 → 자동 낚시 → 판매)
  - 인벤토리가 가득 차면 (Fish 를 3번 눌러도 반응 없음 · Cannot Fish 알림): 기준 장소 → 물고기 판매 장소 → E → 대화 넘기기 → Sell Fish
  - 첫 칸 → Sell All → 확인 Sell 을 왼쪽 물고기 정보가 빌 때까지 반복 (최대 100번) → X 로 닫기 → 낚시 장소로 돌아가서 낚시 이어감
  - 대화창 · 첫 칸 · Sell All · 확인 Sell · 상점 닫기 위치는 위치 템플릿으로 채울 수 있음 (Noteab 매크로의 1920x1080 위치 값) · Sell Fish 버튼과 물고기 정보 영역은 직접 지정
  - [판매 테스트] 로 한 번 돌려 볼 수 있음 · 판매 설정이 비어 있으면 알림으로 알려 주고 낚시를 멈춤
- 이동 장소: 지점 위치와 시간을 전부 지정하면 장소 이름 옆에 **총 걸리는 시간**이 뜸
- 이동 장소의 [바로 재기] 버튼 없앰 (시간 재기는 기준 장소부터)

## Acrux macro V1.23.0 Update
- **Added fish selling to auto fishing** (Macro base position settings → Auto fishing → Selling)
  - When the inventory is full (Fish doesn't respond after 3 clicks · Cannot Fish notice): base spot → fish selling spot → E → skip dialog → Sell Fish
  - First slot → Sell All → confirm Sell repeats until the fish info on the left is empty (max 100) → close with X → back to the fishing spot and fishing resumes
  - Dialog · first slot · Sell All · confirm Sell · shop close positions can be filled by the position template (1920x1080 values from the Noteab macro) · set the Sell Fish button and fish info area yourself
  - [Sell test] runs it once · if selling settings are missing, a notification says so and fishing stops
- Movement places: once every point's position and time are set, the **total time** shows next to the place name
- Removed the [Measure now] button from movement places (time measuring starts from the base spot)
