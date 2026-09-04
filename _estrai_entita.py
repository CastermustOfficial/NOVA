# -*- coding: utf-8 -*-
"""La tabella delle entita' HTML, estratta invece che ricopiata.

Sono duemiladuecento nomi. Ricopiarne anche solo i duecento piu' comuni a mano
vorrebbe dire duecento occasioni di sbagliare un carattere che finisce nel
contesto del modello — e la stessa ragione per cui le dichiarazioni degli
strumenti sono estratte e non ricopiate (D112).

Il banco aveva mostrato il buco su otto entita' (`&oelig;`, `&thorn;`,
`&notin;`...) messe nel corpus **apposta** per vedere dove finiva la tabella
scritta a mano. Vedere e' meglio che scoprire.

Scrive `entita.txt`, da incollare in
`core/crates/nova-strumenti/src/entita.rs`.
"""
import html.entities
import io

# Python tiene sia `amp` sia `amp;`, e la stragrande maggioranza dei nomi
# **solo** con il punto e virgola: la prima estrazione ne aveva presi 106 su
# duemiladuecento, e sembrava un numero plausibile. Il punto e virgola lo
# gestisce il lettore, quindi qui si toglie e si tolgono i doppioni.
senza = {}
for nome, valore in html.entities.html5.items():
    senza.setdefault(nome.rstrip(";"), valore)

voci = []
for nome, valore in sorted(senza.items()):
    v = valore.replace("\\", "\\\\").replace('"', '\\"')
    v = "".join(c if 32 <= ord(c) < 127 else "\\u{%x}" % ord(c) for c in v)
    voci.append('    ("%s", "%s"),' % (nome, v))

io.open("entita.txt", "w", encoding="utf-8", newline="\n").write("\n".join(voci))
print("entita estratte:", len(voci))
