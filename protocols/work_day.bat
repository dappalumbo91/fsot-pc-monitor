@echo off
REM Protocol: work day — 8 hours continuous plant logging + stream, export at end.
cd /d "%~dp0.."
echo === FSOT protocol: work day (8h) ===
echo Logs plant state while you work. Ctrl+C stops early (no auto-export on Ctrl+C).
echo Optional neuron stream: add --stream to the command below.
"%~dp0..\fsot-pc-monitor.exe" --headless --protocol work --export --stream
if errorlevel 1 "%~dp0..\target\release\fsot-pc-monitor.exe" --headless --protocol work --export --stream
echo Done. Check exports\session_*
pause
