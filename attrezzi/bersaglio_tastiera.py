# -*- coding: utf-8 -*-
"""Una finestra che raccoglie quello che le viene scritto dentro.

Serve a `test_tastiera.py`: provare che i tasti arrivino vuol dire avere un
bersaglio, e l'unico bersaglio accettabile e' uno che ci siamo fatti noi.
Scrivere in una finestra dell'utente per vedere se la tastiera funziona e'
il difetto stesso che quella prova esiste per impedire.

Oltre a cio' che finisce nella casella, tiene il conto degli eventi di tasto
che la finestra riceve: serve a distinguere «i tasti non sono arrivati» da
«sono arrivati e la casella non li ha presi», che sono due guasti diversi.

  python bersaglio_tastiera.py <file> <secondi>
"""
import sys
import tkinter as tk

dove, secondi = sys.argv[1], float(sys.argv[2])
r = tk.Tk()
r.title("NOVA prova tastiera")
r.geometry("560x120+120+120")
casella = tk.Entry(r, font=("Segoe UI", 14), width=48)
casella.pack(padx=12, pady=28)
casella.focus_force()
r.attributes("-topmost", True)
r.after(50, lambda: (r.lift(), casella.focus_force()))


ricevuti = []
r.bind_all("<Key>", lambda e: ricevuti.append(repr(e.char) + "/" + e.keysym))


def fine():
    try:
        with open(dove, "w", encoding="utf-8") as f:
            f.write(casella.get())
        with open(dove + ".eventi", "w", encoding="utf-8") as f:
            f.write(f"{len(ricevuti)} eventi\n" + " ".join(ricevuti[:60]))
    finally:
        r.destroy()


r.after(int(secondi * 1000), fine)
r.mainloop()
