# 개발 앱의 데스크톱 모드 SDK

`dev`의 데스크톱 앱은 정식 Python 앱의 `src/mods.py`를 확장 SDK로 재사용한다. 매크로 본체와 게임 입력·캡처는 Rust에 있으며, Python 확장은 별도 실행기에서 동작한다. Android에는 Python 모드 화면이나 실행기를 추가하지 않는다.

## 설치와 화면

기존 `.acxm`, `.acrux`, `.zip` 패키지를 모드 화면에서 선택한다. `mod.json`과 `setup(ctx)` 진입점은 기존 형식을 유지한다. `ctx.api`, `ctx.on`, `ctx.config_keys`, `ctx.settings`, `ctx.save`, `ctx.bridge`, `ctx.core`도 같은 SDK를 사용한다. 설치·활성화·삭제 후에는 다시 시작해야 적용된다.

패키지는 압축 전후 각각 30MiB로 제한하고, 폴더 밖 경로·심볼릭 링크·퇴역한 sniping 패키지를 거부한다. 기존 설치를 교체하기 전에 새 패키지와 진입점을 검사한다. 모드의 화면 파일은 앱의 `acruxmod` 경로로 제공한다. 설치한 Python은 사용자 권한으로 실행되는 확장 코드다.

시스템 Python 3.10 이상이 있으면 사용한다. 없으면 공식 python-build-standalone의 해당 OS·아키텍처 런타임과 SHA256을 확인한 뒤 앱 데이터 폴더에 준비한다. 모드가 추가 Python 패키지를 요구하면 해당 import 오류를 모드 목록에 표시하며, 실행 성공으로 처리하지 않는다.

## 이벤트와 설정

기존 `ready()`는 모드를 모두 불러온 뒤 한 번 실행한다. 실행 관련 이벤트는 크기가 제한된 단일 대기열을 통해 처리한다. 확장이 로그를 남길 때 동일한 로그 이벤트를 재귀 전송하지 않는다. 게임 처리의 잠금을 잡은 채 Python 응답을 기다리지 않는다.

`state(out)`과 `poll(out, since)`의 `out` 변경은 실제 PC 화면 응답에 반영한다. `stats()`가 반환한 숫자 사전은 이번 실행 통계에 합친다. `macro(on)` 및 `start`·`stop` 이벤트는 `core.STOP`에도 반영된다. 앱 종료에서는 실행 중인 native 작업을 먼저 중지한 뒤 확장의 `stop/shutdown`을 최대 3초 기다리고 자식 실행기를 종료한다. 종료 훅은 설정 저장이 가능하지만 새 게임 입력은 거절한다. 실제 게임의 바이옴·입장·복귀 이벤트는 공통 제어기에서 전달한다.

설정 변경은 네이티브 저장소에 기록된다. 실행 중 다른 작업이 설정을 사용하고 있으면 저장 오류를 돌려준다. PC·모바일 기본 UI는 설정을 바꾸기 전에 중지하고, 아직 실행 의도가 남아 있을 때만 한 번 재시작한다. 사용자의 정지는 저장 완료보다 우선한다.

## 네이티브 입력·관측 창구

확장 코드의 `core`·`macro` 호출은 공통 네이티브 입력 소유자에게 전달된다. 이전 Python 엔진의 호스트 키보드 입력이나 전경 캡처를 다시 실행하지 않는다. 지원하지 않는 구형 함수를 호출하면 구체적인 오류를 돌려준다. 전역 창 핸들 조회, `focus`, `bring_front` 등 호스트의 전경 전환은 거절한다.

| 함수 | 입력과 반환 |
| --- | --- |
| `tap(x, y, 40)` | 게임 화면의 0~1 좌표. 실제 입력 완료 확인 후 반환 |
| `click(x, y, ...)` | 게임 화면 기준 픽셀 좌표. 호스트 바탕화면 좌표가 아님 |
| `key_tap(name, 40)` | 알파벳·숫자·Space·Enter·Escape·Tab·Backspace 등 지원 키 |
| `gesture(value)` | `kind`가 `tap`·`drag`·`pinch`·`stick`인 공통 제스처. 좌표는 게임 기준 0~1, 입력은 1~1000ms |
| `text(value, submit=False)` | 게임에 문자열 입력. `type_text`·`paste`도 같은 네이티브 경로 |
| `open_link(url)` | 허용된 Roblox 입장 주소 실행 |
| `ocr_region(rect=None)` | 새 관측의 글자를 문자열로 반환 |
| `ocr_boxes(rect=None)` | `[text, center_x, center_y, width, height]` 목록. 좌표는 게임 기준 0~1 |
| `grab([left, top, width, height])` | 게임 기준 픽셀 영역의 `(BGRA bytes, width, height)` 튜플 |
| `ocr_engine()` / `ocr_engine_name()` | 실제 기기 OCR 엔진 이름 |
| `client_rect()` | 관측한 게임의 `[0, 0, width, height]` |

OCR 영역은 `[left, top, right, bottom]`의 0~1 좌표다. 캡처 영역은 최대 300만 픽셀이다. 캡처 응답의 내부 base64 표시는 확장 실행기가 원래 BGRA 튜플로 복원하므로 모드 코드에서 처리할 필요가 없다.

네이티브 요청은 실행 세대와 실제 입력 완료를 검사한다. 네이티브 응답 대기는 25초, Python의 개별 RPC 대기는 30초다. 타임아웃·중지·창 변경으로 취소된 입력은 성공으로 재시도하지 않는다.

## 업데이트와 검사

개발 앱의 업데이트는 정식 앱의 `latest`·`mobile-latest`와 분리한 `dev-alpha`만 사용한다. dev의 Linux·Windows·Android·macOS 빌드가 모두 성공한 뒤 한 게시 작업에서 실제 자산 및 `.sha256`을 올린다. main의 기존 `alpha`는 이 앱의 업데이트 대상이 아니다. 앱은 SHA256을 검사하고, Android에서는 설치 패키지명과 서명도 검사한다. macOS는 압축 자산과 설치된 실행 파일의 해시를 함께 보관해 같은 업데이트를 반복 제안하지 않는다.

현재 추가한 검사는 실제 `setup(ctx)`, 등록 API, 이벤트, 설정 왕복, BGRA 응답 복원과 패키지 교체·경로 검증을 다룬다. 실제 확장 설치·네트워크 다운로드·각 OS 설치 화면은 이 단위 검사의 완료만으로 실기 검증됐다고 보지 않는다.
