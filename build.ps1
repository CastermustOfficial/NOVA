<#
  Compila il core Rust di NOVA.
    .\build.ps1              build di release
    .\build.ps1 -Debug       build di sviluppo
    .\build.ps1 -Test        esegue i test
    .\build.ps1 -Controlla   solo controllo di tipi ed errori, senza produrre binari

  Trova da solo la toolchain: rustup nel PATH, oppure quella installata
  nella home. Su Windows serve anche il linker MSVC (Visual Studio Build
  Tools con «Desktop development with C++»).
#>
param([switch]$Debug, [switch]$Test, [switch]$Controlla)
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

# --------------------------------------------------------------------------
# Se NOVA e' aperta, la compilazione fallira' a meta': `cargo` non puo'
# sovrascrivere un file in esecuzione, e Windows glielo nega. Il messaggio che
# ne esce e' «failed to remove file ... Accesso negato. (os error 5)» seguito
# da un traceback di PowerShell - cioe' il nome di un errore, non un
# messaggio, dopo un minuto e mezzo di attesa.
#
# Meglio guardare prima: costa niente, e la frase la si puo' dire in italiano.
#
# Non vale per `-Controlla`: `cargo check` fa tutto il lavoro del compilatore
# **tranne** scrivere i binari, quindi con NOVA aperta funziona benissimo - ed
# e' proprio quello che serve a chi vuole sapere se il codice sta in piedi
# senza chiudere l'assistente che sta usando. Fermarlo sarebbe togliere
# l'unica cosa che si poteva ancora fare.
$aperti = if ($Controlla) { @() } else { @(Get-Process -Name (
    $(if (Test-Path (Join-Path $Core 'binari.json')) {
        try {
            (Get-Content (Join-Path $Core 'binari.json') -Raw -Encoding UTF8 |
                ConvertFrom-Json).eseguibili | ForEach-Object { $_.nome }
        } catch { @('novad', 'nova-shell') }
    } else { @('novad', 'nova-shell') })
) -ErrorAction SilentlyContinue) }
if ($aperti) {
    $nomi = ($aperti | Select-Object -ExpandProperty Name -Unique) -join ', '
    Write-Host "[nova] NOVA e' aperta ($nomi)." -ForegroundColor Yellow
    Write-Host "[nova] Windows non lascia riscrivere un programma mentre gira," -ForegroundColor Yellow
    Write-Host "[nova] quindi la compilazione fallirebbe dopo un minuto." -ForegroundColor Yellow
    Write-Host "[nova] Chiudi NOVA dall'orb e ridai .\build.ps1" -ForegroundColor Yellow
    exit 1
}

Push-Location $Core
try {
    # Chiamate esplicite invece dello splatting: con @array PowerShell puo'
    # far arrivare a cargo un «-» isolato, e l'errore che ne esce («unexpected
    # argument») non somiglia per niente alla causa.
    # `check` fa tutto il lavoro del compilatore tranne scrivere i binari:
    # per sapere se il codice sta in piedi costa una frazione di un build
    # di release, e vale la pena poterlo chiedere.
    $azione = if ($Controlla) { 'check' } elseif ($Test) { 'test' } else { 'build' }
    Write-Host "[nova] cargo $azione$(if (-not $Debug -and -not $Controlla) { ' --release' })" -ForegroundColor Cyan
    # cargo racconta l'avanzamento su stderr, non solo gli errori. Con
    # ErrorActionPreference a Stop — che e' quello che usa install.ps1 —
    # PowerShell scambia la prima riga di avanzamento per un errore fatale e
    # interrompe una compilazione perfettamente sana. Per un comando esterno
    # l'unico giudice e' il codice di uscita.
    $primaEAP = $ErrorActionPreference
    $ErrorActionPreference = 'Continue'
    try {
        if ($Debug -or $Controlla) { & $cargo $azione } else { & $cargo $azione '--release' }
        $codice = $LASTEXITCODE
    } finally { $ErrorActionPreference = $primaEAP }
    if ($codice -ne 0) { throw "compilazione fallita (codice $codice)" }
} finally { Pop-Location }

# --------------------------------------------------------------------------
# I binari appena costruiti vanno in bin/, che e' dove l'installatore li mette
# e dove l'avvio automatico va a cercarli.
#
# Senza questo passo la macchina di chi sviluppa fa girare
# «core\target\release\nova-shell.exe» e quella di chiunque altro
# «bin\nova-shell.exe»: due file diversi che si disallineano in silenzio, e
# il secondo e' l'unico che un utente vedra' mai. E' la forma piu' pura di «da
# me funziona» - l'unica macchina su cui NOVA e' provata sta provando
# qualcos'altro.
#
# Non si fa dopo `-Test` ne' `-Controlla`: quelli non producono binari, e
# copiare quelli vecchi facendo finta di aver pubblicato sarebbe peggio di
# non copiare niente.
if (-not $Test -and -not $Controlla) {
    $profilo = if ($Debug) { 'debug' } else { 'release' }
    $da  = Join-Path $Core "target\$profilo"
    $bin = Join-Path (Split-Path -Parent $MyInvocation.MyCommand.Path) 'bin'

    # L'elenco sta in core/binari.json: una copia scritta a mano qui sarebbe
    # la quarta, e le prime tre si erano gia' disallineate.
    $nomi = @()
    $fileBinari = Join-Path $Core 'binari.json'
    if (Test-Path $fileBinari) {
        try {
            $el = Get-Content $fileBinari -Raw -Encoding UTF8 | ConvertFrom-Json
            $nomi = @($el.eseguibili | ForEach-Object { "$($_.nome).exe" })
        } catch {
            Write-Host "[nova] core\binari.json non e' leggibile: non pubblico." -ForegroundColor Yellow
        }
    } else {
        Write-Host "[nova] core\binari.json non c'e': non pubblico in bin\." -ForegroundColor Yellow
    }

    if ($nomi.Count) {
        New-Item -ItemType Directory -Force -Path $bin | Out-Null
        $messi = 0; $saltati = @()
        foreach ($n in $nomi) {
            $src = Join-Path $da $n
            if (-not (Test-Path $src)) { $saltati += $n; continue }
            try {
                Copy-Item $src (Join-Path $bin $n) -Force
                $messi++
            } catch {
                # cargo non puo' sovrascrivere un binario in esecuzione, e
                # nemmeno noi: se l'orb e' aperto, questa copia fallisce. Va
                # detto, perche' altrimenti si continua a provare una
                # modifica che sul disco non e' mai arrivata.
                $saltati += "$n (in uso?)"
            }
        }
        Write-Host "[nova] pubblicati in bin\: $messi su $($nomi.Count)" -ForegroundColor Cyan
        if ($saltati.Count) {
            Write-Host "[nova] non copiati: $($saltati -join ', ')" -ForegroundColor Yellow
            Write-Host "[nova] se NOVA e' aperta, chiudila e ridai build." -ForegroundColor Yellow
        }
    }
}

Write-Host "[nova] fatto." -ForegroundColor Green