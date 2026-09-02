//! Il calendario come lo intendono le persone, non come lo intende un fuso.
//!
//! Qui dentro non c'e' un orologio. Non si chiede mai «che ore sono»: l'ora
//! si passa da fuori, e questo rende ogni cosa provabile senza aspettare
//! domani — la stessa scelta gia' fatta in `nova-registro::giorno`, dove
//! `oggi` e' un argomento.
//!
//! Perche' e' un pezzo suo invece di stare dentro chi lo usa: `giorni_del_mese`
//! esisteva gia', privato, dentro `nova-registro`, e serviva di nuovo qui. Una
//! copia scritta a mano di cio' di cui una cosa e' fatta si disallinea sempre,
//! e questo progetto l'ha gia' imparato tre volte — l'elenco dei binari, le
//! cartelle sincronizzate, i posti dei dati. Alla seconda occorrenza la si
//! mette in comune, non alla quarta.
//!
//! Niente fusi orari, di proposito. Un fuso e' una domanda di piattaforma, e
//! le domande di piattaforma stanno in `nova-platform`. Qui si fa la parte che
//! non cambia mai: quanti giorni ha febbraio, che giorno della settimana e'
//! il tre marzo, cosa viene dopo il 31 dicembre.

/// Il calendario gregoriano, non «divisibile per quattro»: il 1900 non era
/// bisestile e il 2000 si'. E' la regola che sbagliano quasi tutti i calendari
/// scritti in fretta, e sbagliarla si vede solo una volta ogni cent'anni —
/// cioe' esattamente quando non c'e' piu' nessuno che ricordi perche'.
pub fn bisestile(anno: i32) -> bool {
    (anno % 4 == 0 && anno % 100 != 0) || anno % 400 == 0
}

/// Quanti giorni ha quel mese di quell'anno.
///
/// Un mese fuori da 1..=12 torna 30: e' la stessa scelta prudente che aveva
/// `nova-registro`, e vale perche' chi chiama sta gia' sbagliando e non deve
/// per questo trovarsi un panico.
pub fn giorni_del_mese(anno: i32, mese: u32) -> u32 {
    match mese {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if bisestile(anno) => 29,
        2 => 28,
        _ => 30,
    }
}

/// I giorni dal 1970-01-01, in aritmetica civile.
///
/// E' l'algoritmo «days from civil» di Howard Hinnant: sposta l'inizio
/// dell'anno a marzo, cosi' il giorno bisestile finisce in fondo e sparisce
/// il caso speciale di febbraio. Vale per qualunque data del calendario
/// proplettico gregoriano, anche prima del 1970 (torna negativo).
pub fn giorni_dal_1970(anno: i32, mese: u32, giorno: u32) -> i64 {
    let a = anno - if mese <= 2 { 1 } else { 0 };
    let era = if a >= 0 { a } else { a - 399 } / 400;
    let anno_di_era = (a - era * 400) as i64; // 0..=399
    let m = mese as i64;
    let d = giorno as i64;
    let giorno_di_anno = (153 * (m + if m > 2 { -3 } else { 9 }) + 2) / 5 + d - 1;
    let giorno_di_era =
        anno_di_era * 365 + anno_di_era / 4 - anno_di_era / 100 + giorno_di_anno;
    era as i64 * 146097 + giorno_di_era - 719468
}

/// Il giorno della settimana, con **lunedi' = 0**.
///
/// Lunedi' zero e non domenica zero perche' e' cio' che usa Python
/// (`datetime.weekday()`), e questo pezzo nasce per rispondere identico a
/// `pianificazione.py`. Una convenzione diversa qui sarebbe una differenza
/// che non si vede finche' non e' domenica.
pub fn giorno_settimana(anno: i32, mese: u32, giorno: u32) -> u32 {
    // Il 1970-01-01 era un giovedi', cioe' 3 con lunedi' a zero.
    let g = giorni_dal_1970(anno, mese, giorno) + 3;
    g.rem_euclid(7) as u32
}

/// Una data e un'ora **senza fuso**: cio' che l'utente legge sull'orologio.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct DataOra {
    pub anno: i32,
    pub mese: u32,
    pub giorno: u32,
    pub ora: u32,
    pub minuto: u32,
    pub secondo: u32,
}

impl DataOra {
    pub fn nuova(anno: i32, mese: u32, giorno: u32, ora: u32, minuto: u32, secondo: u32) -> Self {
        Self { anno, mese, giorno, ora, minuto, secondo }
    }

    /// `2026-09-02T08:30:00`. Niente fuso in coda, perche' non ce n'e' uno.
    pub fn iso(&self) -> String {
        format!(
            "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}",
            self.anno, self.mese, self.giorno, self.ora, self.minuto, self.secondo
        )
    }

    /// Legge `2026-09-02T08:30:00`, e accetta anche lo spazio al posto della T
    /// e i secondi mancanti: chi scrive a mano una data la scrive cosi'.
    pub fn da_iso(s: &str) -> Option<Self> {
        let s = s.trim();
        let (data, ora) = s.split_once(['T', ' '])?;
        let d: Vec<&str> = data.split('-').collect();
        if d.len() != 3 {
            return None;
        }
        let o: Vec<&str> = ora.split(':').collect();
        if o.len() < 2 {
            return None;
        }
        Some(Self {
            anno: d[0].parse().ok()?,
            mese: d[1].parse().ok()?,
            giorno: d[2].parse().ok()?,
            ora: o[0].parse().ok()?,
            minuto: o[1].parse().ok()?,
            secondo: if o.len() > 2 { o[2].parse().ok()? } else { 0 },
        })
    }

    pub fn giorno_settimana(&self) -> u32 {
        giorno_settimana(self.anno, self.mese, self.giorno)
    }

    /// Azzera secondi e mette ora e minuto: il `replace(...)` di Python.
    pub fn con_orario(&self, ora: u32, minuto: u32) -> Self {
        Self { ora, minuto, secondo: 0, ..*self }
    }

    /// Somma giorni, con il riporto sui mesi e sugli anni.
    pub fn piu_giorni(&self, quanti: i64) -> Self {
        let mut d = *self;
        let mut resto = quanti;
        while resto > 0 {
            let nel_mese = giorni_del_mese(d.anno, d.mese) as i64;
            if d.giorno as i64 + resto <= nel_mese {
                d.giorno += resto as u32;
                return d;
            }
            resto -= nel_mese - d.giorno as i64 + 1;
            d.giorno = 1;
            if d.mese == 12 {
                d.mese = 1;
                d.anno += 1;
            } else {
                d.mese += 1;
            }
        }
        while resto < 0 {
            if d.giorno as i64 + resto >= 1 {
                d.giorno = (d.giorno as i64 + resto) as u32;
                return d;
            }
            resto += d.giorno as i64;
            if d.mese == 1 {
                d.mese = 12;
                d.anno -= 1;
            } else {
                d.mese -= 1;
            }
            d.giorno = giorni_del_mese(d.anno, d.mese);
        }
        d
    }

    /// Somma minuti, con il riporto su ore e giorni.
    pub fn piu_minuti(&self, quanti: i64) -> Self {
        let totale = self.ora as i64 * 60 + self.minuto as i64 + quanti;
        let giorni = totale.div_euclid(1440);
        let nel_giorno = totale.rem_euclid(1440);
        let mut d = self.piu_giorni(giorni);
        d.ora = (nel_giorno / 60) as u32;
        d.minuto = (nel_giorno % 60) as u32;
        d
    }
}

#[cfg(test)]
mod prove {
    use super::*;

    #[test]
    fn il_millenovecento_non_era_bisestile_e_il_duemila_si() {
        assert!(!bisestile(1900));
        assert!(bisestile(2000));
        assert!(bisestile(2024));
        assert!(!bisestile(2026));
        assert_eq!(giorni_del_mese(1900, 2), 28);
        assert_eq!(giorni_del_mese(2000, 2), 29);
    }

    #[test]
    fn i_giorni_della_settimana_sono_quelli_veri() {
        // Date verificate a mano: il 2 settembre 2026 e' un mercoledi'.
        assert_eq!(giorno_settimana(2026, 9, 2), 2, "mercoledi'");
        assert_eq!(giorno_settimana(1970, 1, 1), 3, "giovedi'");
        assert_eq!(giorno_settimana(2000, 1, 1), 5, "sabato");
        assert_eq!(giorno_settimana(2024, 2, 29), 3, "giovedi'");
    }

    #[test]
    fn il_riporto_attraversa_mesi_e_anni() {
        let capodanno = DataOra::nuova(2026, 12, 31, 23, 0, 0);
        assert_eq!(capodanno.piu_giorni(1).iso(), "2027-01-01T23:00:00");
        let fine_febbraio = DataOra::nuova(2024, 2, 28, 9, 0, 0);
        assert_eq!(fine_febbraio.piu_giorni(1).iso(), "2024-02-29T09:00:00");
        let non_bisestile = DataOra::nuova(2026, 2, 28, 9, 0, 0);
        assert_eq!(non_bisestile.piu_giorni(1).iso(), "2026-03-01T09:00:00");
        assert_eq!(capodanno.piu_giorni(365).iso(), "2027-12-31T23:00:00");
    }

    #[test]
    fn indietro_si_torna_come_si_e_venuti() {
        let d = DataOra::nuova(2027, 1, 1, 10, 0, 0);
        assert_eq!(d.piu_giorni(-1).iso(), "2026-12-31T10:00:00");
        // Il 2026 NON e' bisestile, quindi ha 365 giorni: un anno indietro dal
        // primo gennaio 2027 e' 365, non 366. Qui la prima versione di questa
        // prova sbagliava — avevo scritto 366 per analogia con «un anno», e a
        // sbagliare era l'attesa, non il codice. E' il caso opposto a quello
        // solito, e vale la pena tenerlo scritto: quando una prova fallisce, la
        // prima cosa da verificare e' che abbia ragione lei.
        assert_eq!(d.piu_giorni(-365).iso(), "2026-01-01T10:00:00");
        assert_eq!(d.piu_giorni(-366).iso(), "2025-12-31T10:00:00");
        // E dal 2025, che segue un anno bisestile, il conto cambia di uno.
        let bisestile_dietro = DataOra::nuova(2025, 1, 1, 10, 0, 0);
        assert_eq!(bisestile_dietro.piu_giorni(-366).iso(), "2024-01-01T10:00:00");
        // Andata e ritorno: qualunque salto deve tornare al punto di partenza.
        for n in [1, 7, 30, 31, 59, 365, 400] {
            assert_eq!(d.piu_giorni(n).piu_giorni(-n), d, "salto {n}");
        }
    }

    #[test]
    fn i_minuti_scavallano_la_mezzanotte() {
        let sera = DataOra::nuova(2026, 9, 2, 23, 30, 0);
        assert_eq!(sera.piu_minuti(45).iso(), "2026-09-03T00:15:00");
        assert_eq!(sera.piu_minuti(60 * 24).iso(), "2026-09-03T23:30:00");
        let mattina = DataOra::nuova(2026, 9, 2, 0, 10, 0);
        assert_eq!(mattina.piu_minuti(-20).iso(), "2026-09-01T23:50:00");
    }

    #[test]
    fn iso_va_e_torna() {
        let d = DataOra::nuova(2026, 9, 2, 8, 5, 30);
        assert_eq!(DataOra::da_iso(&d.iso()), Some(d));
        assert_eq!(
            DataOra::da_iso("2026-09-02 08:05"),
            Some(DataOra::nuova(2026, 9, 2, 8, 5, 0))
        );
        assert_eq!(DataOra::da_iso("non una data"), None);
    }
}
