<#
  Installazione di NOVA.
    .\install.ps1                    dipendenze + configurazione + avvio automatico
    .\install.ps1 -WithCudaRuntime   scarica anche llama.cpp CUDA (consigliato su NVIDIA)
    .\install.ps1 -NoAutostart       salta l'avvio automatico all'accensione
    .\install.ps1 -Uninstall         rimuove solo l'avvio automatico
#>
param(
    [switch]$WithCudaRuntime,
    [switch]$NoAutostart,
    [switch]$Uninstall
)

$ErrorActionPreference = 'Stop'
$Root = Split-Path -Parent $MyInvocation.MyCommand.Path
$RunName = 'NOVA'

function Info($m) { Write-Host "[nova] $m" -ForegroundColor Cyan }
function Warn($m) { Write-Host "[nova] $m" -ForegroundColor Yellow }
function Ok($m)   { Write-Host "[nova] $m" -ForegroundColor Green }

if ($Uninstall) {
    Remove-ItemProperty -Path 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Run' `
        -Name $RunName -ErrorAction SilentlyContinue
    Ok "Avvio automatico rimosso."
    exit 0
}

Info "Verifico Python..."
$py = (Get-Command python -ErrorAction SilentlyContinue).Source
if (-not $py) { throw "Python non trovato nel PATH. Installalo da python.org." }
$ver = & python -c "import sys;print('.'.join(map(str,sys.version_info[:2])))"
Ok "Python $ver -> $py"

Info "Installo le dipendenze Python..."
& python -m pip install --upgrade pip --quiet
& python -m pip install -r (Join-Path $Root 'requirements.txt') --quiet
if ($LASTEXITCODE -ne 0) { Warn "Alcune dipendenze opzionali non sono state installate." }
Ok "Dipendenze pronte."

$RuntimeDir = Join-Path $Root 'runtime'
if ($WithCudaRuntime) {
    Info "Scarico l'ultima build CUDA di llama.cpp..."
    New-Item -ItemType Directory -Force -Path $RuntimeDir | Out-Null
    try {
        $rel = Invoke-RestMethod 'https://api.github.com/repos/ggml-org/llama.cpp/releases/latest' `
            -Headers @{ 'User-Agent' = 'nova-installer' }
        $server = $rel.assets | Where-Object { $_.name -match '^llama-.*win-cuda.*x64\.zip$' } | Select-Object -First 1
        $cudart = $rel.assets | Where-Object { $_.name -match '^cudart-.*win.*x64\.zip$' } | Select-Object -First 1
        foreach ($a in @($server, $cudart)) {
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
        if (Test-Path (Join-Path $RuntimeDir 'llama-server.exe')) {
            Ok "Runtime CUDA installato in $RuntimeDir"
        } else {
            Warn "Runtime CUDA non trovato nell'archivio: uso i backend gia' presenti sul sistema."
        }
    } catch {
        Warn "Download del runtime CUDA fallito ($($_.Exception.Message)). Uso i backend gia' presenti."
    }
}

Info "Rilevo modello e runtime..."
Push-Location $Root
& python -c "from nova.config import Config;from nova.setup_wizard import autoconfigure;c=Config.load();[print('[setup]',n) for n in autoconfigure(c,force=True)];c.save()"
Pop-Location

if (-not $NoAutostart) {
    $pyw = $py -replace 'python\.exe$', 'pythonw.exe'
    if (-not (Test-Path $pyw)) { $pyw = $py }
    $target = "`"$pyw`" `"$(Join-Path $Root 'run_nova.pyw')`""
    Set-ItemProperty -Path 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Run' `
        -Name $RunName -Value $target
    Ok "Avvio automatico configurato."
}

$ws = New-Object -ComObject WScript.Shell
$lnk = $ws.CreateShortcut((Join-Path ([Environment]::GetFolderPath('Desktop')) 'NOVA.lnk'))
$lnk.TargetPath = ($py -replace 'python\.exe$', 'pythonw.exe')
$lnk.Arguments = "`"$(Join-Path $Root 'run_nova.pyw')`""
$lnk.WorkingDirectory = $Root
$lnk.Description = 'NOVA - assistente digitale locale'
$lnk.Save()
Ok "Collegamento creato sul Desktop."

Ok "Installazione completata. Avvia NOVA dal Desktop oppure con: python -m nova"
