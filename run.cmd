@echo off
rem Chay run.ps1 ma khong can doi ExecutionPolicy cua he thong.
rem Dung: run.cmd          -> cong 8787
rem       run.cmd 8788     -> cong 8788
if not "%~1"=="" set "PORT=%~1"
powershell -NoProfile -ExecutionPolicy Bypass -File "%~dp0run.ps1"
