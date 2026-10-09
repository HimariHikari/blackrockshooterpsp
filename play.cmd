@echo off
rem Starts the port. Double-click this file, or run it with the game's options
rem after it (e.g.  play.cmd --battle STG01_02 --group 3 --level 30).
rem It starts the copy kept for playing (target\debug\brs-play.exe) if there is
rem one, so that the game can be rebuilt while it is open; else the build itself.
cd /d "%~dp0"
if exist target\debug\brs-play.exe (
    start "" target\debug\brs-play.exe %*
) else if exist target\debug\brs-game.exe (
    start "" target\debug\brs-game.exe %*
) else (
    echo The game has not been built yet: run  cargo build -p brs-game  first.
    pause
)
