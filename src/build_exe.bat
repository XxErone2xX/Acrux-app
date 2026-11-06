@echo off
chcp 65001 >nul
cd /d "%~dp0"
title exe 만들기
where py >nul 2>nul && (set PY=py) || (set PY=python)

echo [1/2] 필요한 패키지 설치 중...
%PY% -m pip install -q websocket-client mss winrt-runtime winrt-Windows.Media.Ocr winrt-Windows.Graphics.Imaging winrt-Windows.Storage.Streams winrt-Windows.Foundation winrt-Windows.Foundation.Collections winrt-Windows.Globalization rapidocr_onnxruntime onnxruntime zstandard pyinstaller || goto :fail

echo [2/2] AcruxMacro.exe 만드는 중... (2~4분)
%PY% -m PyInstaller --noconfirm --onefile --windowed --name AcruxMacro ^
  --add-data "hook.js;." --add-data "web;web" --hidden-import websocket --hidden-import mss --hidden-import tkinter --collect-submodules winrt --collect-all rapidocr_onnxruntime --collect-binaries onnxruntime app.py || goto :fail

copy /y dist\AcruxMacro.exe . >nul
rmdir /s /q build dist 2>nul
del /q AcruxMacro.spec 2>nul
echo.
echo 완료: AcruxMacro.exe  (이 파일 하나만 있으면 됨. 설정은 옆에 config.json 으로 저장됨)
pause
exit /b

:fail
echo.
echo 실패함. 위 에러 메시지를 확인해줘.
pause
