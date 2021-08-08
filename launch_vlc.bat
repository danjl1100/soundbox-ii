@echo off

if "%VLC_BIND_HOST%" =="" (echo "VLC_BIND_HOST is not set" & goto :EOF)
if "%VLC_PORT%" =="" (echo "VLC_PORT is not set" & goto :EOF)
if "%VLC_PASSWORD%" =="" (echo "VLC_PASSWORD is not set" & goto :EOF)

set ARGS=--audio-replay-gain-mode track --http-host %VLC_BIND_HOST% --http-port %VLC_PORT% --http-password %VLC_PASSWORD%

if not "%1" =="-v" (goto :HEADLESS)

echo NOTE Need to click menu:  View - Add Interface - Web
echo.
echo Press enter to launch visual interface
pause > nul
start "" vlc %ARGS%
goto :EOF

:HEADLESS
call vlc -I dummy --dummy-quiet --intf http %ARGS%
