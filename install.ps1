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
    [switch]$Disinstalla
)

$ErrorActionPreference = 'Stop'
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
function Chiedi($domanda, $opzioni, $predefinita = 1) {
    if ($Silenzioso) { return $predefinita }
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
            Start-Process $boot -ArgumentList '/silent', '/install' -Wait
            Ok "WebView2 installato."
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
    & $py -m pip install --upgrade pip --quiet 2>&1 | Out-Null
    & $py -m pip install -r (Join-Path $Root 'requirements.txt') --quiet
    if ($LASTEXITCODE -ne 0) { Warn "Qualche dipendenza opzionale non e' entrata: NOVA parte lo stesso." }
    else { Ok "Dipendenze installate." }
} else {
    Ok "Tutte le dipendenze sono gia' a posto."
}

# --------------------------------------------------------------- core Rust
Titolo "Il core"

New-Item -ItemType Directory -Force -Path $BinDir | Out-Null
$binari = @('novad.exe', 'nova-shell.exe', 'nova.exe')
function Core-Presente { foreach ($b in $binari) { if (-not (Test-Path (Join-Path $BinDir $b))) { return $false } }; return $true }

function Scarica-Core {
    $rel = Invoke-RestMethod "https://api.github.com/repos/$Repo/releases/latest" -Headers @{ 'User-Agent' = 'nova-installer' }
    $asset = $rel.assets | Where-Object { $_.name -eq 'nova-core-windows-x64.zip' } | Select-Object -First 1
    if (-not $asset) { throw "la release $($rel.tag_name) non contiene i binari per Windows" }
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
# ------------------------------------------------------------------ cervello
Titolo "Il cervello"

$catalogo = Get-Content (Join-Path $Root 'models.json') -Raw | ConvertFrom-Json

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

$famiglia = $catalogo.famiglie | Where-Object { $_.consigliata } | Select-Object -First 1
$suggerita = Scegli-Variante $famiglia $vram

$opzioni = @(
    'Chiave API (qualita, si paga a consumo) - la strada consigliata',
    $(if ($suggerita) { "Modello locale: $($famiglia.nome) $($suggerita.qualita) ($($suggerita.gb) GB da scaricare)" }
      else { "Modello locale: la tua GPU non regge $($famiglia.nome), girerebbe in RAM (molto lento)" }),
    'Ho gia un file .gguf: indico il percorso',
    'Decido dopo (NOVA si installa ma non potra ragionare)'
)
$scelta = Chiedi "Chi fa ragionare NOVA?" $opzioni $(if ($suggerita) { 1 } else { 1 })

$cfgPatch = @{}

switch ($scelta) {
  1 {
      $chiave = if ($Silenzioso) { '' } else { Read-Host "  Incolla la chiave API (invio per saltare)" }
      if ($chiave.Trim()) {
          $base = Read-Host "  URL del servizio [https://api.openai.com]"
          if (-not $base.Trim()) { $base = 'https://api.openai.com' }
          $modello = Read-Host "  Nome del modello (es. gpt-4o-mini)"
          $cfgPatch['brains'] = @{ active = 'api'; api_key = $chiave.Trim(); api_base_url = $base.Trim(); api_model = $modello.Trim() }
          Ok "Cervello: API remota."
      } else { Warn "Nessuna chiave: dovrai configurarla dopo." }
  }
  2 {
      if (-not $suggerita) {
          $suggerita = $famiglia.varianti | Sort-Object { $_.gb } | Select-Object -First 1
          Warn "Scarico la variante piu' leggera, ma su questa macchina andra' piano."
      }
      $mDir = Join-Path $Runtime 'modelli'
      New-Item -ItemType Directory -Force -Path $mDir | Out-Null
      $dest = Join-Path $mDir $suggerita.file
      if ($libero -lt ($suggerita.gb + 2)) {
          Err "Servono almeno $([math]::Round($suggerita.gb + 2,1)) GB liberi, ce ne sono $([math]::Round($libero,1))."
          exit 1
      }
      Scarica ($famiglia.url_base + $suggerita.file) $dest "$($famiglia.nome) $($suggerita.qualita)"
      $cfgPatch['brains'] = @{ active = 'locale' }
      $cfgPatch['server'] = @{ model_path = $dest }
      Ok "Cervello: modello locale."
  }
  3 {
      $percorso = Read-Host "  Percorso del file .gguf"
      if (Test-Path $percorso) {
          $cfgPatch['brains'] = @{ active = 'locale' }
          $cfgPatch['server'] = @{ model_path = (Resolve-Path $percorso).Path }
          Ok "Cervello: $(Split-Path $percorso -Leaf)"
      } else { Warn "Non trovo «$percorso»: lo configurerai dopo." }
  }
  4 { Warn "NOVA si installa senza cervello: si avvia ma non potra' rispondere." }
}
# ---------------------------------------------------------------------- voce
Titolo "La voce"

$vOpz = @(
    'Alta qualita con ElevenLabs (serve una chiave, la voce esce dal PC)',
    'Tutto in locale (circa 800 MB da scaricare, niente esce dal PC)',
    'Niente voce per ora'
)
$vScelta = Chiedi "Come vuoi parlare con NOVA?" $vOpz 3

switch ($vScelta) {
  1 {
      $k = if ($Silenzioso) { '' } else { Read-Host "  Chiave ElevenLabs (invio per saltare)" }
      if ($k.Trim()) {
          $cfgPatch['voice'] = @{ enabled = $true; api_key = $k.Trim(); tts_engine = 'elevenlabs'; stt_engine = 'elevenlabs' }
          Ok "Voce: ElevenLabs. Nessun download."
      } else { Warn "Senza chiave non attivo la voce." }
  }
  2 {
      $vDir = Join-Path $Runtime 'voce'
      $aDir = Join-Path $Runtime 'ascolto'
      New-Item -ItemType Directory -Force -Path $vDir, $aDir | Out-Null
      $completa = $true
      try {
          # Sintesi: Kokoro.
          $kb = 'https://github.com/thewh1teagle/kokoro-onnx/releases/download/model-files-v1.0'
          Scarica "$kb/kokoro-v1.0.onnx" (Join-Path $vDir 'kokoro-v1.0.onnx') 'Kokoro (sintesi)'
          Scarica "$kb/voices-v1.0.bin"  (Join-Path $vDir 'voices-v1.0.bin')  'Voci'
          Copy-Item (Join-Path $Root 'core\crates\nova-voce\src\vocab.json') $vDir -Force -ErrorAction SilentlyContinue

          # ONNX Runtime: il crate ort lo carica dinamicamente, non e' collegato.
          $ozip = Join-Path $env:TEMP 'onnxruntime.zip'
          Scarica 'https://github.com/microsoft/onnxruntime/releases/download/v1.20.1/onnxruntime-win-x64-1.20.1.zip' $ozip 'ONNX Runtime'
          $tmp = Join-Path $env:TEMP 'ort_estratto'
          Remove-Item $tmp -Recurse -Force -ErrorAction SilentlyContinue
          Expand-Archive $ozip -DestinationPath $tmp -Force
          Get-ChildItem $tmp -Recurse -Filter 'onnxruntime*.dll' | ForEach-Object { Copy-Item $_.FullName $vDir -Force }
          Remove-Item $ozip, $tmp -Recurse -Force -ErrorAction SilentlyContinue

          # Ascolto: whisper.cpp e il modello.
          Scarica 'https://github.com/ggml-org/whisper.cpp/releases/download/v1.8.4/whisper-cublas-12.4.0-bin-x64.zip' `
                  (Join-Path $aDir 'whisper.zip') 'whisper.cpp'
          Expand-Archive (Join-Path $aDir 'whisper.zip') -DestinationPath $aDir -Force
          Get-ChildItem $aDir -Directory | ForEach-Object {
              Get-ChildItem $_.FullName -File | Move-Item -Destination $aDir -Force -ErrorAction SilentlyContinue
          }
          Remove-Item (Join-Path $aDir 'whisper.zip') -Force -ErrorAction SilentlyContinue
          $wm = $catalogo.voce.varianti | Where-Object { $_.predefinita } | Select-Object -First 1
          Scarica ($catalogo.voce.url_base + $wm.file) (Join-Path $aDir $wm.file) "modello di ascolto ($($wm.qualita))"

          # espeak-ng: senza, Kokoro non ha fonemi. E' GPLv3, percio' non lo
          # ridistribuiamo: si scarica dalla sua fonte ufficiale.
          if (-not (Test-Path (Join-Path $vDir 'espeak-ng.dll'))) {
              try {
                  $er = Invoke-RestMethod 'https://api.github.com/repos/espeak-ng/espeak-ng/releases/latest' -Headers @{ 'User-Agent' = 'nova-installer' }
                  $ea = $er.assets | Where-Object { $_.name -match 'X64\.msi$' } | Select-Object -First 1
                  if ($ea) {
                      $msi = Join-Path $env:TEMP $ea.name
                      Scarica $ea.browser_download_url $msi 'espeak-ng'
                      $estratto = Join-Path $env:TEMP 'espeak_estratto'
                      Remove-Item $estratto -Recurse -Force -ErrorAction SilentlyContinue
                      Start-Process msiexec -ArgumentList '/a', "`"$msi`"", '/qn', "TARGETDIR=`"$estratto`"" -Wait
                      $dll = Get-ChildItem $estratto -Recurse -Filter 'espeak-ng.dll' -ErrorAction SilentlyContinue | Select-Object -First 1
                      if ($dll) { Copy-Item $dll.FullName $vDir -Force }
                      $dati = Get-ChildItem $estratto -Recurse -Directory -Filter 'espeak-ng-data' -ErrorAction SilentlyContinue | Select-Object -First 1
                      if ($dati) { Copy-Item $dati.FullName $vDir -Recurse -Force -ErrorAction SilentlyContinue }
                      Remove-Item $msi, $estratto -Recurse -Force -ErrorAction SilentlyContinue
                  }
              } catch { Warn "espeak-ng non scaricato: $($_.Exception.Message)" }
          }
          if (-not (Test-Path (Join-Path $vDir 'espeak-ng.dll'))) {
              $completa = $false
              Warn "Manca espeak-ng: NOVA capira' quello che dici, ma non parlera'."
              Warn "Installalo da https://github.com/espeak-ng/espeak-ng/releases e copia"
              Warn "espeak-ng.dll in $vDir"
          }
          $cfgPatch['voice'] = @{ enabled = $true; tts_engine = 'locale'; stt_engine = 'faster-whisper' }
          if ($completa) { Ok "Voce: tutto in locale." } else { Warn "Voce: solo ascolto." }
      } catch {
          Warn "Voce locale non completata: $($_.Exception.Message)"
          Warn "NOVA funziona lo stesso, solo senza voce."
      }
  }
  3 { Info "Niente voce: si attiva quando vuoi dalle impostazioni." }
}

# ------------------------------------------------------------ configurazione
Titolo "Configurazione"

Info "Rilevo runtime e modelli gia' presenti..."
Push-Location $Root
try { & $py -c "from nova.config import Config;from nova.setup_wizard import autoconfigure;c=Config.load();[print('  ',n) for n in autoconfigure(c,force=True)];c.save()" }
finally { Pop-Location }

# Le scelte fatte qui sopra vanno scritte DOPO autoconfigure, che altrimenti
# le sovrascriverebbe con quello che ha trovato in giro.
if ($cfgPatch.Count -gt 0) {
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
    Set-ItemProperty 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Run' -Name $RunName -Value "`"$shell`""
    Ok "NOVA si avviera' da sola all'accensione."
}
$ws  = New-Object -ComObject WScript.Shell
$lnk = $ws.CreateShortcut((Join-Path ([Environment]::GetFolderPath('Desktop')) 'NOVA.lnk'))
$lnk.TargetPath = $shell; $lnk.WorkingDirectory = $Root
$lnk.Description = 'NOVA - assistente digitale locale'
$lnk.Save()
Ok "Collegamento sul Desktop."

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
