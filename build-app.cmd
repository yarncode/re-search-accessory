@echo off
rem Build the desktop app without changing the system ExecutionPolicy.
powershell -NoProfile -ExecutionPolicy Bypass -File "%~dp0build-app.ps1"
