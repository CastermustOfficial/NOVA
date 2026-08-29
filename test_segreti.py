"""La porta del vault non lascia entrare le credenziali.

NOVA ha rifiutato a voce di memorizzare mail e password, e ha spiegato bene
perche': cio' che entra nel vault viene riletto in ogni conversazione futura,
comprese quelle in cui sta leggendo testo scritto da altri. Quel ragionamento
era giusto — ma era un ragionamento, e un ragionamento vale finche' regge.

L'esposizione vera non e' nemmeno quella che ha rifiutato: e' l'apprendimento
automatico, che gira a ogni turno e scrive fatti da solo. Se in una
conversazione a voce salta fuori una password, nessuno decide di salvarla:
succede.

Quindi il controllo sta sulla porta, non nel giudizio.
"""
from __future__ import annotations

import sys
import tempfile

sys.path.insert(0, ".")
sys.stdout.reconfigure(encoding="utf-8", errors="replace")

from nova.kb.riservatezza import perche_non_si_salva
from nova.kb.schema import Node
from nova.kb.store import Vault

esiti: list[tuple[str, bool]] = []


def controlla(nome: str, cond: bool, dettaglio: str = "") -> None:
    esiti.append((nome, bool(cond)))
    print(f"  [{'ok ' if cond else 'NO '}] {nome}" + (f"  {dettaglio}" if dettaglio else ""))


print("\n1. le forme che sono un segreto per costruzione")
per_forma = [
    # Inventata qui: il marcatore deve stare sulla riga della chiave, perche'
    # e' quella che il controllo sulle fughe legge.
    ("chiave di servizio",
     "api_key: sk_30a2c8c84a4612a7d6acefb3a32db0008b52"),  # chiave-finta
    ("token GitHub", "il token e' ghp_ABCDEFGHIJKLMNOPQRSTUVWXYZ012345"),
    ("token Slack", "xoxb-1234567890-abcdefghij"),
    ("chiave AWS", "AKIAIOSFODNN7EXAMPLE"),
    ("chiave privata", "-----BEGIN RSA PRIVATE KEY-----"),
    ("JWT", "eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiIxMjM0NTY3ODkwIn0.abc"),
    ("credenziali nell'URL", "https://mario:segreta99@server.local/api"),
    ("numero di carta", "la carta e' 4539 1488 0343 6467"),
]
for nome, testo in per_forma:
    controlla(f"riconosce {nome}", perche_non_si_salva(testo) is not None)

print("\n2. la forma «chiave valore», come la si dice a voce")
detto = [
    "La password del wifi e Tramonto2026!",
    "La password del wifi e' Hunter2!x9",
    "la password è Tramonto2026!",
    "il pin del bancomat e 4471",
    "il codice otp era 839201",
    "password = Correct-Horse-Battery-Staple",
]
for testo in detto:
    controlla(f"blocca: {testo[:38]}", perche_non_si_salva(testo) is not None)

print("\n3. le frasi che parlano di credenziali senza esserlo")
innocue = [
    "Giovanni preferisce usare un gestore di password invece di ricordarle.",
    "Mi ha chiesto di non salvare credenziali in memoria.",
    "La password gliela chiedo al momento, non la salvo.",
    "La password è cambiata la settimana scorsa.",
    "Il token di sessione scade dopo sei ore.",
    "La password e utente non coincidono mai.",
    "Il pin lo sa solo lui.",
    "Il progetto NOVA sta in C:/Users/giova/NOVA",
]
for testo in innocue:
    motivo = perche_non_si_salva(testo)
    controlla(f"passa: {testo[:38]}", motivo is None, motivo or "")

print("\n4. il rifiuto arriva alla porta, non solo alla funzione")
v = Vault(tempfile.mkdtemp(prefix="nova-segreti-"))
try:
    v.upsert(Node(slug="", title="Wifi di casa", body="La password del wifi e Tramonto2026!"))
    controlla("upsert rifiuta il nodo con la credenziale", False, "e' passato!")
except ValueError as e:
    controlla("upsert rifiuta il nodo con la credenziale", True)
    controlla("il rifiuto NON ripete il segreto", "Tramonto2026" not in str(e), str(e)[:44] + "...")
    controlla("il rifiuto dice cosa fare invece", "chiedila al momento" in str(e))

salvato = v.upsert(Node(slug="", title="Preferenza", body="Usa un gestore di password."))
controlla("un nodo innocuo entra lo stesso", salvato.slug != "")

print("\n5. il titolo conta quanto il corpo")
try:
    v.upsert(Node(slug="", title="password: Tramonto2026!", body="niente di che"))
    controlla("guarda anche il titolo", False, "e' passato!")
except ValueError:
    controlla("guarda anche il titolo", True)

passati = sum(1 for _, c in esiti if c)
print(f"\n{passati}/{len(esiti)} passati")
raise SystemExit(0 if passati == len(esiti) else 1)
