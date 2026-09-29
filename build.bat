@echo off
setlocal

set IMAGE_NAME=plasmer-builder
set CONTAINER_NAME=plasmer-out
set BIN_NAME=Plasmer.exe
set ICON_PATH=assets\icon.ico
set RCEDIT=rcedit.exe
set OUT_DIR=target\docker

echo [1/7] Downloading rcedit...
if not exist "%RCEDIT%" (
    curl -L -o %RCEDIT% https://github.com/electron/rcedit/releases/download/v2.0.0/rcedit-x64.exe
    if errorlevel 1 (
        echo ERROR: Failed to download rcedit
        exit /b 1
    )
)

echo [2/7] Creating output directory...
if not exist "%OUT_DIR%" mkdir "%OUT_DIR%"

echo [3/7] Building Docker image...
docker build -t %IMAGE_NAME% .
if errorlevel 1 (
    echo ERROR: Docker build failed
    exit /b 1
)

echo [4/7] Creating container...
docker rm -f %CONTAINER_NAME% >nul 2>&1
docker create --name %CONTAINER_NAME% %IMAGE_NAME% >nul
if errorlevel 1 (
    echo ERROR: Failed to create container
    exit /b 1
)

echo [5/7] Extracting %BIN_NAME%...
docker cp %CONTAINER_NAME%:/out/%BIN_NAME% %OUT_DIR%\%BIN_NAME%
if errorlevel 1 (
    echo ERROR: Failed to extract binary
    docker rm -f %CONTAINER_NAME% >nul 2>&1
    exit /b 1
)

docker rm -f %CONTAINER_NAME% >nul 2>&1

echo [6/7] Embedding icon...
.\%RCEDIT% %OUT_DIR%\%BIN_NAME% --set-icon %ICON_PATH%
if errorlevel 1 (
    echo ERROR: Failed to embed icon
    exit /b 1
)

echo [7/7] Done!
echo.
echo Output: %OUT_DIR%\%BIN_NAME%
for %%A in (%OUT_DIR%\%BIN_NAME%) do echo Size: %%~zA bytes
echo.

endlocal