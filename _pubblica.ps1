$Host.UI.RawUI.WindowTitle = 'NOVA - pubblicazione su GitHub'
# Dalla cartella dello script, non da un percorso fisso: cosi' vale anche
# per chi clona il repository, e il nome utente di chi l'ha scritto non
# finisce dentro un file pubblico.
Set-Location (Split-Path -Parent $MyInvocation.MyCommand.Path)
Write-Host ""
Write-Host "  Pubblicazione di NOVA su GitHub" -ForegroundColor White
Write-Host "  -------------------------------" -ForegroundColor DarkGray
Write-Host ""
Write-Host "  Si aprira' il browser per l'accesso a GitHub." -ForegroundColor Gray
Write-Host "  Accedi come CastermustOfficial e autorizza." -ForegroundColor Gray
Write-Host ""
git push -u origin master
if ($LASTEXITCODE -eq 0) {
    Write-Host ""
    Write-Host "  Ramo pubblicato. Ora il tag (fa partire la CI)..." -ForegroundColor Cyan
    # Il tag va creato adesso e deve essere nuovo: ripubblicare uno che
    # esiste gia' non fa partire niente e sembra riuscito.
    # Il numero sta in tre posti - qui, in core/Cargo.toml e in
# nova/__init__.py - e i tre erano gia' divergenti: il codice diceva
# 0.1.0 e questo tag v0.0.2. Ora una prova li confronta
# (test_versione.py), cosi' non si pubblica piu' un numero che non
# corrisponde a cio' che c'e' dentro.
$tag = 'v0.1.1'
    if (git tag -l $tag) {
        Write-Host "  Il tag $tag esiste gia': cambialo prima di ripubblicare." -ForegroundColor Yellow
    } else {
        git tag -a $tag -m "NOVA $tag"
        git push origin $tag
    }
}
Write-Host ""
if ($LASTEXITCODE -eq 0) {
    Write-Host "  FATTO: https://github.com/CastermustOfficial/NOVA" -ForegroundColor Green
} else {
    Write-Host "  Qualcosa non ha funzionato: leggi l'errore qui sopra." -ForegroundColor Yellow
}
Write-Host ""
Write-Host "  Premi un tasto per chiudere." -ForegroundColor DarkGray
$null = $Host.UI.RawUI.ReadKey('NoEcho,IncludeKeyDown')