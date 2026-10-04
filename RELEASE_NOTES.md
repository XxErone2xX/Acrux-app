## Acrux macro V1.11.0 업데이트
- 매크로 기능 설정에 **자동 낚시** 추가 (1단계: 제자리 낚시)
  - Fish 클릭 → 입질 대기 → 릴링(구간 안에 표시가 머물도록 자동 클릭) → 결과창 닫기 를 반복
  - 결과(성공 / 쓰레기 / 실패)를 세서 로그에 표시
  - Fish 를 눌러도 반응이 없으면(최대 3번 재확인) 인벤토리가 가득 찬 것으로 보고 정지 — 자동 판매 · 이동은 다음 업데이트에서 추가 예정
  - 위치 템플릿 [16:9 적용] 으로 Fish 버튼 · 낚시 바 · 결과창 위치를 한 번에 채울 수 있고, 직접 지정도 가능
  - [상태 확인] 으로 지금 화면에서 버튼 · 낚시 바를 제대로 읽는지 확인 가능
  - 레어 바이옴 자동 팝핑이 시작되면 낚시를 잠시 멈추고, 끝나면 다시 이어서 진행
  - 스나이프 중에는 동작하지 않음 · F7 로 정지
- 매크로 탭 · 팝핑 설정을 여러 번 바꾸면 처음 바꾼 것만 저장되고 이후 변경이 사라지던 문제 수정

## Acrux macro V1.11.0 Update
- Added **Auto Fishing** to Macro Features (step 1: fishing in place)
  - Repeats: click Fish → wait for a bite → reel (auto-clicks to keep the marker inside the zone) → close the result
  - Counts results (success / junk / fail) and shows them in the log
  - If Fish does nothing (rechecked up to 3 times), the inventory is treated as full and fishing stops — auto selling and movement are coming in a later update
  - The position template [Apply 16:9] fills the Fish button, fishing bar and result positions at once; you can also set them yourself
  - [Check status] shows whether the button and fishing bar are read correctly on the current screen
  - Pauses while rare biome auto popping runs, then resumes
  - Does not run while sniping · F7 to stop
- Fixed only the first change being saved when Macro tab or popping settings were edited several times
