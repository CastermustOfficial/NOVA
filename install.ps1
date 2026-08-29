<#
  Installazione di NOVA.

    .\install.ps1                  installazione guidata
    .\install.ps1 -Silenzioso      senza domande: solo il minimo che funziona
    .\install.ps1 -DaSorgente      compila il core invece di scaricarlo
    .\install.ps1 -SenzaAvvioAuto  non parte all'accensione
    .\install.ps1 -Disinstalla     toglie avvio automatico e collegamento

  Non serve ne' Rust ne' Visual Studio: il core arriva gia' compilato.
  Serve Python 3.10+.
#>
param(
    [switch]$Silenzioso,
    [switch]$DaSorgente,
    [switch]$SenzaAvvioAuto,
    [switch]$Disinstalla,
    [switch]$Prova
)

$ErrorActionPreference = 'Stop'

# Windows PowerShell 5.1 — quello che ha ogni Windows — non carica da solo
# System.Net.Http: senza questa riga «New-Object System.Net.Http.HttpClient»
# fallisce e con lui OGNI scaricamento dell'installer. Su PowerShell 7 il tipo
# c'e' gia' e Add-Type non fa danno.
try { Add-Type -AssemblyName System.Net.Http -ErrorAction SilentlyContinue } catch { }
$Root    = Split-Path -Parent $MyInvocation.MyCommand.Path
$RunName = 'NOVA'
$Repo    = 'CastermustOfficial/NOVA'
$BinDir  = Join-Path $Root 'bin'
$Runtime = Join-Path $Root 'runtime'

function Info($m) { Write-Host "[nova] $m" -ForegroundColor Cyan }
function Warn($m) { Write-Host "[nova] $m" -ForegroundColor Yellow }
function Ok($m)   { Write-Host "[nova] $m" -ForegroundColor Green }
function Err($m)  { Write-Host "[nova] $m" -ForegroundColor Red }
function Titolo($m) {
    Write-Host ""
    Write-Host "  $m" -ForegroundColor White
    Write-Host "  $('-' * $m.Length)" -ForegroundColor DarkGray
}

# Una domanda a scelta multipla. In modalita' silenziosa prende il default.
# Anche una domanda a risposta libera va difesa: Read-Host su uno standard
# input rediretto non torna mai, e chi guarda vede solo un installer piantato.
# In piu' chi incolla un percorso da Esplora risorse si porta dietro le
# virgolette: toglierle qui evita un "file non trovato" che non e' colpa sua.
function Chiedi-Testo($domanda, $predefinito = '') {
    if ($Silenzioso) { return $predefinito }
    if ([Console]::IsInputRedirected) {
        Warn "Non posso chiedere: $domanda (input non interattivo)."
        return $predefinito
    }
    $r = Read-Host "  $domanda"
    if ([string]::IsNullOrWhiteSpace($r)) { return $predefinito }
    return $r.Trim().Trim('"').Trim("'")
}

function Chiedi($domanda, $opzioni, $predefinita = 1) {
    if ($Silenzioso) { return $predefinita }
    # Read-Host non legge da una pipe: se lo standard input e' rediretto — uno
    # script che lancia l'installer, un'automazione, una sessione remota — la
    # domanda resterebbe li' per sempre, e chi guarda vede solo un programma
    # piantato senza spiegazione. Meglio proseguire con la scelta di base e
    # dirlo, che appendersi.
    if ([Console]::IsInputRedirected) {
        Warn "Non posso fare domande (input non interattivo): vado con la scelta di base."
        return $predefinita
    }
    Write-Host ""
    Write-Host "  $domanda" -ForegroundColor White
    for ($i = 0; $i -lt $opzioni.Count; $i++) {
        $n = $i + 1
        $segno = if ($n -eq $predefinita) { '*' } else { ' ' }
        Write-Host "   $segno $n) $($opzioni[$i])" -ForegroundColor Gray
    }
    while ($true) {
        $r = Read-Host "  Scelta [$predefinita]"
        if ([string]::IsNullOrWhiteSpace($r)) { return $predefinita }
        $n = 0
        if ([int]::TryParse($r, [ref]$n) -and $n -ge 1 -and $n -le $opzioni.Count) { return $n }
        Warn "Rispondi con un numero da 1 a $($opzioni.Count)."
    }
}

# Scarica mostrando l'avanzamento. Invoke-WebRequest su PowerShell 5 tiene
# tutto in memoria: per un file da 16 GB non e' un'opzione.
function Scarica($url, $destinazione, $etichetta) {
    if (Test-Path $destinazione) {
        $mb = [math]::Round((Get-Item $destinazione).Length / 1MB)
        Info "$etichetta gia' presente ($mb MB)"
        return
    }
    if ($Prova) {
        Info "[prova] scaricherei $etichetta da $url"
        Info "[prova]   in $destinazione"
        return
    }
    $parziale = "$destinazione.parziale"
    Info "$etichetta - scarico..."
    $cliente = New-Object System.Net.Http.HttpClient
    $cliente.Timeout = [TimeSpan]::FromHours(6)
    try {
        $risposta = $cliente.GetAsync($url, [System.Net.Http.HttpCompletionOption]::ResponseHeadersRead).Result
        if (-not $risposta.IsSuccessStatusCode) { throw "il server ha risposto $($risposta.StatusCode)" }
        $totale = $risposta.Content.Headers.ContentLength
        $sorgente = $risposta.Content.ReadAsStreamAsync().Result
        $uscita = [System.IO.File]::Create($parziale)
        try {
            $buffer = New-Object byte[] 1048576
            $fatti = 0L
            $ultimo = -1
            while (($letti = $sorgente.Read($buffer, 0, $buffer.Length)) -gt 0) {
                $uscita.Write($buffer, 0, $letti)
                $fatti += $letti
                if ($totale) {
                    $pc = [int](100 * $fatti / $totale)
                    if ($pc -ne $ultimo) {
                        $ultimo = $pc
                        Write-Progress -Activity $etichetta -Status "$pc% di $([math]::Round($totale/1GB,1)) GB" -PercentComplete $pc
                    }
                }
            }
        } finally { $uscita.Close(); $sorgente.Close() }
        Write-Progress -Activity $etichetta -Completed
        Move-Item $parziale $destinazione -Force
        $mb = [math]::Round((Get-Item $destinazione).Length / 1MB)
        Ok "$etichetta - fatto ($mb MB)"
    } catch {
        Remove-Item $parziale -Force -ErrorAction SilentlyContinue
        throw
    } finally { $cliente.Dispose() }
}

# --------------------------------------------------------------- disinstalla
if ($Disinstalla) {
    if ($Prova) { Info "[prova] toglierei avvio automatico e collegamento"; exit 0 }
    Get-Process novad, nova-shell -ErrorAction SilentlyContinue | Stop-Process -Force
    Remove-ItemProperty 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Run' -Name $RunName -ErrorAction SilentlyContinue
    Remove-Item (Join-Path ([Environment]::GetFolderPath('Desktop')) 'NOVA.lnk') -Force -ErrorAction SilentlyContinue
    Ok "Avvio automatico e collegamento rimossi."
    Warn "I tuoi dati restano in $env:APPDATA\NOVA (memoria, credenziali, configurazione)."
    Warn "Se vuoi cancellare anche quelli, elimina quella cartella a mano: non lo faccio io."
    exit 0
}

Write-Host ""
Write-Host "   NOVA" -ForegroundColor White
Write-Host "   un esperto seduto accanto a te, dentro il tuo PC" -ForegroundColor DarkGray
Write-Host ""

# ------------------------------------------------------------- prerequisiti
Titolo "Controllo i prerequisiti"

if ([Environment]::OSVersion.Platform -ne 'Win32NT') {
    Err "NOVA per ora gira solo su Windows: l'automazione usa UI Automation e"
    Err "l'archivio credenziali usa DPAPI, che altrove non esistono."
    exit 1
}
if (-not [Environment]::Is64BitOperatingSystem) {
    Err "Serve un Windows a 64 bit."
    exit 1
}
$arch = $env:PROCESSOR_ARCHITECTURE
if ($arch -eq 'ARM64') {
    Err "Windows ARM64 non e' ancora supportato: i binari sono compilati per x64."
    Err "Puoi provare con -DaSorgente, ma non e' collaudato."
    if (-not $DaSorgente) { exit 1 }
}
Ok "Windows x64"

$libero = (Get-PSDrive -Name (Split-Path $Root -Qualifier).TrimEnd(':')).Free / 1GB
Info ("Spazio libero: {0:N1} GB" -f $libero)
if ($libero -lt 3) {
    Err "Meno di 3 GB liberi: non basta nemmeno per il minimo indispensabile."
    exit 1
}

# WebView2: l'interfaccia e' Tauri e senza questo non si apre nessuna finestra.
# Su Windows 11 c'e' sempre; su parecchi Windows 10 no.
$wv = @(
    'HKLM:\SOFTWARE\WOW6432Node\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}',
    'HKLM:\SOFTWARE\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}',
    'HKCU:\SOFTWARE\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}'
) | Where-Object { Test-Path $_ } | Select-Object -First 1
if ($wv) {
    Ok "WebView2 presente"
} else {
    Warn "WebView2 non c'e': senza, l'interfaccia di NOVA non si apre."
    $s = Chiedi "Lo installo adesso? (e' un componente ufficiale Microsoft)" @('Si, installalo', 'No, faccio da me') 1
    if ($s -eq 1) {
        try {
            $boot = Join-Path $env:TEMP 'MicrosoftEdgeWebview2Setup.exe'
            Scarica 'https://go.microsoft.com/fwlink/p/?LinkId=2124703' $boot 'WebView2'
            if ($Prova) { Info "[prova] installerei WebView2" }
            else {
                Start-Process $boot -ArgumentList '/silent', '/install' -Wait
                Ok "WebView2 installato."
            }
        } catch { Warn "Installazione di WebView2 fallita: $($_.Exception.Message)" }
    }
}
# ------------------------------------------------------------------ Python
Titolo "Python e dipendenze"

$py = $null
foreach ($c in 'python', 'py') {
    $trovato = (Get-Command $c -ErrorAction SilentlyContinue)
    if ($trovato) {
        $v = & $trovato.Source -c "import sys;print('%d.%d'%sys.version_info[:2])" 2>$null
        if ($v -and [version]$v -ge [version]'3.10') { $py = $trovato.Source; break }
    }
}
if (-not $py) {
    Err "Serve Python 3.10 o piu' recente."
    Err "Scaricalo da https://www.python.org/downloads/ e spunta «Add python.exe to PATH»."
    exit 1
}
Ok "Python $(& $py -c "import sys;print('%d.%d.%d'%sys.version_info[:3])")"

# Si installano solo le dipendenze che mancano davvero: reinstallare tutto a
# ogni esecuzione fa perdere minuti e non serve a niente.
Info "Controllo le dipendenze..."
$mancanti = & $py -c @"
import importlib.util, sys
moduli = {'PyQt6':'PyQt6','requests':'requests','psutil':'psutil','pywinctl':'pywinctl',
          'keyboard':'keyboard','send2trash':'send2trash','pycaw':'pycaw','comtypes':'comtypes',
          'sounddevice':'sounddevice'}
print(' '.join(p for m,p in moduli.items() if importlib.util.find_spec(m) is None))
"@
if ($mancanti -and $mancanti.Trim()) {
    Info "Mancano: $mancanti"
    if ($Prova) {
        Info "[prova] installerei: $mancanti"
    } else {
    & $py -m pip install --upgrade pip --quiet 2>&1 | Out-Null
    & $py -m pip install -r (Join-Path $Root 'requirements.txt') --quiet
    if ($LASTEXITCODE -ne 0) { Warn "Qualche dipendenza opzionale non e' entrata: NOVA parte lo stesso." }
    else { Ok "Dipendenze installate." }
    }
} else {
    Ok "Tutte le dipendenze sono gia' a posto."
}

# --------------------------------------------------------------- core Rust
Titolo "Il core"

New-Item -ItemType Directory -Force -Path $BinDir | Out-Null
$binari = @('novad.exe', 'nova-shell.exe', 'nova.exe')
function Core-Presente { foreach ($b in $binari) { if (-not (Test-Path (Join-Path $BinDir $b))) { return $false } }; return $true }

function Scarica-Core {
    # ATTENZIONE: «/releases/latest» ESCLUDE le prerelease. Finche' NOVA e' in
    # alpha ogni release e' una prerelease, quindi quell'endpoint da' sempre
    # vuoto e l'installer «non trova niente» pur essendo tutto pubblicato.
    # Si guarda l'elenco completo e si prende la piu' recente non bozza.
    $H = @{ 'User-Agent' = 'nova-installer' }
    $rel = $null
    try { $rel = Invoke-RestMethod "https://api.github.com/repos/$Repo/releases/latest" -Headers $H } catch { }
    if (-not $rel -or -not $rel.tag_name) {
        $tutte = Invoke-RestMethod "https://api.github.com/repos/$Repo/releases?per_page=10" -Headers $H
        $rel = $tutte | Where-Object { -not $_.draft } | Select-Object -First 1
    }
    if (-not $rel) { throw "nessuna release pubblicata" }
    $asset = $rel.assets | Where-Object { $_.name -eq 'nova-core-windows-x64.zip' } | Select-Object -First 1
    if (-not $asset) { throw "la release $($rel.tag_name) non contiene i binari per Windows" }
    if ($Prova) {
        Info "[prova] scaricherei e verificherei il core $($rel.tag_name) ($([math]::Round($asset.size/1MB,1)) MB)"
        Info "[prova]   da $($asset.browser_download_url)"
        Info "[prova]   in $BinDir"
        return
    }
    $zip = Join-Path $env:TEMP $asset.name
    Scarica $asset.browser_download_url $zip "core $($rel.tag_name)"
    Expand-Archive -Path $zip -DestinationPath $BinDir -Force
    Remove-Item $zip -Force
    # Gli hash sono pubblicati apposta: verificarli costa un secondo.
    $somme = Join-Path $BinDir 'SHA256SUMS.txt'
    if (Test-Path $somme) {
        foreach ($riga in Get-Content $somme) {
            if ($riga -match '^([0-9a-f]{64})\s+(\S+)$') {
                $file = Join-Path $BinDir $matches[2]
                if (Test-Path $file) {
                    if ((Get-FileHash $file -Algorithm SHA256).Hash.ToLower() -ne $matches[1]) {
                        throw "$($matches[2]): l'impronta non corrisponde. Non lo installo."
                    }
                }
            }
        }
        Ok "Impronte verificate."
    }
    if (-not (Core-Presente)) { throw "l'archivio non conteneva tutti i binari attesi" }
}

function Compila-Core {
    if ($Prova) { Info "[prova] compilerei il core da sorgente"; return }
    Info "Compilo da sorgente (diversi minuti)..."
    & (Join-Path $Root 'build.ps1')
    foreach ($b in $binari) {
        $src = Join-Path $Root "core\target\release\$b"
        if (-not (Test-Path $src)) { throw "compilazione incompleta: manca $b" }
        Copy-Item $src $BinDir -Force
    }
}

if ($DaSorgente) { Compila-Core }
else {
    try { Scarica-Core }
    catch {
        Warn "Non ho potuto scaricare il core: $($_.Exception.Message)"
        Info "Provo a compilarlo."
        try { Compila-Core }
        catch {
            Err "Non riesco ne' a scaricare ne' a compilare il core."
            Err "Per compilare servono Rust (https://rustup.rs) e i Build Tools di"
            Err "Visual Studio con «Desktop development with C++»."
            exit 1
        }
    }
}
Ok "Core pronto in $BinDir"
# ------------------------------------------------------------------- lingua
Titolo "La lingua"

# Si chiede prima del cervello perche' tocca tutto il resto. Il prompt di
# sistema resta in italiano in ogni caso: e' il sorgente di NOVA, e al modello
# si dice soltanto in che lingua rispondere.
$lingue = @(
    @{ codice = 'it'; nome = 'Italiano' },
    @{ codice = 'en'; nome = 'English' },
    @{ codice = 'es'; nome = 'Espanol' },
    @{ codice = 'fr'; nome = 'Francais' },
    @{ codice = 'de'; nome = 'Deutsch' },
    @{ codice = 'pt'; nome = 'Portugues' }
)
# Se il PC e' gia' configurato in una lingua, quello e' il suggerimento
# migliore che si possa dare: chi ha Windows in inglese non vuole NOVA in
# italiano solo perche' l'installer e' stato scritto qui.
$sistema = try { (Get-Culture).TwoLetterISOLanguageName } catch { 'it' }
$predLingua = 1
for ($i = 0; $i -lt $lingue.Count; $i++) { if ($lingue[$i].codice -eq $sistema) { $predLingua = $i + 1 } }

$etichetteLingua = @($lingue | ForEach-Object {
    if ($_.codice -in @('it', 'en')) { $_.nome }
    else { "$($_.nome) - NOVA risponde cosi', ma i menu restano in italiano" }
})
$lSel = $lingue[(Chiedi "In che lingua deve parlarti NOVA?" $etichetteLingua $predLingua) - 1]
Ok "Lingua: $($lSel.nome)"
if ($lSel.codice -notin @('it', 'en')) {
    Info "L'interfaccia e' tradotta in italiano e inglese: nelle altre lingue i"
    Info "nomi e i titoli restano in italiano finche' non ne arriva il dizionario."
}

# ------------------------------------------------------------------ cervello
Titolo "Il cervello"

# Se il catalogo manca — qualcuno ha scaricato solo l'installer, o il file e'
# rotto — non si muore con un errore di PowerShell: si dice cosa manca e si
# prosegue con le vie che non ne hanno bisogno.
$catalogo = $null
$fileCatalogo = Join-Path $Root 'models.json'
if (Test-Path $fileCatalogo) {
    try { $catalogo = Get-Content $fileCatalogo -Raw | ConvertFrom-Json }
    catch { Warn "models.json non e' leggibile: $($_.Exception.Message)" }
}
if (-not $catalogo) {
    Warn "Il catalogo dei modelli non c'e' o non si legge: posso configurare una"
    Warn "chiave API o un modello che hai gia', ma non scaricarne uno nuovo."
    Warn "Per riaverlo: scarica models.json dal repository accanto a install.ps1."
}

# Quanta VRAM c'e' davvero. Win32_VideoController.AdapterRAM e' un intero a 32
# bit e su ogni scheda sopra i 4 GB mente: dice sempre 4095 MB. Le fonti
# attendibili sono nvidia-smi e il registro.
function Rileva-VRAM {
    $smi = Get-Command nvidia-smi -ErrorAction SilentlyContinue
    if ($smi) {
        $mb = & $smi.Source --query-gpu=memory.total --format=csv,noheader,nounits 2>$null | Select-Object -First 1
        if ($mb -match '^\d+$') { return [math]::Round([int]$mb / 1024, 1) }
    }
    $classe = 'HKLM:\SYSTEM\CurrentControlSet\Control\Class\{4d36e968-e325-11ce-bfc1-08002be10318}'
    $max = 0
    Get-ChildItem $classe -ErrorAction SilentlyContinue | ForEach-Object {
        $q = (Get-ItemProperty $_.PSPath -Name 'HardwareInformation.qwMemorySize' -ErrorAction SilentlyContinue).'HardwareInformation.qwMemorySize'
        if ($q -and $q -gt $max) { $max = $q }
    }
    if ($max -gt 0) { return [math]::Round($max / 1GB, 1) }
    return 0
}

$vram = Rileva-VRAM
$ram  = [math]::Round((Get-CimInstance Win32_ComputerSystem).TotalPhysicalMemory / 1GB, 1)
$gpu  = (Get-CimInstance Win32_VideoController | Where-Object { $_.AdapterCompatibility -notmatch 'Microsoft' } |
         Select-Object -First 1 -ExpandProperty Name)
Info "GPU: $(if ($gpu) { $gpu } else { 'non rilevata' })"
Info "VRAM: $(if ($vram) { "$vram GB" } else { 'sconosciuta' })   RAM: $ram GB"

# La variante piu' grande che ENTRA, non la piu' grande che si riesce a
# caricare: se non ci sta tutta in VRAM, llama.cpp mette il resto in RAM e la
# velocita' crolla.
function Scegli-Variante($famiglia, $vramGb) {
    if (-not $vramGb) { return $null }
    $famiglia.varianti | Where-Object { $_.vram_gb -le $vramGb } |
        Sort-Object { $_.gb } -Descending | Select-Object -First 1
}

$famiglia = if ($catalogo) { $catalogo.famiglie | Where-Object { $_.consigliata } | Select-Object -First 1 } else { $null }
$suggerita = if ($famiglia) { Scegli-Variante $famiglia $vram } else { $null }

# Cercare i modelli e riconoscere un GGUF sono cose che NOVA sa gia' fare, in
# nova/modelli_trova.py. Averne una seconda copia qui in PowerShell voleva dire
# tenerle allineate a mano: si aggiunge una cartella a una e non all'altra, e
# nessuno se ne accorge finche' qualcuno non si lamenta che il suo modello non
# viene visto. Qui restano solo due involucri.
function Trova-Gguf {
    if (-not $py) { return @() }
    Push-Location $Root
    try {
        $grezzo = & $py -m nova.modelli_trova --secondi 20 2>$null | Out-String
        if (-not $grezzo.Trim()) { return @() }
        $r = $grezzo | ConvertFrom-Json
        if ($r.troncato) {
            Warn "Mi sono fermato dopo $($r.secondi) secondi: se il tuo modello non e' in elenco, indicalo a mano."
        }
        return @($r.modelli)
    } catch { return @() } finally { Pop-Location }
}

function Verifica-Gguf($percorso) {
    if (-not $py) { return $null }
    Push-Location $Root
    try { return (& $py -m nova.modelli_trova --verifica "$percorso" 2>$null | Out-String | ConvertFrom-Json) }
    catch { return $null } finally { Pop-Location }
}

# Le CLI degli abbonamenti. NOVA sa gia' pilotarle - le specifiche stanno in
# nova/routing.py, cli_predefinite() - ma finora nessuno le nominava: per usare
# Codex o Gemini bisognava scrivere a mano una voce in config.json, cioe'
# bisognava sapere che quella voce esisteva. Qui si guarda solo se il binario
# c'e'; cosa passargli lo sa Python.
function Trova-Cli {
    $note = @(
        @{ nome = 'claude'; binario = 'claude'; etichetta = 'Claude Code' },
        @{ nome = 'codex';  binario = 'codex';  etichetta = 'Codex (OpenAI)' },
        @{ nome = 'gemini'; binario = 'gemini'; etichetta = 'Gemini' },
        @{ nome = 'qwen';   binario = 'qwen';   etichetta = 'Qwen Code' }
    )
    $fuori = @()
    foreach ($c in $note) {
        foreach ($sfx in @('.cmd', '.exe', '')) {
            # npm su Windows installa .cmd: e' quello che si riesce a eseguire
            $g = Get-Command ($c.binario + $sfx) -ErrorAction SilentlyContinue | Select-Object -First 1
            if ($g) { $c['percorso'] = $g.Source; $fuori += $c; break }
        }
    }
    $fuori
}

# Chi ha l'SSD di sistema piccolo e i modelli su un altro disco finora non
# aveva modo di dirlo: la cartella era cablata accanto a NOVA.
function Chiedi-Cartella-Modelli($servonoGb) {
    $predefinita = Join-Path $Runtime 'modelli'
    $dischi = Get-CimInstance Win32_LogicalDisk -Filter 'DriveType=3' -ErrorAction SilentlyContinue |
        ForEach-Object { "{0} {1} GB" -f $_.DeviceID, [math]::Round($_.FreeSpace / 1GB) }
    if ($dischi) { Info "Spazio libero: $($dischi -join '   ')" }
    $scelta = Chiedi-Testo "Dove metto i modelli? [$predefinita]" $predefinita
    $scelta = [IO.Path]::GetFullPath($scelta)
    # In prova non si crea niente: una cartella lasciata in giro da un giro a
    # vuoto e' esattamente cio' che «non ho toccato niente» promette di non fare.
    if ($Prova) {
        Info "[prova] userei la cartella $scelta"
    } else {
        try {
            New-Item -ItemType Directory -Force -Path $scelta -ErrorAction Stop | Out-Null
        } catch {
            Warn "Non riesco a creare $scelta - uso $predefinita"
            $scelta = [IO.Path]::GetFullPath($predefinita)
            New-Item -ItemType Directory -Force -Path $scelta | Out-Null
        }
    }
    $radice = [IO.Path]::GetPathRoot($scelta).TrimEnd('\')
    $d = Get-CimInstance Win32_LogicalDisk -Filter "DeviceID='$radice'" -ErrorAction SilentlyContinue
    if ($d -and ($d.FreeSpace / 1GB) -lt $servonoGb) {
        Err "Su $radice ci sono $([math]::Round($d.FreeSpace/1GB,1)) GB liberi e ne servono $([math]::Round($servonoGb,1))."
        exit 1
    }
    return $scelta
}

# Ollama, LM Studio, llama.cpp server e KoboldCpp parlano tutti il dialetto di
# OpenAI. Se uno di loro e' gia' acceso, il cervello c'e' gia': niente da
# scaricare, niente da pagare. Su localhost una porta chiusa rifiuta subito,
# quindi provarle tutte costa un istante.
function Trova-Server-Locale {
    $porte = @(
        @{ nome = 'Ollama';           url = 'http://127.0.0.1:11434' },
        @{ nome = 'LM Studio';        url = 'http://127.0.0.1:1234'  },
        @{ nome = 'llama.cpp server'; url = 'http://127.0.0.1:8080'  },
        @{ nome = 'KoboldCpp';        url = 'http://127.0.0.1:5001'  }
    )
    foreach ($p in $porte) {
        try {
            $r = Invoke-RestMethod -Uri "$($p.url)/v1/models" -TimeoutSec 3 -ErrorAction Stop
            $modelli = @($r.data | ForEach-Object { $_.id } | Where-Object { $_ })
            if ($modelli.Count) { return @{ nome = $p.nome; url = $p.url; modelli = $modelli } }
        } catch { }
    }
    return $null
}

Info "Guardo se hai gia' un modello, un abbonamento o un server acceso..."
$gia = @(Trova-Gguf)
$srv = Trova-Server-Locale
$cli = @(Trova-Cli)

# Prima la famiglia, poi il dettaglio. Le sei vie di prima erano tutte vere e
# tutte allo stesso livello: chi installa doveva confrontare «una chiave API»
# con «un server gia' acceso» senza sapere che la seconda e' un modo di dire
# la prima. Le famiglie sono quattro, e sono la domanda che uno si fa davvero:
# dove gira il modello, e chi lo paga.
$famiglie = @(
    'Nessuna per ora - la scelgo dopo dalle impostazioni',
    $(if ($gia.Count -or $srv) { "In casa: un modello sul tuo PC (ne vedo gia' qualcosa)" }
      else { 'In casa: un modello che gira sul tuo PC' }),
    $(if ($cli.Count) { "Un abbonamento che hai gia': $(($cli | ForEach-Object { $_.etichetta }) -join ', ')" }
      else { 'Un abbonamento (Claude Code, Codex, Gemini, Qwen): non ne vedo installati' }),
    'Una chiave API - si paga a consumo'
)
$fam = Chiedi "Con quale IA vuoi far ragionare NOVA, per cominciare?" $famiglie $(if ($srv -or $gia.Count) { 2 } else { 1 })

switch ($fam) {
  1 { Info "In casa non esce niente e non si paga niente: serve una scheda video capace." }
  2 { Info "In casa: niente esce dal PC e non si paga a consumo. Serve spazio su disco," 
      Info "e la velocita' dipende dalla scheda video." }
  3 { Info "Un abbonamento gia' pagato e' la via piu' potente e la piu' delicata:" 
      Info "leggi l'avvertenza qui sotto prima di sceglierla." }
  4 { Info "Una chiave API si paga a consumo, funziona subito e non occupa disco." }
}

# `$scelta` resta la numerazione di prima: le famiglie scelgono per l'utente,
# ma sotto non cambia niente. Riscrivere anche i rami avrebbe voluto dire
# rifare - e poter rompere - cinque procedure che gia' funzionano, per un
# menu'.
$scelta = 6
switch ($fam) {
  1 { $scelta = 6 }
  3 { $scelta = 2 }
  4 { $scelta = 1 }
  2 {
      $inCasa = @(
          $(if ($gia.Count) { "Un modello che ho gia': ne ho trovati $($gia.Count) sul disco" }
            else { "Un modello che ho gia': indico il percorso" }),
          $(if ($srv) { "Un server gia' acceso: $($srv.nome) su $($srv.url)" }
            else { "Un server locale (Ollama, LM Studio, llama.cpp): non ne vedo di accesi" }),
          $(if ($suggerita) { "Scarico un modello: $($famiglia.nome) $($suggerita.qualita) ($($suggerita.gb) GB)" }
            elseif ($famiglia) { "Scarico un modello: la tua GPU non regge $($famiglia.nome), girerebbe in RAM (molto lento)" }
            else { 'Scarico un modello: non disponibile, manca il catalogo' })
      )
      $pre = if ($srv) { 2 } elseif ($gia.Count) { 1 } else { 3 }
      $scelta = @(3, 4, 5)[(Chiedi "In casa, come?" $inCasa $pre) - 1]
  }
}
$opzioni = $famiglie
# Un server gia' acceso e' l'unica strada senza attesa e senza costi: parte in
# vantaggio anche in installazione silenziosa. Un abbonamento no, mai in
# automatico: usare la CLI di un abbonamento consumer come motore di
# un'applicazione terza e' fuori dai termini di servizio di quasi tutti i
# fornitori, e quella scelta deve restare di chi ci mette l'account.
$cfgPatch = @{}

switch ($scelta) {
  1 {
      $chiave = Chiedi-Testo "Incolla la chiave API (invio per saltare)"
      if ($chiave) {
          $base = Chiedi-Testo "URL del servizio [https://api.openai.com]" 'https://api.openai.com'
          $modello = Chiedi-Testo "Nome del modello (es. gpt-4o-mini)"
          $cfgPatch['brains'] = @{ active = 'api'; api_key = $chiave; api_base_url = $base; api_model = $modello }
          Ok "Cervello: API remota."
      } else { Warn "Nessuna chiave: dovrai configurarla dopo." }
  }
  2 {
      if (-not $cli.Count) {
          Warn "Non trovo nessuna CLI nel PATH. Installane una e riesegui, oppure scegli un'altra strada."
          break
      }
      $et = @($cli | ForEach-Object { "$($_.etichetta)  ($($_.percorso))" })
      $sc = $cli[(Chiedi "Quale abbonamento?" $et 1) - 1]
      $cfgPatch['brains'] = @{ active = $sc.nome }
      Ok "Cervello: $($sc.etichetta)"
      Warn "Usare la CLI di un abbonamento consumer come motore di un'applicazione"
      Warn "terza e' fuori dai termini di servizio di quasi tutti i fornitori: il"
      Warn "rischio ricade sul tuo account. NOVA lo permette, non lo consiglia."
      if ($sc.nome -ne 'claude') {
          Info "Se questa CLI vuole argomenti diversi si regolano in config.json,"
          Info "sotto brains.cli.$($sc.nome).args - NOVA non li indovina."
      }
  }
  3 {
      $scelto = $null
      if ($gia.Count) {
          $et = @($gia | ForEach-Object { "{0}  ({1} GB)  in {2}" -f $_.nome, $_.gb, $_.cartella })
          $et += 'Nessuno di questi: indico il percorso a mano'
          $n = Chiedi "Quale modello uso?" $et 1
          if ($n -le $gia.Count) { $scelto = $gia[$n - 1] }
      }
      if (-not $scelto) {
          $percorso = Chiedi-Testo "Percorso del file .gguf (invio per saltare)"
          if ($percorso) {
              $v = Verifica-Gguf $percorso
              if ($v -and $v.ok) { $scelto = $v }
              elseif ($v) { Warn "$($v.percorso): $($v.motivo)" }
              else { Warn "Non riesco a controllare quel file." }
          }
      }
      if (-not $scelto) { Warn "Nessun modello indicato: lo configurerai dopo." }
      else {
          $cfgPatch['brains'] = @{ active = 'locale' }
          $cfgPatch['server'] = @{ model_path = $scelto.percorso; models_dir = $scelto.cartella }
          Ok "Cervello: $($scelto.nome) ($($scelto.gb) GB)"
          if ($vram -and $scelto.gb -gt $vram) {
              Warn "Il file e' piu' grande della VRAM ($vram GB): una parte girera' in RAM e andra' piano."
          }
          if ($scelto.proiettore) { Ok "Accanto c'e' anche la vista: $(Split-Path $scelto.proiettore -Leaf)" }
          else { Info "Nessun proiettore accanto al modello: con questo cervello NOVA non vedra' le immagini." }
      }
  }
  4 {
      $sv = if ($srv) { $srv } else { Trova-Server-Locale }
      $url = $null; $mod = $null
      if ($sv) {
          Ok "Trovato $($sv.nome) su $($sv.url)"
          $url = $sv.url
          if ($sv.modelli.Count -eq 1) { $mod = $sv.modelli[0] }
          else { $mod = $sv.modelli[(Chiedi "Quale dei modelli di $($sv.nome)?" $sv.modelli 1) - 1] }
      } else {
          Warn "Non vedo nessun server acceso sulle porte solite (11434, 1234, 8080, 5001)."
          $url = Chiedi-Testo "Indirizzo del server (invio per saltare, es. http://127.0.0.1:11434)"
          if ($url) { $mod = Chiedi-Testo "Nome del modello" }
      }
      if ($url -and $mod) {
          # Un server in casa non chiede chiavi, e NOVA non ne pretende.
          $cfgPatch['brains'] = @{ active = 'api'; api_base_url = $url; api_model = $mod; api_key = '' }
          Ok "Cervello: $mod su $url"
          Info "Quel server deve restare acceso perche' NOVA possa ragionare."
      } else { Warn "Nessun server configurato: lo farai dopo." }
  }
  5 {
      if (-not $famiglia) {
          Warn "Senza catalogo non so quale modello proporti: configuralo dopo, o usa l'opzione 3."
          break
      }
      if (-not $suggerita) {
          $suggerita = $famiglia.varianti | Sort-Object { $_.gb } | Select-Object -First 1
          Warn "Scarico la variante piu' leggera, ma su questa macchina andra' piano."
      }
      # Il consiglio non e' un obbligo: chi vuole spingere o alleggerire deve
      # poterlo fare qui, non scoprendo dopo che l'installer ha deciso da solo.
      if ($famiglia.varianti.Count -gt 1) {
          $sc = Chiedi "Quale versione di $($famiglia.nome)?" @(
              "Quella consigliata per la tua scheda: $($suggerita.qualita) ($($suggerita.gb) GB)",
              'Scelgo io fra tutte le versioni'
          ) 1
          if ($sc -eq 2) {
              $lista = @($famiglia.varianti | Sort-Object { $_.gb } -Descending)
              $et = @($lista | ForEach-Object {
                  $nota = if ($vram -and $_.vram_gb -le $vram) { 'entra in VRAM' }
                          elseif ($vram) { "servono $($_.vram_gb) GB di VRAM: andra' piano" }
                          else { "servono $($_.vram_gb) GB di VRAM" }
                  "$($_.qualita) - $($_.gb) GB - $nota"
              })
              $pre = 1
              for ($i = 0; $i -lt $lista.Count; $i++) { if ($lista[$i].file -eq $suggerita.file) { $pre = $i + 1 } }
              $suggerita = $lista[(Chiedi "Quale versione?" $et $pre) - 1]
          }
      }
      # Il proiettore visivo pesa quasi un giga in piu': va contato nello
      # spazio richiesto, non scoperto a meta' scaricamento.
      $vista = $famiglia.proiettore
      $totale = $suggerita.gb + $(if ($vista) { $vista.gb } else { 0 })
      $mDir = Chiedi-Cartella-Modelli ($totale + 2)
      $dest = Join-Path $mDir $suggerita.file
      Scarica ($famiglia.url_base + $suggerita.file) $dest "$($famiglia.nome) $($suggerita.qualita)"

      # Senza questo il modello e' cieco anche se saprebbe vedere, e llama.cpp
      # non lo segnala: si scaricherebbe la vista spenta in silenzio, che e'
      # peggio di non averla. Se non riesce si dice, e NOVA funziona lo stesso.
      if ($vista) {
          try {
              Scarica ($famiglia.url_base + $vista.file) (Join-Path $mDir $vista.file) "vista del modello"
          } catch {
              Warn "Il proiettore visivo non e' stato scaricato: $($_.Exception.Message)"
              Warn "NOVA funzionera', ma con il modello locale non vedra' le immagini."
              Warn "Per aggiungerla dopo, scarica $($vista.file) da $($famiglia.repo) e mettilo in $mDir"
          }
      }

      $cfgPatch['brains'] = @{ active = 'locale' }
      $cfgPatch['server'] = @{ model_path = $dest; models_dir = $mDir }
      Ok "Cervello: modello locale."
  }
  6 { Warn "NOVA si installa senza cervello: si avvia ma non potra' rispondere." }
}
# ------------------------------------------------------------------- ascolto
# Ascolto e voce sono due scelte, non una. Prima erano una sola - «come vuoi
# parlare con NOVA» - e chi voleva sentirla parlare bene senza mandare fuori la
# propria voce non aveva la casella: doveva prendere ElevenLabs per tutti e due
# o niente per tutti e due.
Titolo "Come ti ascolta"

function Procura-Componenti($nomi, $etichetta) {
    if ($Prova) { Info "[prova] scaricherei: $($nomi -join ', ')"; return $true }
    # Cosa serve, dove si prende e come si mette a posto lo sa
    # nova/componenti.py - lo stesso posto che usa il pannello quando qualcuno
    # cambia idea dopo. Due copie della stessa procedura sono due procedure che
    # divergono.
    $completa = $true
    Push-Location $Root
    try {
        foreach ($c in $nomi) {
            Info "Procuro: $c"
            $ultimo = ''
            & $py -m nova.componenti --scarica $c 2>&1 | ForEach-Object {
                $riga = "$_"
                if ($riga -match '"evento":\s*"(errore|finito|interrotto)"') { $ultimo = $riga }
            }
            if ($LASTEXITCODE -ne 0 -or $ultimo -match '"evento":\s*"errore"') {
                $completa = $false
                Warn "«$c» non e' stato completato."
                if ($ultimo) { Warn "  $ultimo" }
            }
        }
    } finally { Pop-Location }
    if (-not $completa) {
        Warn "$etichetta - qualche pezzo manca. NOVA funziona lo stesso, e i pezzi"
        Warn "che mancano si scaricano dalle impostazioni, sezione Componenti."
    }
    return $completa
}

$aOpz = @(
    'Non mi ascolta - decido dopo dalle impostazioni',
    'In casa: whisper.cpp (circa 420 MB, la tua voce non esce dal PC)',
    'ElevenLabs Scribe (serve una chiave; trascrive meglio, ma la voce esce)'
)
$aScelta = Chiedi "Come deve ascoltarti?" $aOpz 1
$vocePatch = @{}

switch ($aScelta) {
  1 { Info "Nessun ascolto: si attiva quando vuoi dalle impostazioni." }
  2 {
      if (Procura-Componenti @('ascolto_locale') 'Ascolto in casa') { Ok "Ascolto: in casa." }
      $vocePatch['enabled'] = $true
      $vocePatch['stt_engine'] = 'faster-whisper'
  }
  3 {
      $k = Chiedi-Testo "Chiave ElevenLabs (invio per saltare)"
      if ($k.Trim()) {
          $vocePatch['enabled'] = $true
          $vocePatch['stt_engine'] = 'elevenlabs'
          $vocePatch['api_key'] = $k.Trim()
          Ok "Ascolto: ElevenLabs Scribe."
          Info "Se la rete manca o la chiave viene rifiutata, NOVA ascolta in locale"
          Info "invece di restare sorda - ma i pezzi di whisper devono esserci."
          if ((Chiedi "Scarico anche l'ascolto in casa, come rete di sicurezza?" @('Si', 'No') 1) -eq 1) {
              Procura-Componenti @('ascolto_locale') 'Ascolto in casa' | Out-Null
          }
      } else { Warn "Senza chiave non attivo l'ascolto." }
  }
}

# --------------------------------------------------------------------- voce
Titolo "Come ti parla"

$vOpz = @(
    'Non parla - decido dopo dalle impostazioni',
    'In casa: Kokoro (circa 840 MB, niente esce dal PC, nessun tetto)',
    'La voce di Windows (gratis, illimitata, meccanica)',
    'ElevenLabs (serve una chiave; 10.000 caratteri al mese sul piano gratuito)'
)
$vScelta = Chiedi "Come deve parlarti?" $vOpz 3

switch ($vScelta) {
  1 { Info "Niente voce: si attiva quando vuoi dalle impostazioni." }
  2 {
      if (Procura-Componenti @('voce_locale', 'onnx', 'espeak') 'Voce in casa') { Ok "Voce: in casa, Kokoro." }
      $vocePatch['enabled'] = $true
      $vocePatch['tts_engine'] = 'locale'
  }
  3 {
      $vocePatch['enabled'] = $true
      $vocePatch['tts_engine'] = 'sapi'
      Ok "Voce: quella di Windows. Niente da scaricare."
  }
  4 {
      $k = if ($vocePatch['api_key']) { $vocePatch['api_key'] } else { Chiedi-Testo "Chiave ElevenLabs (invio per saltare)" }
      if ("$k".Trim()) {
          $vocePatch['enabled'] = $true
          $vocePatch['tts_engine'] = 'elevenlabs'
          $vocePatch['api_key'] = "$k".Trim()
          Ok "Voce: ElevenLabs."
          Info "Finiti i caratteri del mese NOVA continua a parlare con la voce di casa,"
          Info "se c'e': senza, resta muta. Conviene scaricarla come rete di sicurezza."
          if ((Chiedi "Scarico anche la voce in casa?" @('Si', 'No') 1) -eq 1) {
              Procura-Componenti @('voce_locale', 'onnx', 'espeak') 'Voce in casa' | Out-Null
          }
      } else { Warn "Senza chiave non attivo la voce." }
  }
}

if ($vocePatch.Count -gt 0) { $cfgPatch['voice'] = $vocePatch }

# La lingua entra qui e non prima: i rami della voce riscrivono per intero la
# sezione «voice», e una chiave messa prima sparirebbe senza che nessuno se ne
# accorga fino al primo ascolto nella lingua sbagliata.
$cfgLingua = @{ ui = @{ lingua = $lSel.codice }; voice = @{ language = $lSel.codice } }
foreach ($sez in $cfgLingua.Keys) {
    if ($cfgPatch.ContainsKey($sez)) {
        foreach ($k in $cfgLingua[$sez].Keys) {
            if (-not $cfgPatch[$sez].ContainsKey($k)) { $cfgPatch[$sez][$k] = $cfgLingua[$sez][$k] }
        }
    } else { $cfgPatch[$sez] = $cfgLingua[$sez] }
}

# ------------------------------------------------------------ configurazione
Titolo "Configurazione"

Info "Rilevo runtime e modelli gia' presenti..."
Push-Location $Root
try { if ($Prova) { Info "[prova] rileverei modello e runtime e salverei la configurazione" } else { & $py -c "from nova.config import Config;from nova.setup_wizard import autoconfigure;c=Config.load();[print('  ',n) for n in autoconfigure(c,force=True)];c.save()" } }
finally { Pop-Location }

# Le scelte fatte qui sopra vanno scritte DOPO autoconfigure, che altrimenti
# le sovrascriverebbe con quello che ha trovato in giro.
if ($cfgPatch.Count -gt 0 -and $Prova) {
    Info "[prova] scriverei in configurazione: $($cfgPatch.Keys -join ', ')"
} elseif ($cfgPatch.Count -gt 0) {
    $json = ($cfgPatch | ConvertTo-Json -Depth 5 -Compress)
    $tmpJson = Join-Path $env:TEMP 'nova_patch.json'
    [IO.File]::WriteAllText($tmpJson, $json, (New-Object Text.UTF8Encoding($false)))
    Push-Location $Root
    try {
        & $py -c @"
import json, sys
from nova.config import Config
patch = json.load(open(r'$tmpJson', encoding='utf-8'))
c = Config.load()
for sezione, valori in patch.items():
    obj = getattr(c, sezione, None)
    if obj is None: continue
    for k, v in valori.items():
        if hasattr(obj, k): setattr(obj, k, v)
c.save()
print('  configurazione aggiornata:', ', '.join(patch))
"@
    } finally { Pop-Location }
    Remove-Item $tmpJson -Force -ErrorAction SilentlyContinue
}
# ------------------------------------------------------------ avvio e scorciatoie
Titolo "Avvio"

$shell = Join-Path $BinDir 'nova-shell.exe'
if (-not $SenzaAvvioAuto) {
    if ($Prova) { Info "[prova] configurerei l'avvio automatico" }
    else {
        Set-ItemProperty 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Run' -Name $RunName -Value "`"$shell`""
        Ok "NOVA si avviera' da sola all'accensione."
    }
}
if ($Prova) { Info "[prova] creerei il collegamento sul Desktop" }
else {
    $ws  = New-Object -ComObject WScript.Shell
    $lnk = $ws.CreateShortcut((Join-Path ([Environment]::GetFolderPath('Desktop')) 'NOVA.lnk'))
    $lnk.TargetPath = $shell; $lnk.WorkingDirectory = $Root
    $lnk.Description = 'NOVA - assistente digitale locale'
    $lnk.Save()
    Ok "Collegamento sul Desktop."
}

# ------------------------------------------------------------------ verifica
Titolo "Verifica"

# Un'installazione non e' riuscita perche' i file sono al loro posto: e'
# riuscita se NOVA risponde. Conta la conseguenza, non il gesto.
$esiti = @()

foreach ($b in $binari) {
    $ok = Test-Path (Join-Path $BinDir $b)
    $esiti += [pscustomobject]@{ Cosa = $b; Ok = $ok }
}

$demone = Join-Path $BinDir 'novad.exe'
$cli    = Join-Path $BinDir 'nova.exe'
if ($Prova) {
    Info "[prova] avvierei il demone e chiederei a NOVA di rispondere"
    Write-Host ""
    Ok "Prova finita: non ho toccato niente."
    exit 0
}
$giaSu  = [bool](Get-Process novad -ErrorAction SilentlyContinue)
if (-not $giaSu) { Start-Process $demone -WindowStyle Hidden; Start-Sleep -Seconds 6 }

# «status» e' il comando vero del client: «list» e «sistema.stato» non
# esistono, e un controllo di salute che interroga un comando inesistente
# fallisce sempre, cioe' mente su un sistema sano.
$demoneVivo = $false
foreach ($tentativo in 1..3) {
    try {
        $null = & $cli status 2>&1
        if ($LASTEXITCODE -eq 0) { $demoneVivo = $true; break }
    } catch { }
    Start-Sleep -Seconds 3
}
$esiti += [pscustomobject]@{ Cosa = 'il demone risponde'; Ok = $demoneVivo }

# La prova vera: il cervello sa rispondere a una domanda?
$cervelloOk = $false
$cfgFinale = & $py -c "from nova.config import Config;c=Config.load();print(c.brains.active + '|' + (c.server.model_path or '') + '|' + (c.brains.api_key or ''))" 2>$null
$parti = ($cfgFinale -split '\|')
$attivo = $parti[0]
$haCervello = ($attivo -eq 'locale' -and $parti[1]) -or ($attivo -eq 'api' -and $parti[2]) -or ($attivo -eq 'claude')
if ($haCervello) {
    Info "Provo a fargli una domanda (puo' volerci un minuto: carica il modello)..."
    Push-Location $Root
    try {
        $risposta = & $py -m nova --ask "Rispondi solo con la parola: pronto" 2>$null | Out-String
        $cervelloOk = ($risposta -match '(?i)pronto')
    } catch { $cervelloOk = $false } finally { Pop-Location }
} 
$esiti += [pscustomobject]@{ Cosa = 'il cervello risponde'; Ok = $cervelloOk }

Write-Host ""
foreach ($e in $esiti) {
    if ($e.Ok) { Write-Host "   [ok]     $($e.Cosa)" -ForegroundColor Green }
    else       { Write-Host "   [manca]  $($e.Cosa)" -ForegroundColor Yellow }
}

$tuttoOk = -not ($esiti | Where-Object { -not $_.Ok })
Write-Host ""
if ($tuttoOk) {
    Ok "Installazione completata e verificata: NOVA risponde."
} elseif ($demoneVivo) {
    Warn "NOVA e' installata e il demone gira, ma non e' tutto a posto."
    if (-not $cervelloOk) {
        if (-not $haCervello) {
            Warn "Non hai ancora scelto un cervello: rilancia l'installer, oppure"
            Warn "imposta una chiave API o un modello dalle impostazioni."
        } else {
            Warn "Il cervello e' configurato ma non ha risposto. Prova a mano con:"
            Warn "  python -m nova --ask \"ciao\""
            Warn "cosi' vedi l'errore per esteso."
        }
    }
} else {
    Err "Il demone non risponde: NOVA non funzionera'."
    Err "Prova ad avviarlo a mano e guarda cosa dice:"
    Err "  $demone"
}

Write-Host ""
Write-Host "  Avvia NOVA dal collegamento sul Desktop: comparira' un orb in un angolo." -ForegroundColor Gray
Write-Host "  Cliccalo per scrivere, oppure chiamala per nome se hai attivato la voce." -ForegroundColor Gray
Write-Host ""
Write-Host "  NOVA parte con «conferma sempre»: chiede il permesso prima di ogni azione" -ForegroundColor Gray
Write-Host "  che tocca il sistema, e ti dice cosa sta per fare. Allentalo quando ti fidi." -ForegroundColor Gray
Write-Host ""
