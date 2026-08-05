@echo off
REM Protocol: step load — 1 min idle, ~90s multi-core stress, continue to 5 min total, export.
cd /d "%~dp0.."
echo === FSOT protocol: step load (300s) ===
echo Will auto-spawn a PowerShell CPU load at t=60s.
"%~dp0..\fsot-pc-monitor.exe" --headless --protocol step --export
if errorlevel 1 "%~dp0..\target\release\fsot-pc-monitor.exe" --headless --protocol step --export
echo Done. Check exports\session_*  (look for excursion + recovery tau)
pause
