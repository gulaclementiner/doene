@echo off
setlocal
rem Launch Doene. Builds if needed, then runs the app.
rem   run.bat            debug build
rem   run.bat --release  optimised build
rem Data dir: %APPDATA%\Doene by default; override with DONE_DATA_DIR.
rem The window stays open on failure so the error can be read.

rem UTF-8 console: the project folder name contains a non-ASCII character.
chcp 65001 >nul

pushd "%~dp0" || (echo Could not enter "%~dp0" & pause & exit /b 1)

where cargo >nul 2>&1
if errorlevel 1 if exist "%USERPROFILE%\.cargo\bin\cargo.exe" set "PATH=%USERPROFILE%\.cargo\bin;%PATH%"
where cargo >nul 2>&1
if errorlevel 1 (
  echo cargo was not found on PATH.
  echo Install Rust from https://rustup.rs (choose the default MSVC toolchain;
  echo it will also ask for the Visual Studio Build Tools), then retry.
  popd & pause & exit /b 1
)

set "PROFILE_FLAG="
if /i "%~1"=="--release" set "PROFILE_FLAG=--release"

echo Starting Doene... (first build can take a few minutes)
cargo run %PROFILE_FLAG%
set "RC=%errorlevel%"

if not "%RC%"=="0" (
  echo.
  echo Doene exited with code %RC%.
  popd & pause & exit /b %RC%
)
popd
endlocal
