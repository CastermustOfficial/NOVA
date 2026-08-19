# Scarica la build CUDA di llama.cpp dentro NOVA\runtime (indipendente da LM Studio).
$ErrorActionPreference = 'Stop'
$rt = Join-Path (Split-Path -Parent $MyInvocation.MyCommand.Path) 'runtime'
$log = Join-Path $env:TEMP 'nova_cuda.log'
New-Item -ItemType Directory -Force -Path $rt | Out-Null
function L($m) { Add-Content -Path $log -Value "$(Get-Date -Format HH:mm:ss) $m" }
try {
    $rel = Invoke-RestMethod 'https://api.github.com/repos/ggml-org/llama.cpp/releases/latest' -Headers @{'User-Agent' = 'nova' }
    L "release $($rel.tag_name)"
    foreach ($pat in @('^llama-.*win-cuda-12\.4-x64\.zip$', '^cudart-llama-bin-win-cuda-12\.4-x64\.zip$')) {
        $a = $rel.assets | Where-Object { $_.name -match $pat } | Select-Object -First 1
        if (-not $a) { L "MANCA $pat"; continue }
        $z = Join-Path $env:TEMP $a.name
        L "scarico $($a.name) ($([math]::Round($a.size/1MB,1)) MB)"
        Invoke-WebRequest $a.browser_download_url -OutFile $z -UseBasicParsing
        Expand-Archive $z -DestinationPath $rt -Force
        Remove-Item $z -Force
        L "estratto $($a.name)"
    }
    Get-ChildItem $rt -Directory | ForEach-Object {
        Get-ChildItem $_.FullName -File | Move-Item -Destination $rt -Force -ErrorAction SilentlyContinue
    }
    if (Test-Path (Join-Path $rt 'llama-server.exe')) { L 'FATTO: llama-server.exe pronto' }
    else { L 'ATTENZIONE: llama-server.exe non trovato' }
}
catch { L "ERRORE: $($_.Exception.Message)" }
