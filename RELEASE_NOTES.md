## Acrux macro V1.25.3 업데이트
- 물고기 판매 중 남은 물고기 확인을 OCR 대신 **픽셀 검사**로 바꿈 (훨씬 가볍고 빠름)
  - 물고기를 안 골랐을 때 이름 자리에 뜨는 '...' (흰 네모 3개) 를 찾아서 다 팔았는지 판단
  - 픽셀로 판단이 안 될 때만 OCR 을 한 번 씀

## Acrux macro V1.25.3 Update
- Checking for remaining fish while selling now uses a **pixel check** instead of OCR (much lighter and faster)
  - Finds the '...' (three white squares) shown in the name spot when no fish is selected to tell when everything is sold
  - OCR is used once only when the pixels can't decide
