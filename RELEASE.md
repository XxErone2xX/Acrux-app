# Acrux macro — 배포 / 자동 업데이트

사용자는 **Acrux.exe 하나만** 받아서 실행합니다. 나머지 파일은 전부
`%LOCALAPPDATA%\AcruxMacro` 안에 들어가고, 켤 때마다 이 레포(Acrux-app)의 최신 릴리스를 확인해서 자동으로 업데이트합니다.

## 폴더 구조

| 경로 | 내용 |
| --- | --- |
| `src/` | 앱 코드 (app.py, web/ …) — 평소 수정하는 곳 |
| `src/version.py` | `VERSION` (앱 버전) · `RUNTIME` (라이브러리 버전) |
| `launcher/launcher.py` | 실행기 Acrux.exe — `LAUNCHER_VERSION` |
| `launcher/host.py` | 런타임 AcruxHost.exe (파이썬 + 라이브러리) |
| `launcher/pack.py` | 릴리스 파일 묶기 |
| `.github/workflows/release.yml` | 윈도우에서 빌드 → 릴리스 |

## 새 버전 올리기

1. `src/` 수정
2. `src/version.py` 의 `VERSION` 올리기 (예: `"1.2.2"`)
3. `RELEASE_NOTES.md` 를 이번 버전 내용으로 바꾸기 — 릴리스 설명에 그대로 들어감
   (앱의 Acrux 탭 → 업데이트 로그도 이 릴리스 설명을 GitHub 에서 가져와 보여줌 · 제목 형식을 그대로 지킬 것)
   ```
   ## Acrux macro V1.2.2 업데이트
   - ○○ 추가
   - ○○ 삭제

   ## Acrux macro V1.2.2 Update
   - Added ○○
   - Removed ○○
   ```
4. 커밋 · 푸시
5. GitHub → **Actions** → **Release** → **Run workflow**
6. 몇 분 뒤 `v1.2.2` 릴리스가 생기면 끝 — 사용자들은 다음에 켤 때 자동으로 받습니다 (앱 코드만, 몇 MB)

## 이럴 때만 숫자를 더 올리기

- **pip 패키지(라이브러리)를 추가·변경** → `src/version.py` 의 `RUNTIME` +1
  (host.py 맨 위 import 목록에도 그 패키지를 추가) → 사용자들이 런타임(약 100MB+)을 한 번 다시 받음
- **실행기(launcher.py)를 수정** → `LAUNCHER_VERSION` +1 → 사용자들의 Acrux.exe 가 스스로 교체됨

## 사용자 PC 폴더 (`%LOCALAPPDATA%\AcruxMacro`)

| 폴더 | 내용 |
| --- | --- |
| `app\` | 앱 코드 (업데이트 때 통째로 교체) |
| `runtime\` | 파이썬 + 라이브러리 |
| `data\` | 설정(config.json) · 로그 — 업데이트해도 유지 |
| `ui\` | 화면(Edge) 프로필 |

- 인터넷 / GitHub 이 안 되면 설치된 버전으로 그냥 실행
- 받은 파일은 release.json 의 SHA-256 으로 확인 (다르면 설치 안 함)
- 예전 방식(run.bat 폴더)의 config.json 이 Acrux.exe 옆에 있으면 처음 한 번 자동으로 가져옴
- 개발할 때는 지금처럼 `src/run.bat` 으로 바로 실행 가능

> 바이옴 웹후크 썸네일 이미지는 따로 `rngenesis0-coder/Acrux-macro` 레포 맨 위의 png 를 씁니다.
