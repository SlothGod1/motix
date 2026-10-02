@echo off
rem Sets this PC up to build and publish MOTIX updates (ADR-036). Safe to run again.
powershell.exe -NoProfile -ExecutionPolicy Bypass -File "%~dp0setup.ps1"
pause
