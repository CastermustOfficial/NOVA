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
    # ATTENZIONE: «${env:ProgramFiles(x86)}» si espande a stringa vuota — le
    # parentesi di «(x86)» rompono quella sintassi e il percorso diventa
    # «\Microsoft Visual Studio». Serve la forma esplicita.
    $pf   = [Environment]::GetEnvironmentVariable('ProgramFiles')
    $pf86 = [Environment]::GetEnvironmentVariable('ProgramFiles(x86)')
    # In certi contesti (servizi, shell ristrette) queste variabili non ci
    # sono affatto. Un Join-Path su null interrompe tutto con un errore che
    # non c'entra niente con la compilazione: meglio un ripiego ragionevole.
    if (-not $pf)   { $pf   = 'C:\Program Files' }
    if (-not $pf86) { $pf86 = "$pf (x86)" }

    $vc = $null
    # vswhere e' la via ufficiale, ma le installazioni dei soli Build Tools
    # spesso non ce l'hanno: in quel caso si cerca il file a mano.
    $vswhere = Join-Path $pf86 'Microsoft Visual Studio\Installer\vswhere.exe'
    if (Test-Path $vswhere) {
        $vsPath = & $vswhere -latest -products * -property installationPath 2>$null | Select-Object -First 1
        if ($vsPath) {
            $c = Join-Path $vsPath 'VC\Auxiliary\Build\vcvars64.bat'
            if (Test-Path $c) { $vc = $c }
        }
    }
    if (-not $vc) {
        # Si cerca direttamente vcvars64.bat, non «la prima cartella»: sotto
        # «Microsoft Visual Studio» ci sono anche Installer e Shared, che non
        # contengono nulla di utile.
        foreach ($radice in @($pf86, $pf)) {
            if (-not $radice) { continue }
            $base = Join-Path $radice 'Microsoft Visual Studio'
            if (-not (Test-Path $base)) { continue }
            $trovato = Get-ChildItem $base -Recurse -Filter vcvars64.bat -ErrorAction SilentlyContinue |
                       Sort-Object FullName -Descending | Select-Object -First 1
            if ($trovato) { $vc = $trovato.FullName; break }
        }
    }

    if ($vc) {
        Write-Host "[nova] carico l'ambiente MSVC" -ForegroundColor Cyan
        cmd /c "`"$vc`" >nul 2>&1 && set" | ForEach-Object {
            if ($_ -match '^([^=]+)=(.*)$') {
                Set-Item -Path "env:$($matches[1])" -Value $matches[2] -ErrorAction SilentlyContinue
            }
        }
    } else {
        Write-Host "[nova] non trovo Visual Studio: se la compilazione fallisce," -ForegroundColor Yellow
        Write-Host "[nova] installa i Build Tools con «Desktop development with C++»." -ForegroundColor Yellow
    }
}

Push-Location $Core
try {
    # Chiamate esplicite invece dello splatting: con @array PowerShell puo'
    # far arrivare a cargo un «-» isolato, e l'errore che ne esce («unexpected
    # argument») non somiglia per niente alla causa.
    $azione = if ($Test) { 'test' } else { 'build' }
    Write-Host "[nova] cargo $azione$(if (-not $Debug) { ' --release' })" -ForegroundColor Cyan
    # cargo racconta l'avanzamento su stderr, non solo gli errori. Con
    # ErrorActionPreference a Stop — che e' quello che usa install.ps1 —
    # PowerShell scambia la prima riga di avanzamento per un errore fatale e
    # interrompe una compilazione perfettamente sana. Per un comando esterno
    # l'unico giudice e' il codice di uscita.
    $primaEAP = $ErrorActionPreference
    $ErrorActionPreference = 'Continue'
    try {
        if ($Debug) { & $cargo $azione } else { & $cargo $azione '--release' }
        $codice = $LASTEXITCODE
    } finally { $ErrorActionPreference = $primaEAP }
    if ($codice -ne 0) { throw "compilazione fallita (codice $codice)" }
    Write-Host "[nova] fatto." -ForegroundColor Green
} finally { Pop-Location }