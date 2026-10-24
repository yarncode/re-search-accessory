@echo off
rem Chay run-dev.ps1 ma khong can doi ExecutionPolicy cua he thong.
rem Dung: run-dev.cmd          -> cong 8787
rem       run-dev.cmd 8788     -> cong 8788
if not "%~1"=="" set "PORT=%~1"
powershell -NoProfile -ExecutionPolicy Bypass -File "%~dp0run-dev.ps1"
