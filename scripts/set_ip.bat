@echo off
REM ++++++++++++++++
REM * https://github.com/ibolzov
REM * gaucho - conquer mobile private server
REM * with love from bolzov ʕ•ᴥ•ʔ
REM ++++++++++++++++
setlocal
set "APK=%~1"
set "IP=%~2"
if "%APK%"=="" set /p "APK=APK (drag it here): "
if "%IP%"=="" set /p "IP=Server IP: "
set "APK=%APK:"=%"
where py >nul 2>&1 && (set PY=py) || (set PY=python)
%PY% "%~dp0..\tools\build_apk.py" %IP% --base "%APK%"
pause
endlocal
