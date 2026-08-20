@echo off
REM Ambiente di build per nova-core.
REM Rust e' installato ma non nel PATH, e il linker MSVC va attivato con vcvars.
REM Uso:  x build --release  |  x run --bin novad  |  x test
setlocal
if exist "C:\Program Files (x86)\Microsoft Visual Studio\2019\BuildTools\VC\Auxiliary\Build\vcvars64.bat" (
    call "C:\Program Files (x86)\Microsoft Visual Studio\2019\BuildTools\VC\Auxiliary\Build\vcvars64.bat" >nul 2>&1
) else (
    for /f "usebackq tokens=*" %%i in (`"%ProgramFiles(x86)%\Microsoft Visual Studio\Installer\vswhere.exe" -latest -products * -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath 2^>nul`) do (
        if exist "%%i\VC\Auxiliary\Build\vcvars64.bat" call "%%i\VC\Auxiliary\Build\vcvars64.bat" >nul 2>&1
    )
)
set "PATH=%USERPROFILE%\.rustup\toolchains\stable-x86_64-pc-windows-msvc\bin;%PATH%"
cd /d "%~dp0"
cargo %*
