@echo off
chcp 65001 >nul
cd /d "%~dp0"
title Acrux macro V1.2.1
echo 필요한 파일 다운로드 및 초기 셋팅중...
echo Downloading required files and setting up...
echo.
where py >nul 2>nul && (set PY=py) || (set PY=python)
%PY% -c "import websocket, mss, winrt.windows.media.ocr" 2>nul || %PY% -m pip install -q websocket-client mss winrt-runtime winrt-Windows.Media.Ocr winrt-Windows.Graphics.Imaging winrt-Windows.Storage.Streams winrt-Windows.Foundation winrt-Windows.Foundation.Collections winrt-Windows.Globalization
rem RapidOCR (stronger OCR). If this fails, Windows OCR is used instead.
%PY% -c "import rapidocr_onnxruntime, zstandard" 2>nul || %PY% -m pip install -q rapidocr_onnxruntime onnxruntime zstandard
echo 완료 - 프로그램을 실행합니다.
echo Done - starting the program.
start "" %PY%w app.py 2>nul || start "" %PY% app.py
