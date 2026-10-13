@echo off
rem Builds the game and the setup program from this source. Double-click it.
rem It needs Rust (https://rustup.rs) with its C++ build tools; nothing else.
rem Players do not need this: the bundle on the project's Releases page has
rem the programs ready-made (see HOW-TO-PLAY.txt).
cd /d "%~dp0"
where cargo >nul 2>nul
if errorlevel 1 (
    echo Rust is not installed. Get it from https://rustup.rs and run this again.
    echo ^(Or take the ready-made bundle from the project's Releases page instead.^)
    pause
    exit /b 1
)
echo Building the game and the setup program. The first time takes a good while.
cargo build -p brs-game -p brs-extract
if errorlevel 1 (
    echo.
    echo The build did not go through: what went wrong is above.
    pause
    exit /b 1
)
echo.
echo Done. Now double-click  play.cmd  (or %~dp0target\debug\brs-game.exe)
echo and drag the .iso of your copy of the game onto the window that opens:
echo the game is made in this folder, under "game", and then starts
echo (see HOW-TO-PLAY.txt).
pause
