## Acrux macro V2.0.9 업데이트
- 물고기 판매: 선택지가 떴는데도 팔지 않고 창이 닫히던 문제 수정 — 대화를 넘길 때 따로 연타하던 걸, 누를 때마다 선택지가 떴는지 먼저 확인하도록 바꿈 (선택지가 뜬 뒤에 더 눌려서 다른 선택지가 눌리지 않게)
- 물고기 판매: Sell All 뒤 확인창이 지정한 자리에 안 보이거나 늦게 뜨면, 바로 '물고기 없음' 으로 보고 닫던 것 → 글자(Sell · Cancel)로 한 번 더 찾아서 누름
- 상인 자동 구매의 대화 넘기기도 같은 방식으로 바뀜

## Acrux macro V2.0.9 Update
- Fish selling: fixed the dialog closing without selling even though the choices appeared — dialog skipping no longer spam-clicks in the background; it checks for the choices after every click (so no extra click lands on another choice)
- Fish selling: if the confirm window after Sell All is late or not at the saved position, it no longer assumes there are no fish and closes — it looks for the Sell · Cancel text once more and clicks Sell
- Merchant auto buy uses the same dialog skipping
