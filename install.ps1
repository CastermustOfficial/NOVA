<#
  Installazione di NOVA.

    .\install.ps1                  installa tutto e configura l'avvio automatico
    .\install.ps1 -DaSorgente      compila il core Rust invece di scaricarlo
    .\install.ps1 -ConCuda         scarica anche llama.cpp CUDA (per il modello locale)
    .\install.ps1 -SenzaAvvioAuto  non parte da solo all'accensione
    .\install.ps1 -Disinstalla     toglie avvio automatico e collegamenti

  Non serve Rust ne' Visual Studio: i binari arrivano gia' compilati dalle
  release di GitHub. Serve Python 3.10+ per il cervello e la memoria.
#>
param(
    [switch]$DaSorgente,
    [switch]$ConCuda,
    [switch]$SenzaAvvioAuto,
    [switch]$Disinstalla
)

$ErrorActionPreference = 'Stop'
$Root    = Split-Path -Parent $MyInvocation.MyCommand.Path
$RunName = 'NOVA'
$Repo    = 'CastermustOfficial/NOVA'
$BinDir  = Join-Path $Root 'bin'

function Info($m) { Write-Host "[nova] $m" -ForegroundColor Cyan }
function Warn($m) { Write-Host "[nova] $m" -ForegroundColor Yellow }
function Ok($m)   { Write-Host "[nova] $m" -ForegroundColor Green }
function Err($m)  { Write-Host "[nova] $m" -ForegroundColor Red }

# ---------------------------------------------------------------- disinstalla
if ($Disinstalla) {
    Get-Process novad, nova-shell -ErrorAction SilentlyContinue | Stop-Process -Force
    Remove-ItemProperty 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Run' `
        -Name $RunName -ErrorAction SilentlyContinue
    Remove-Item (Join-Path ([Environment]::GetFolderPath('Desktop')) 'NOVA.lnk') `
        -Force -ErrorAction SilentlyContinue
    Ok "Avvio automatico e collegamento rimossi."
    Warn "I tuoi dati restano in $env:APPDATA\NOVA (memoria, credenziali, configurazione)."
    Warn "Se vuoi cancellare anche quelli, elimina quella cartella a mano: non lo faccio io."
    exit 0
}

Write-Host ""
Write-Host "  NOVA - installazione" -ForegroundColor White
Write-Host "  --------------------" -ForegroundColor DarkGray
Write-Host ""

# -------------------------------------------------------------------- Python
Info "Cerco Python..."
$py = (Get-Command python -ErrorAction SilentlyContinue).Source
if (-not $py) { $py = (Get-Command py -ErrorAction SilentlyContinue).Source }
if (-not $py) {
    Err "Python non trovato."
    Err "Scaricalo da https://www.python.org/downloads/ e ricordati di spuntare"
    Err "«Add python.exe to PATH» durante l'installazione."
    exit 1
}
$ver = & $py -c "import sys;print('%d.%d'%sys.version_info[:2])"
if ([version]$ver -lt [version]'3.10') {
    Err "Serve Python 3.10 o piu' recente; qui c'e' $ver."
    exit 1
}
Ok "Python $ver"

# -------------------------------------------------------------- dipendenze
Info "Installo le dipendenze Python..."
& $py -m pip install --upgrade pip --quiet 2>&1 | Out-Null
& $py -m pip install -r (Join-Path $Root 'requirements.txt') --quiet
if ($LASTEXITCODE -ne 0) {
    Warn "Qualche dipendenza opzionale non e' entrata: NOVA parte lo stesso,"
    Warn "ma certe funzioni (voce, interfaccia) potrebbero mancare."
} else { Ok "Dipendenze pronte." }

# ------------------------------------------------------------- core Rust
New-Item -ItemType Directory -Force -Path $BinDir | Out-Null
$binari = @('novad.exe', 'nova-shell.exe', 'nova.exe')

function Core-Presente {
    foreach ($b in $binari) { if (-not (Test-Path (Join-Path $BinDir $b))) { return $false } }
    return $true
}

function Scarica-Core {
    Info "Scarico il core gia' compilato dall'ultima release..."
    $rel = Invoke-RestMethod "https://api.github.com/repos/$Repo/releases/latest" `
        -Headers @{ 'User-Agent' = 'nova-installer' }
    $asset = $rel.assets | Where-Object { $_.name -eq 'nova-core-windows-x64.zip' } | Select-Object -First 1
    if (-not $asset) { throw "la release $($rel.tag_name) non contiene i binari per Windows" }

    $zip = Join-Path $env:TEMP $asset.name
    Info "  $($rel.tag_name) - $([math]::Round($asset.size/1MB,1)) MB"
    Invoke-WebRequest $asset.browser_download_url -OutFile $zip -UseBasicParsing
    Expand-Archive -Path $zip -DestinationPath $BinDir -Force
    Remove-Item $zip -Force

    # Verifica degli hash: e' pubblicato apposta, sarebbe sciocco non usarlo.
    $somme = Join-Path $BinDir 'SHA256SUMS.txt'
    if (Test-Path $somme) {
        foreach ($riga in Get-Content $somme) {
            if ($riga -match '^([0-9a-f]{64})\s+(\S+)$') {
                $atteso = $matches[1]; $file = Join-Path $BinDir $matches[2]
                if (Test-Path $file) {
                    $vero = (Get-FileHash $file -Algorithm SHA256).Hash.ToLower()
                    if ($vero -ne $atteso) { throw "$($matches[2]): l'impronta non corrisponde. Non lo installo." }
                }
            }
        }
        Ok "Impronte verificate."
    }
    if (-not (Core-Presente)) { throw "l'archivio non conteneva tutti i binari attesi" }
}

function Compila-Core {
    Info "Compilo il core da sorgente (ci vuole qualche minuto)..."
    & (Join-Path $Root 'build.ps1')
    foreach ($b in $binari) {
        $src = Join-Path $Root "core\target\release\$b"
        if (-not (Test-Path $src)) { throw "compilazione incompleta: manca $b" }
        Copy-Item $src $BinDir -Force
    }
}

if ($DaSorgente) {
    Compila-Core
} else {
    try { Scarica-Core }
    catch {
        Warn "Download non riuscito: $($_.Exception.Message)"
        Info "Provo a compilare da sorgente."
        try { Compila-Core }
        catch {
            Err "Non riesco ne' a scaricare ne' a compilare il core."
            Err "Per compilare servono Rust (https://rustup.rs) e i Build Tools di"
            Err "Visual Studio con «Desktop development with C++»."
            exit 1
        }
    }
}
Ok "Core installato in $BinDir"

# ------------------------------------------------------------ runtime CUDA
if ($ConCuda) {
    $RuntimeDir = Join-Path $Root 'runtime'
    Info "Scarico llama.cpp con CUDA..."
    New-Item -ItemType Directory -Force -Path $RuntimeDir | Out-Null
    try {
        $rel = Invoke-RestMethod 'https://api.github.com/repos/ggml-org/llama.cpp/releases/latest' `
            -Headers @{ 'User-Agent' = 'nova-installer' }
        foreach ($rx in '^llama-.*win-cuda.*x64\.zip$', '^cudart-.*win.*x64\.zip$') {
            $a = $rel.assets | Where-Object { $_.name -match $rx } | Select-Object -First 1
            if (-not $a) { continue }
            $zip = Join-Path $env:TEMP $a.name
            Info "  $($a.name) ($([math]::Round($a.size/1MB,1)) MB)"
            Invoke-WebRequest $a.browser_download_url -OutFile $zip -UseBasicParsing
            Expand-Archive -Path $zip -DestinationPath $RuntimeDir -Force
            Remove-Item $zip -Force
        }
        Get-ChildItem $RuntimeDir -Directory | ForEach-Object {
            Get-ChildItem $_.FullName -File | Move-Item -Destination $RuntimeDir -Force -ErrorAction SilentlyContinue
        }
        if (Test-Path (Join-Path $RuntimeDir 'llama-server.exe')) { Ok "Runtime CUDA pronto." }
        else { Warn "Non ho trovato llama-server nell'archivio: usero' i backend gia' presenti." }
    } catch { Warn "Runtime CUDA non scaricato ($($_.Exception.Message)). NOVA usa comunque gli altri cervelli." }
}

# ------------------------------------------------------------ configurazione
Info "Rilevo modello e runtime..."
Push-Location $Root
try {
    & $py -c "from nova.config import Config;from nova.setup_wizard import autoconfigure;c=Config.load();[print('  ',n) for n in autoconfigure(c,force=True)];c.save()"
} finally { Pop-Location }

# --------------------------------------------------------------- avvio auto
$shell = Join-Path $BinDir 'nova-shell.exe'
if (-not $SenzaAvvioAuto) {
    Set-ItemProperty 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Run' -Name $RunName -Value "`"$shell`""
    Ok "NOVA si avviera' da sola all'accensione."
}

$ws  = New-Object -ComObject WScript.Shell
$lnk = $ws.CreateShortcut((Join-Path ([Environment]::GetFolderPath('Desktop')) 'NOVA.lnk'))
$lnk.TargetPath       = $shell
$lnk.WorkingDirectory = $Root
$lnk.Description      = 'NOVA - assistente digitale locale'
$lnk.Save()
Ok "Collegamento creato sul Desktop."

Write-Host ""
Ok "Installazione completata."
Write-Host ""
Write-Host "  Avvia NOVA dal collegamento sul Desktop." -ForegroundColor Gray
Write-Host "  L'orb comparira' in un angolo: cliccalo per scrivere, o chiamala per nome." -ForegroundColor Gray
Write-Host ""
Write-Host "  NOVA parte con «conferma sempre»: ti chiede il permesso prima di ogni" -ForegroundColor Gray
Write-Host "  azione che tocca il sistema. Puoi allentarlo dalle impostazioni quando" -ForegroundColor Gray
Write-Host "  ti fidi." -ForegroundColor Gray
Write-Host ""