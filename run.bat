@echo off
rem Launch Doene. Builds if needed, then runs the app.
rem   run.bat            debug build
rem   run.bat --release  optimised build
rem Data dir: %APPDATA%\Doene by default; override with DONE_DATA_DIR.
cd /d "%~dp0"
if /i "%1"=="--release" (
  cargo run --release
) else (
  cargo run
)
