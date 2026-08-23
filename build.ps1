<#
  Compila il core Rust di NOVA.
    .\build.ps1              build di release
    .\build.ps1 -Debug       build di sviluppo
    .\build.ps1 -Test        esegue i test

  Trova da solo la toolchain: rustup nel PATH, oppure quella installata
  nella home. Su Windows serve anche il linker MSVC (Visual Studio Build
  Tools con «Desktop development with C++»).
#>
param([switch]$Debug, [switch]$Test)
$ErrorActionPreference = 'Stop'
$Core = Join-Path (Split-Path -Parent $MyInvocation.MyCommand.Path) 'core'

function Trova-Cargo {
    $c = Get-Command cargo -ErrorAction SilentlyContinue
    if ($c) { return $c.Source }
    # rustup installa i binari qui anche quando il PATH non e' stato aggiornato
    $t = Join-Path $env:USERPROFILE '.rustup\toolchains\stable-x86_64-pc-windows-msvc\bin\cargo.exe'
    if (Test-Path $t) { return $t }
    throw "cargo non trovato. Installa Rust da https://rustup.rs"
}

$cargo = Trova-Cargo
$bin = Split-Path -Parent $cargo
if ($env:PATH -notlike "*$bin*") { $env:PATH = "$bin;$env:PATH" }

# Il linker MSVC non e' nel PATH finche' non si entra nell'ambiente di
# Visual Studio: se manca cl.exe lo si cerca e si carica vcvars64.
if (-not (Get-Command cl.exe -ErrorAction SilentlyContinue)) {
    $vswhere = "${env:ProgramFiles(x86)}\Microsoft Visual Studio\Installer\vswhere.exe"
    $vs = if (Test-Path $vswhere) {
        & $vswhere -latest -products * -property installationPath 2>$null
    } else {
        Get-ChildItem "${env:ProgramFiles(x86)}\Microsoft Visual Studio","$env:ProgramFiles\Microsoft Visual Studio" `
            -Directory -ErrorAction SilentlyContinue | Select-Object -First 1 -ExpandProperty FullName
    }
    if ($vs) {
        $vc = Get-ChildItem $vs -Recurse -Filter vcvars64.bat -ErrorAction SilentlyContinue |
              Select-Object -First 1 -ExpandProperty FullName
        if ($vc) {
            Write-Host "[nova] carico l'ambiente MSVC" -ForegroundColor Cyan
            cmd /c "`"$vc`" >nul 2>&1 && set" | ForEach-Object {
                if ($_ -match '^([^=]+)=(.*)$') { Set-Item -Path "env:$($matches[1])" -Value $matches[2] -ErrorAction SilentlyContinue }
            }
        }
    }
}

Push-Location $Core
try {
    $azione = if ($Test) { 'test' } else { 'build' }
    $profilo = if ($Debug) { @() } else { @('--release') }
    Write-Host "[nova] cargo $azione $profilo" -ForegroundColor Cyan
    & $cargo $azione @profilo
    if ($LASTEXITCODE -ne 0) { throw "compilazione fallita (codice $LASTEXITCODE)" }
    Write-Host "[nova] fatto." -ForegroundColor Green
} finally { Pop-Location }