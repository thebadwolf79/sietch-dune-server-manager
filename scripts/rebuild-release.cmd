@echo off
setlocal EnableExtensions

set "SCRIPT_DIR=%~dp0"
for %%I in ("%SCRIPT_DIR%..") do set "REPO_ROOT=%%~fI"
set "APP_DIR=%REPO_ROOT%\app"
set "SERVICE_BIN=%REPO_ROOT%\target\x86_64-unknown-linux-musl\release\dune-server-service"
set "BUNDLED_SERVICE_BIN=%APP_DIR%\src-tauri\binaries\dune-server-service"
set "NSIS_DIR=%REPO_ROOT%\target\release\bundle\nsis"
set "RUSTUP_RUSTC=%USERPROFILE%\.rustup\toolchains\stable-x86_64-pc-windows-msvc\bin\rustc.exe"

pushd "%REPO_ROOT%" || exit /b 1

echo.
echo == Cargo workspace tests ==
cargo test --workspace
if errorlevel 1 goto :fail

echo.
echo == Frontend build ==
pushd "%APP_DIR%" || goto :fail
call npm.cmd run build
if errorlevel 1 goto :fail_pop_app
popd

echo.
echo == Linux management service build ==
if exist "%RUSTUP_RUSTC%" (
  set "RUSTC=%RUSTUP_RUSTC%"
)
rustup run stable cargo zigbuild -p dune-server-service --release --target x86_64-unknown-linux-musl
if errorlevel 1 goto :fail

echo.
echo == Bundle service binary ==
copy /Y "%SERVICE_BIN%" "%BUNDLED_SERVICE_BIN%" >nul
if errorlevel 1 goto :fail

echo.
echo == Tauri release installer ==
pushd "%APP_DIR%" || goto :fail
call npm.cmd run tauri -- build
if errorlevel 1 goto :fail_pop_app
popd

echo.
echo Release build complete:
for /f "delims=" %%F in ('dir /b /o-d "%NSIS_DIR%\*.exe" 2^>nul') do (
  echo   %NSIS_DIR%\%%F
  goto :done
)
echo   %NSIS_DIR%

:done
popd
exit /b 0

:fail_pop_app
popd

:fail
echo.
echo Rebuild failed.
popd
exit /b 1
