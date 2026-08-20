# Prova rapida del demone: capacita', policy, eventi, supervisione.
# Uso:  powershell -ExecutionPolicy Bypass -File smoke.ps1
$ErrorActionPreference = 'Continue'
$nova = Join-Path $PSScriptRoot 'target\debug\nova.exe'
if (-not (Test-Path $nova)) { throw "nova.exe non compilato: esegui  x build" }

function Titolo($t) { Write-Host "`n=== $t ===" -ForegroundColor Cyan }

Titolo 'stato'
& $nova status

Titolo 'shell.exec (deve funzionare)'
& $nova call shell.exec command='Get-Date -Format o'

Titolo 'policy: comando vietato (deve essere BLOCCATO)'
& $nova call shell.exec command='diskpart /s script.txt'

Titolo 'policy: scrittura in percorso protetto (deve essere BLOCCATA)'
& $nova call fs.write path='C:\Windows\nova_prova.txt' content='x'

Titolo 'fs.write in cartella consentita (deve funzionare)'
$tmp = Join-Path $env:TEMP 'nova_core_prova.txt'
& $nova call fs.write path=$tmp content='scritto dal demone'
Get-Content $tmp

Titolo 'supervisione + eventi'
$job = Start-Job -ScriptBlock { param($n) & $n watch 'proc.*' } -ArgumentList $nova
Start-Sleep 1
& $nova call proc.spawn name=eco program=cmd 'args=["/c","echo ciao dal figlio"]'
Start-Sleep 3
& $nova call proc.list
Write-Host '--- eventi osservati ---' -ForegroundColor DarkGray
Receive-Job $job
Stop-Job $job; Remove-Job $job -Force

Write-Host "`nfatto." -ForegroundColor Green
