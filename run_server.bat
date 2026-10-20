@echo off
REM +++++++++++
REM * gaucho - conquer mobile private server
REM * with love from bolzov (ᵔᴥᵔ)
REM * https://github.com/ibolzov
REM +++++++++++
setlocal
if not defined ADB set ADB=D:\AndroidSDK\platform-tools\adb.exe
if not defined DEV set DEV=emulator-5554

for /f "tokens=5" %%p in ('netstat -ano ^| findstr ":8020" ^| findstr LISTENING') do taskkill /F /PID %%p >nul 2>&1
for /f "tokens=5" %%p in ('netstat -ano ^| findstr ":5816" ^| findstr LISTENING') do taskkill /F /PID %%p >nul 2>&1
for /f "tokens=5" %%p in ('netstat -ano ^| findstr ":9528" ^| findstr LISTENING') do taskkill /F /PID %%p >nul 2>&1

"%ADB%" -s %DEV% reverse tcp:8020 tcp:8020 >nul 2>&1
"%ADB%" -s %DEV% reverse tcp:5816 tcp:5816 >nul 2>&1
"%ADB%" -s %DEV% reverse tcp:9528 tcp:9528 >nul 2>&1

if "%~1"=="" (
  set CQ2_BIND_HOST=127.0.0.1
) else (
  set CQ2_BIND_HOST=127.0.0.1,%~1
)

cd /d "%~dp0rust"
cargo run --release
pause
endlocal

