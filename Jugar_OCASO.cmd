@echo off
cd /d "%~dp0"
if exist "ocaso.exe" (
    "ocaso.exe" %*
) else (
    cargo run --release -- %*
)
if errorlevel 1 pause
