# -*- coding: utf-8 -*-
"""I segreti che il guardiano del vault lasciava passare.

Il vault ha una proprieta' scomoda: cio' che contiene finisce nel prompt a
ogni turno. Una credenziale entrata li' si affaccia in ogni conversazione
futura, comprese quelle in cui NOVA sta leggendo una pagina web scritta da
qualcun altro. Non serve che nessuno sbagli: basta che sia memorizzata.

`test_riservatezza.py` prova che il filtro funziona. Questa prova e' un'altra
cosa: e' l'elenco di cio' che **non** funzionava, scritto per non
riprovarcisi. Nasce chiedendo al filtro esistente di giudicare venti segreti
di forma realistica invece di chiedergli se era d'accordo con se stesso — D51
applicata al modulo da cui D51 e' nata.

Cinque buchi, e uno fa piu' male degli altri:

- **`Authorization: Bearer <token>`** passava. E' *esattamente* il caso che
  D51 aveva gia' trovato e chiuso dall'altra parte, nei messaggi d'errore di
  `guasti`. Stessa forma, stesso buco, due moduli diversi: **una lezione
  imparata in un posto non si sposta da sola**. Se un difetto e' stato trovato
  in un modulo, va cercato a mano in tutti quelli che fanno la stessa domanda.
- **`seed phrase: abandon ability able ...`** passava, perche' il valore e'
  fatto di parole comuni e il controllo sulla densita' — giusto per
  distinguere «la password e' cambiata» da «la password e' Tramonto2026» — le
  lasciava passare tutte. Ed e' la cosa che non si puo' cambiare dopo: una
  seed phrase rubata svuota un portafoglio, e non c'e' un «reimposta».
- **`otp 903214`** e **`PIN 4829`** passavano: il separatore era obbligatorio,
  e li' fra chiave e valore non c'e' niente.
- **`parola d ordine`** senza apostrofo passava. Non e' un refuso: NOVA si fa
  dettare, e whisper l'apostrofo non sempre lo mette.
- E il piu' insidioso: **la prima coppia nascondeva la seconda**. In «la
  password del wifi e Tramonto2026» l'espressione trovava «password ... wifi»
  — parola comune, nessun allarme — e si fermava. `finditer` non rimediava,
  perche' le corrispondenze non si sovrappongono e la prima consuma la
  chiave. Il segreto si nascondeva mettendogli davanti una frase innocua, che
  e' quello che succede da solo quando qualcuno incolla una riga di
  configurazione intera.
"""
import sys
from pathlib import Path

RADICE = Path(__file__).resolve().parent
sys.path.insert(0, str(RADICE))
sys.stdout.reconfigure(encoding="utf-8", errors="replace")

from nova.kb.riservatezza import perche_non_si_salva      # noqa: E402

passati = 0
falliti: list[str] = []


def controlla(nome, condizione, dettaglio=""):
    global passati
    if condizione:
        passati += 1
        print(f"  [ok ] {nome}")
    else:
        falliti.append(nome + (f" — {dettaglio}" if dettaglio else ""))
        print(f"  [NO ] {nome}" + (f" — {dettaglio}" if dettaglio else ""))


#: Cose che NON devono mai entrare nel vault. Le prime cinque sezioni sono i
#: buchi veri, misurati sul filtro di prima.
NON_DEVE_ENTRARE = [
    ("il Bearer senza due punti", "Authorization: Bearer abcdef1234567890abcdef"),
    ("il Bearer nudo", "Bearer eyJhbGciOiJIUzI1NiJ9xxxxxxxxxx"),
    ("il Bearer dentro una riga di header",
     "header Authorization Bearer sk-ant-api03-xyz123456789"),
    ("la seed phrase", "seed phrase: abandon ability able about above absent"),
    ("la passphrase a parole", "la passphrase e correct horse battery staple"),
    ("l'OTP senza separatore", "otp 903214"),
    ("il PIN senza separatore", "PIN 4829"),
    ("la parola d ordine dettata a voce", "la mia parola d ordine e Zafferano!92"),
    ("il segreto dietro una frase innocua", "la password del wifi e Tramonto2026"),
    ("e dietro due frasi innocue", "nota: la password del router e Admin2026!"),
    ("la password della posta", "la password della posta e Girasole.2026"),
    # e le forme che gia' funzionavano: restano provate, perche' una cura puo'
    # rompere quello che curava
    ("la coppia con i due punti", "password: hunter2xyz"),
    ("la coppia con l'uguale", "password = P@ssw0rd!"),
    ("una chiave di servizio", "api key: sk-proj-abcdefghij1234567890"),
    ("un token GitHub", "il token e ghp_abcdefghijklmnopqrstuvwxyz123456"),
    ("una chiave AWS", "AKIA1234567890ABCDEF"),
    ("un numero di carta", "carta 4111 1111 1111 1111"),
    ("credenziali in un indirizzo", "https://utente:segreto@example.com/x"),
    ("un JWT", "token=eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiIxIn0.abc"),
    ("una chiave privata", "chiave privata: MIIEvgIBADANBgkqhkiG9w0BA"),
    ("il PIN con il verbo", "il PIN e 4829"),
    ("il client secret", "client_secret=Gx8vQ2mNp4Rt7Ys1"),
]

#: Cose che DEVONO poter entrare. Un guardiano che blocca meta' della
#: conversazione viene spento, e allora non protegge piu' niente: i falsi
#: allarmi qui costano quanto i buchi.
DEVE_ENTRARE = [
    ("un consiglio sulle password", "usa un gestore di password"),
    ("una password cambiata", "la password e cambiata ieri"),
    ("un token scaduto", "il token e scaduto"),
    ("una password dimenticata", "ho dimenticato la password"),
    ("un fatto sulla persona", "mi chiamo Giovanni e lavoro di mattina"),
    ("il gatto", "il mio gatto si chiama Ugo"),
    ("una preferenza", "preferisce le riunioni brevi"),
    ("un token scaduto, con avverbio", "il token e scaduto stamattina"),
    ("un promemoria", "ricordami di cambiare la password"),
    ("una password non ricordata", "la password non me la ricordo"),
    ("una preferenza di sicurezza", "usa sempre autenticazione a due fattori"),
    ("un PIN dimenticato", "il PIN lo ha dimenticato"),
    ("dove tiene le password", "preferisce che le password stiano in Bitwarden"),
    ("una chiave che non e' una chiave", "la chiave inglese e nel cassetto"),
    ("un segreto che non e' un segreto", "il segreto e la pazienza"),
    ("una password cambiata, con quando", "ha cambiato la password del wifi ieri sera"),
    ("un token che non e' un token", "il token del bus e scaduto la settimana scorsa"),
    ("una regola di condotta", "conserva le credenziali nel gestore, mai nei file"),
]

print(f"\n=== {len(NON_DEVE_ENTRARE)} cose che non devono entrare nel vault ===")
for nome, testo in NON_DEVE_ENTRARE:
    motivo = perche_non_si_salva(testo)
    controlla(nome, motivo is not None, "e' passato")

print(f"\n=== {len(DEVE_ENTRARE)} cose che devono poterci entrare ===")
for nome, testo in DEVE_ENTRARE:
    motivo = perche_non_si_salva(testo)
    controlla(nome, motivo is None, f"bloccato come «{motivo}»")

print("\n=== E il motivo non contiene mai il valore ===")
# Un messaggio d'errore finisce nei log, e un log che riporta la password che
# ha appena rifiutato non ha protetto niente.
for nome, testo in NON_DEVE_ENTRARE:
    motivo = perche_non_si_salva(testo) or ""
    # nessuna parola lunga del testo deve comparire nel motivo
    perse = [p for p in testo.replace(":", " ").replace("=", " ").split()
             if len(p) >= 8 and p.lower() in motivo.lower()]
    if perse:
        controlla(f"il motivo di «{nome}» non ripete il segreto", False, str(perse))
        break
else:
    passati += 1
    print("  [ok ] nessun motivo ripete il valore che ha appena rifiutato")

print(f"\n{passati} passati, {len(falliti)} falliti")
for f in falliti:
    print(f"  - {f}")
sys.exit(1 if falliti else 0)
