@echo off
REM Protocol: idle calibration — 10 minutes quiet host, then export certificate.
cd /d "%~dp0.."
echo === FSOT protocol: idle (600s) ===
echo Close heavy apps if you want a clean noise floor.
"%~dp0..\fsot-pc-monitor.exe" --headless --protocol idle --export
if errorlevel 1 "%~dp0..\target\release\fsot-pc-monitor.exe" --headless --protocol idle --export
echo Done. Check exports\session_*
pause
