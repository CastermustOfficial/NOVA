# -*- coding: utf-8 -*-
"""Il confine vale anche **dopo** che il comando e' partito.

Le guardie di NOVA hanno sempre vissuto dentro il processo che decide:
`check_write` confronta percorsi, `comando_permesso` applica espressioni
regolari. Servono a dire di no **prima**. Dopo non servono a niente: quel che
passa il controllo gira con tutti i privilegi di chi l'ha lanciato, e un
comando che la regola non ha riconosciuto puo' scrivere dove arriva l'utente.

Questa prova guarda l'altra meta'. Il demone esegue un comando che prova a
scrivere in due posti — uno dichiarato in `write_roots`, uno no — e pretende
che il secondo **non ci riesca**, con il rifiuto che arriva dal kernel e non
da una frase nostra.

Su Linux il recinto lo tiene Landlock, su Windows un contenitore
(AppContainer, D367). Dove non c'e' — un kernel vecchio — la prova si dichiara
saltata invece di passare per finta: un confine che si crede di avere e non si
ha e' peggio di un confine che manca, e una prova verde che non ha provato
niente e' esattamente quel modo di crederci.

Esce 2 se il demone non e' costruito o se qui il recinto non esiste.
"""
import json
import os
import subprocess
import sys
import tempfile
import time
from pathlib import Path

RADICE = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(RADICE))
sys.stdout.reconfigure(encoding="utf-8", errors="replace")

NOME = "novad.exe" if os.name == "nt" else "novad"
DEMONE = next((p for p in (RADICE / "core" / "target" / "release" / NOME,
                           RADICE / "core" / "target" / "debug" / NOME)
               if p.is_file()), None)
if DEMONE is None:
    print("Il demone non e' costruito per questo sistema. Per averlo:")
    print("  cd core && cargo build --release --bin novad")
    sys.exit(2)

passati = 0
falliti: list[str] = []


def controlla(nome, condizione, dettaglio=""):
    global passati
    if condizione:
        passati += 1
        print(f"  [ok ] {nome}")
    else:
        falliti.append(nome)
        print(f"  [NO ] {nome}  {dettaglio}")


from nova.core_client import CoreClient                           # noqa: E402

casa = tempfile.mkdtemp(prefix="nova-recinto-")
lavoro = Path(casa) / "lavoro"
altrove = Path(casa) / "altrove"
lavoro.mkdir(parents=True)
altrove.mkdir(parents=True)
# Una radice mia sul disco di sistema: la sua prima cartella sotto la radice la
# si puo' preparare, a differenza di `C:\Users`. Serve a provare la cartella di
# lavoro dove il meccanismo puo' funzionare.
lavoro2 = (Path(os.environ.get("SystemDrive", "C:") + "\\")
           / f"nova-prova-e2e-{os.getpid()}" / "lavoro2") if os.name == "nt" else None
if lavoro2 is not None:
    lavoro2.mkdir(parents=True, exist_ok=True)
segreto = altrove / "segreto.txt"
da_cancellare = altrove / "da_cancellare.txt"
segreto.write_text("non si legge", encoding="utf-8")
da_cancellare.write_text("resto", encoding="utf-8")

(Path(casa) / "NOVA").mkdir(parents=True, exist_ok=True)
# La memoria spenta: questa prova non la usa, e il demone acceso la
# creerebbe e la seminerebbe nel vault del progetto (D365).
(Path(casa) / "NOVA" / "config.json").write_text('{"kb": {"enabled": false}}', encoding="utf-8")
(Path(casa) / "NOVA" / "core.json").write_text(json.dumps({
    "write_roots": [str(lavoro)] + ([str(lavoro2)] if lavoro2 is not None else []),
    "autonomy": "autonomous",
    "log_level": "warn",
}, ensure_ascii=False), encoding="utf-8")

endpoint = (rf"\\.\pipe\nova-recinto-{os.getpid()}" if os.name == "nt"
            else str(Path(casa) / "nova.sock"))
ambiente = dict(os.environ)
ambiente["APPDATA"] = casa
ambiente["HOME"] = casa
ambiente["XDG_CONFIG_HOME"] = casa
ambiente["XDG_RUNTIME_DIR"] = casa
# Il controllo dei dischi parte un minuto dopo l'avvio; qui parte subito, per
# vedere cosa succede se il demone si chiude mentre sta ancora percorrendoli.
ambiente["NOVA_RECINTO_CONTROLLO_RITARDO_S"] = "0"

processo = subprocess.Popen(
    [str(DEMONE), "--endpoint", endpoint, "--log", "warn"],
    env=ambiente, stdout=subprocess.DEVNULL, stderr=subprocess.PIPE)

try:
    scadenza = time.time() + 20
    while time.time() < scadenza and processo.poll() is None:
        if CoreClient.disponibile(endpoint):
            break
        time.sleep(0.3)
    else:
        fine = (processo.stderr.read() or b"").decode("utf-8", "replace")[-400:]
        print(f"  il demone non ha risposto: {fine}")
        processo.kill()
        sys.exit(2)

    dentro = lavoro / "scritto.txt"
    fuori = altrove / "scritto.txt"
    if os.name == "nt":
        comando = (f"Set-Content -Path '{dentro}' -Value 'dentro'; "
                   f"Set-Content -Path '{fuori}' -Value 'fuori'")
    else:
        comando = f"echo dentro > '{dentro}'; echo fuori > '{fuori}'; true"

    with CoreClient(endpoint, timeout=60) as c:
        r = c.call("shell.exec", {"command": comando})
        # Un comando che si appoggia a una cartella temporanea: dentro il
        # recinto deve averne una sua, o meta' dei programmi non parte.
        if os.name == "nt":
            appoggio = "New-Item -ItemType File -Path (Join-Path $env:TEMP 'x.txt') -Force"
        else:
            appoggio = 'echo x > "$TMPDIR/x.txt" && echo fatto'
        r2 = c.call("shell.exec", {"command": appoggio})
        r3 = r4 = r5 = None
        if os.name == "nt":
            # Fuori dal recinto il resto del profilo non si legge e non si
            # cancella: nel token ristretto di prima si leggeva tutto.
            r3 = c.call("shell.exec", {"command": (
                f"try {{ [IO.File]::ReadAllText('{segreto}') | Out-Null; 'LEGGE' }} catch {{ 'NON-LEGGE' }}; "
                f"try {{ Remove-Item -LiteralPath '{da_cancellare}' -ErrorAction Stop; 'CANCELLA' }} "
                f"catch {{ 'NON-CANCELLA' }}")})
            # E la cartella di lavoro chiesta e' quella: PowerShell si
            # posiziona solo se il contenitore legge le antenate.
            r4 = c.call("shell.exec", {"command": "Set-Content -Path rel.txt -Value x -ErrorAction SilentlyContinue; (Get-Location).Path",
                                       "cwd": str(lavoro)})
            r5 = c.call("shell.exec", {"command": "Set-Content -Path rel2.txt -Value x; (Get-Location).Path",
                                       "cwd": str(lavoro2)})

    racconto = (r.get("recinto") or "")
    print(f"\n  il demone dice: {racconto}")
    # «chiuso dal kernel» su Linux, «chiuso da Windows» su Windows: dove il
    # racconto non dice nessuno dei due, il recinto di sistema non c'e'.
    if "chiuso dal kernel" not in racconto and "chiuso da Windows" not in racconto:
        print("  qui il recinto di sistema non c'e': non provo un confine che non esiste")
        raise SystemExit(2)

    print("\n0. il racconto dice il vero")
    # Le cartelle dichiarate, non la temporanea del comando: su Windows sono due
    # (`lavoro` e la radice mia), altrove una.
    atteso = "2 cartelle dichiarate" if os.name == "nt" else "1 cartella dichiarata"
    controlla("conta le cartelle dichiarate, non la temporanea", atteso in racconto, racconto)
    if os.name == "nt":
        controlla("dice che il recinto lo tiene Windows",
                  "chiuso da Windows" in racconto, racconto)
        controlla("dice cosa il comando non vede: il resto del profilo e il loopback",
                  "non il resto del profilo" in racconto and "loopback" in racconto, racconto)
        controlla("e com'e' la rete", "rete accesa" in racconto, racconto)
        controlla("e dice cosa si sa delle cartelle di terzi che il contenitore puo' scrivere",
                  "controllo delle cartelle di terzi" in racconto, racconto)

    print("\n1. il confine vale dopo, non solo prima")
    controlla("dentro le cartelle dichiarate si scrive", dentro.is_file(),
              f"stderr: {str(r.get('stderr'))[:150]}")
    controlla("fuori NO", not fuori.exists(),
              "ha scritto lo stesso: il recinto non tiene")
    controlla("e il rifiuto lo dice il sistema, non noi",
              "denied" in str(r.get("stderr", "")).lower()
              or "negato" in str(r.get("stderr", "")).lower(),
              str(r.get("stderr"))[:200])

    if os.name == "nt":
        print("\n1b. il resto del profilo non si legge ne' si cancella, e la cartella di lavoro e' quella")
        o3 = str(r3.get("stdout", ""))
        controlla("un file fuori dal recinto non si legge", "NON-LEGGE" in o3, o3[:200])
        controlla("e non si cancella", "NON-CANCELLA" in o3 and da_cancellare.is_file(), o3[:200])
        # Sotto una radice che si puo' preparare, la cartella di lavoro e' quella.
        controlla("la cartella di lavoro chiesta e' quella, dove il contenitore puo' leggere la prima componente",
                  (lavoro2 / "rel2.txt").is_file(), json.dumps(r5, ensure_ascii=False)[:300])
        # Sotto `C:\Users` dipende da una cosa sola: se `novad --recinto
        # --prepara` ha aperto `C:\Users` da amministratore. Se si', il comando
        # si posiziona; se no, parte altrove, **non scrive altrove**, e la
        # risposta lo racconta. In entrambi i casi la prova deve poter passare.
        posizionato = (lavoro / "rel.txt").is_file()
        avvisato = any("non puo' leggere" in a for a in r4.get("avvisi", []))
        controlla("nel profilo: o si posiziona, o lo dice negli avvisi",
                  posizionato or avvisato, json.dumps(r4, ensure_ascii=False)[:300])
        controlla("e se non si posiziona non scrive altrove",
                  posizionato or not Path("C:/rel.txt").exists(),
                  json.dumps(r4, ensure_ascii=False)[:300])
        stato = "aperta" if posizionato else "chiusa"
        print(f"     (nel profilo: {'posizionato' if posizionato else 'non posizionato'}, C:\\Users e' {stato})")

    print("\n2. e un comando confinato riesce comunque a lavorare")
    controlla("ha un posto dove appoggiare un file",
              r2.get("code") == 0, json.dumps(r2, ensure_ascii=False)[:200])
    if os.name == "nt":
        # Windows impone al contenitore la sua temporanea, una sola e
        # condivisa: non muore col comando, quindi la svuota chi l'ha usata.
        temp_contenitore = Path(os.environ["LOCALAPPDATA"]) / "Packages" / "nova.recinto" / "AC" / "Temp"
        controlla("e la temporanea del contenitore non resta piena",
                  not temp_contenitore.exists() or not any(temp_contenitore.iterdir()),
                  str(list(temp_contenitore.iterdir())[:3]) if temp_contenitore.exists() else "")
    else:
        controlla("che pero' non resta sul disco",
                  not any(p.name.startswith("nova-comando-")
                          for p in Path(tempfile.gettempdir()).glob("nova-comando-*")
                          if p.is_dir() and any(p.iterdir())),
                  "una cartella effimera piena e' un residuo di cui nessuno sa piu' niente")

    print("\n3. e quel che e' successo resta scritto")
    with CoreClient(endpoint, timeout=30) as c:
        azioni = c.call("registro.cerca", {"testo": "comando"})
    controlla("il comando e' nel registro delle azioni",
              azioni.get("quante", 0) >= 2, json.dumps(azioni)[:200])

    if os.name == "nt":
        # Su Windows il recinto scrive voci sulle cartelle dell'utente e
        # registra un profilo nel sistema: devono essere annotati, e devono
        # potersi togliere come fa il disinstallatore — demone fermo, poi
        # `novad --recinto --togli` (D367).
        print("\n4. e le voci sulle cartelle, il profilo e le antenate si tolgono")
        elenco = Path(casa) / "NOVA" / "recinto.json"
        dati = json.loads(elenco.read_text(encoding="utf-8")) if elenco.is_file() else {}
        annotate = {str(Path(v["percorso"])).lower(): v for v in dati.get("cartelle", [])}
        identita = dati.get("identita", "")

        def voci(cartella):
            return subprocess.run(["icacls", str(cartella)], capture_output=True,
                                  text=True, errors="replace").stdout.lower()

        sunto = json.dumps(dati, ensure_ascii=False)[:300]
        controlla("l'elenco ricorda l'identita' del contenitore",
                  identita.startswith("S-1-15-2-"), sunto)
        controlla("e che il profilo e' registrato nel sistema", dati.get("profilo") is True, sunto)
        controlla("la cartella dichiarata e' nell'elenco, come cartella in cui si scrive",
                  "scrive" in annotate.get(str(lavoro).lower(), {}).get("generi", []), sunto)
        prima2 = str(lavoro2.parent).lower() if lavoro2 is not None else ""
        controlla("la prima cartella della radice mia come antenata",
                  "antenata" in annotate.get(prima2, {}).get("generi", []), sunto)
        # Le cartelle che PowerShell non vuole non si toccano mai: la radice del
        # profilo, `AppData` e `Local` erano quelle che la prima versione
        # preparava, e scrivere un permesso su di esse ripassava l'intero
        # profilo. Genitore e nonno della cartella di lavoro (la cartella di
        # prova e `Temp`) si preparano solo se la prima, `C:\\Users`, e' aperta.
        profilo_utente = Path(os.environ["USERPROFILE"])
        mai = {str(profilo_utente).lower(),
               str(profilo_utente / "AppData").lower(),
               str(profilo_utente / "AppData" / "Local").lower()}
        controlla("niente voci sulle cartelle del profilo che non servono",
                  not (mai & set(annotate)), sunto)
        if (lavoro / "rel.txt").is_file():
            controlla("e, con la prima aperta, il genitore della cartella di lavoro come antenata",
                      "antenata" in annotate.get(str(Path(casa)).lower(), {}).get("generi", []), sunto)
        controlla("nessuna cartella temporanea per comando",
                  not any("nova-comando-" in a for a in annotate), sunto)
        controlla("la cartella dichiarata porta la voce del contenitore",
                  bool(identita) and identita.lower() in voci(lavoro), voci(lavoro)[:300])
        controlla("la prima cartella porta la voce del contenitore",
                  bool(identita) and identita.lower() in voci(lavoro2.parent), voci(lavoro2.parent)[:300])
        antenate = [Path(v["percorso"]) for v in dati.get("cartelle", [])
                    if "antenata" in v.get("generi", [])]

        with CoreClient(endpoint, timeout=5) as c:
            c.request("daemon/shutdown")
        chiuso = time.time()
        try:
            processo.wait(timeout=10)
            spento = True
        except subprocess.TimeoutExpired:
            spento = False
            processo.kill()
        controlla("il demone si spegne in pochi secondi anche con il controllo dei dischi in corso",
                  spento, "non e' uscito entro 10 s dallo spegnimento")
        if spento:
            print(f"     (spento in {time.time() - chiuso:.1f} s)")
        controlla("e un controllo fermato a meta' non scrive un rapporto che non ha finito",
                  not (Path(casa) / "NOVA" / "recinto-controllo.json").exists())
        tolto = subprocess.run([str(DEMONE), "--recinto", "--togli"], env=ambiente,
                               capture_output=True, text=True, errors="replace")
        controlla("novad --recinto --togli riesce", tolto.returncode == 0,
                  (tolto.stderr or tolto.stdout)[:200])
        controlla("la voce non c'e' piu' sulla cartella dichiarata",
                  bool(identita) and identita.lower() not in voci(lavoro), voci(lavoro)[:300])
        rimaste = [str(a) for a in antenate if identita.lower() in voci(a)]
        controlla("ne' su nessuna antenata, comprese quelle vere del profilo", not rimaste, str(rimaste))
        controlla("ne' l'elenco", not elenco.exists())
        controlla("ne' sulla prima cartella della radice mia",
                  bool(identita) and identita.lower() not in voci(lavoro2.parent), voci(lavoro2.parent)[:300])
        profilo = Path(os.environ["LOCALAPPDATA"]) / "Packages" / "nova.recinto"
        controlla("ne' il profilo del contenitore", not profilo.exists(), str(profilo))
finally:
    try:
        with CoreClient(endpoint, timeout=5) as c:
            c.request("daemon/shutdown")
    except Exception:                                            # noqa: BLE001
        pass
    try:
        processo.wait(timeout=10)
    except subprocess.TimeoutExpired:
        processo.kill()
    import shutil
    shutil.rmtree(casa, ignore_errors=True)
    if lavoro2 is not None:
        shutil.rmtree(lavoro2.parent, ignore_errors=True)

print(f"\n{passati} passati, {len(falliti)} falliti")
for f in falliti:
    print(f"  - {f}")
sys.exit(1 if falliti else 0)
