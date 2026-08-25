@echo off
setlocal EnableExtensions

set "ROOT_DIR=%~dp0.."
for %%I in ("%ROOT_DIR%") do set "ROOT_DIR=%%~fI"
set "APP_DIR=%ROOT_DIR%\apps\desktop"
set "SOURCE_EXIFTOOL=%ROOT_DIR%\shuttertrail-lightroom-gpx-sync.lrplugin\bin\windows"
set "STAGE_DIR=%APP_DIR%\src-tauri\resources\exiftool\windows"
set "BUNDLE_ROOT=%APP_DIR%\src-tauri\target\release\bundle\nsis"
set "DIST_DIR=%ROOT_DIR%\dist"

where cargo >nul 2>nul || (
  echo ERROR: Rust and Cargo are required.
  exit /b 1
)
where rustc >nul 2>nul || (
  echo ERROR: The Rust compiler is required.
  exit /b 1
)
where node >nul 2>nul || (
  echo ERROR: Node.js is required.
  exit /b 1
)
where npm >nul 2>nul || (
  echo ERROR: npm is required.
  exit /b 1
)

if not exist "%SOURCE_EXIFTOOL%\exiftool.exe" (
  echo ERROR: Bundled ExifTool was not found at:
  echo   %SOURCE_EXIFTOOL%\exiftool.exe
  exit /b 1
)
if not exist "%SOURCE_EXIFTOOL%\exiftool_files\exiftool.pl" (
  echo ERROR: The bundled ExifTool support files were not found at:
  echo   %SOURCE_EXIFTOOL%\exiftool_files
  exit /b 1
)
"%SOURCE_EXIFTOOL%\exiftool.exe" -ver >nul 2>nul || (
  echo ERROR: The bundled ExifTool runtime could not be started.
  exit /b 1
)

echo Staging ExifTool...
if exist "%STAGE_DIR%" rmdir /s /q "%STAGE_DIR%"
mkdir "%STAGE_DIR%" || exit /b 1
xcopy "%SOURCE_EXIFTOOL%\*" "%STAGE_DIR%\" /e /i /q /y >nul || exit /b 1

pushd "%APP_DIR%" || exit /b 1
echo Installing locked frontend dependencies...
call npm ci || goto :failed

echo Running frontend and Rust tests...
call npm run check || goto :failed
cargo test --manifest-path src-tauri\Cargo.toml || goto :failed

echo Building Windows NSIS installer...
if exist "%BUNDLE_ROOT%" rmdir /s /q "%BUNDLE_ROOT%" || goto :failed
call npm run tauri -- build --bundles nsis || goto :failed

echo Collecting Windows package...
if not exist "%DIST_DIR%" mkdir "%DIST_DIR%" || goto :failed
set "PACKAGE_FOUND="
for %%F in ("%BUNDLE_ROOT%\*.exe") do (
  if exist "%%~fF" (
    copy /y "%%~fF" "%DIST_DIR%\" >nul || goto :failed
    set "PACKAGE_FOUND=1"
  )
)
if not defined PACKAGE_FOUND (
  echo ERROR: No NSIS installer was produced under:
  echo   %BUNDLE_ROOT%
  goto :failed
)

echo.
echo Windows package copied to:
echo   %DIST_DIR%
popd
exit /b 0

:failed
set "BUILD_EXIT=%ERRORLEVEL%"
if "%BUILD_EXIT%"=="0" set "BUILD_EXIT=1"
popd
echo ERROR: Windows package build failed.
exit /b %BUILD_EXIT%
