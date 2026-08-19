@echo off
REM Avvia NOVA in modalita' testuale: nova.bat --cli
setlocal
cd /d "%~dp0"
python -m nova %*
