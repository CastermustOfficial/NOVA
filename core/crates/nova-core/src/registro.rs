//! Il registro delle azioni che non si annullano, dal lato del demone.
//!
//! NOVA ne aveva gia' uno, in Python, e diceva di se' la cosa giusta: «la
//! responsabilita' ha bisogno di visibilita', si risponde solo di quello che
//! si puo' vedere». Ci finivano le candidature inviate, i click su un
//! pulsante, i moduli compilati.
//!
//! Non ci finiva niente di quello che fa **il demone**, che e' il pezzo di
//! NOVA che gira quando non c'e' nessuno a guardare: `shell.exec` esegue un
//! comando qualunque nella shell del sistema, `fs.write` riscrive un file.
//! Il demone teneva il suo giornale — quello serve ad **annullare**, ed e'
//! un'altra domanda — e chi chiedeva a NOVA «cosa hai fatto?» si sentiva
//! rispondere con meta' della storia, senza sapere che era meta'.
//!
//! Qui c'e' l'altra meta', e scrive **lo stesso file**: due registri sarebbero
//! due risposte alla stessa domanda. La forma della riga — il mascheramento,
//! i tagli, cosa si perde quando il campo annuncia una credenziale — sta in
//! `nova-registro`, con il Python di fronte e un banco che li confronta
//! carattere per carattere.

use std::path::{Path, PathBuf};

use nova_registro::{Filtro, Riga, Scritta};

/// Quante righe si leggono quando nessuno dice quante.
pub const QUANTE: usize = 30;

/// Dove sta il registro: lo stesso file del Python, e non per caso.
pub fn percorso() -> PathBuf {
    let base = std::env::var("APPDATA")
        .map(PathBuf::from)
        .or_else(|_| std::env::var("HOME").map(|h| PathBuf::from(h).join(".config")))
        .unwrap_or_else(|_| std::env::temp_dir());
    base.join("NOVA").join("azioni.jsonl")
}

/// L'ora come la scrive il Python: `2026-08-30T14:18:05`, **locale**.
///
/// Locale e non UTC, e non e' formattazione: su questo file scrivono tutte e
/// due le meta' di NOVA, e la parte Python scrive l'ora dell'orologio di chi
/// sta davanti al computer. Due righe della stessa giornata sfalsate di
/// un'ora non darebbero nessun errore — sarebbero solo sbagliate, e chi
/// rilegge non ha modo di accorgersene.
pub(crate) fn adesso() -> String {
    let t = nova_platform::orologio::adesso();
    quando(t, nova_platform::fuso_secondi(t))
}

/// Quell'istante, in quel fuso, come lo scrive il Python.
///
/// Separata da [`adesso`] perche' cosi' si puo' provare: con l'orologio e il
/// fuso dentro, l'unica prova possibile sarebbe rifare lo stesso conto — e su
/// una macchina regolata su UTC, che e' quella della CI, non distinguerebbe
/// **niente** da un demone che scrive l'ora di Greenwich.
fn quando(istante: i64, fuso_secondi: i64) -> String {
    nova_calendario::da_istante(istante, fuso_secondi).iso()
}

/// Scrive una riga. **Non fallisce mai**, come dall'altra parte: un registro
/// che impedisce di lavorare viene tolto di mezzo dopo mezza giornata, ed e'
/// peggio che non averlo.
pub fn annota(azione: &str, dove: &str, dettagli: &str, tipo: &str, esito: &str) {
    annota_in(&percorso(), azione, dove, dettagli, tipo, esito);
}

/// Come [`annota`], ma su un file scelto.
///
/// Il file si passa invece di leggerlo da una variabile d'ambiente: una
/// variabile e' del **processo**, e due prove che la cambiano insieme si
/// pestano i piedi — cosa che e' successa subito, la prima volta che ho
/// scritto queste prove.
pub fn annota_in(f: &Path, azione: &str, dove: &str, dettagli: &str, tipo: &str, esito: &str) {
    let s = Scritta {
        azione,
        dove,
        dettagli,
        tipo,
        esito,
    };
    let riga = nova_registro::riga_da_scrivere(&s, &adesso());
    // Si pota **prima** di scrivere: dopo vuol dire che il file supera sempre
    // il tetto di una riga, e su un file che si apre per capire cos'e'
    // successo l'ultima riga e' quella che conta. Uno alla volta, perche' ci
    // scrivono Nova e i Dot insieme (`crate::righe`).
    let _ = crate::righe::aggiungi(f, &riga, true);
}

/// Le ultime righe, dalla piu' recente.
///
/// Legge **anche lo storico**, e prima di quello vivo. Il registro si pota, ed
/// e' l'unico file potato di NOVA su cui si fa una domanda vecchia: «cosa ho
/// mandato a quella societa'?» tre settimane dopo. Leggendo solo il file vivo,
/// il giorno della potatura quella domanda comincerebbe a rispondere
/// «niente» — senza errori e senza che si possa capire perche'.
pub fn leggi(quante: usize) -> Vec<Riga> {
    leggi_da(&percorso(), quante)
}

/// Come [`leggi`], ma da un file scelto.
pub fn leggi_da(f: &Path, quante: usize) -> Vec<Riga> {
    let mut righe: Vec<Riga> = Vec::new();
    for parte in [nova_potatura::precedente(f), f.to_path_buf()] {
        let Ok(testo) = std::fs::read_to_string(&parte) else {
            continue;
        };
        righe.extend(testo.lines().filter_map(|r| {
            let r = r.trim();
            if r.is_empty() {
                None
            } else {
                nova_registro::riga_letta(r)
            }
        }));
    }
    let quante = if quante == 0 { righe.len() } else { quante };
    let da = righe.len().saturating_sub(quante);
    let mut ultime: Vec<Riga> = righe.split_off(da);
    ultime.reverse();
    ultime
}

/// Oggi, in AAAA-MM-GG: quel che serve per dire «oggi» e «ieri» invece di
/// una data.
pub fn oggi() -> String {
    adesso().chars().take(10).collect()
}

/// Cerca, con le regole del Python: tutte le parole, in qualunque campo e in
/// qualunque ordine, senza accenti e senza maiuscole.
pub fn cerca(f: &Filtro, fra_quante: usize) -> Vec<Riga> {
    cerca_in(&percorso(), f, fra_quante)
}

/// Come [`cerca`], ma su un file scelto.
pub fn cerca_in(dove: &Path, f: &Filtro, fra_quante: usize) -> Vec<Riga> {
    let righe = leggi_da(dove, fra_quante);
    nova_registro::cerca(&righe, f)
        .into_iter()
        .cloned()
        .collect()
}

/// Quanto c'e' dentro, di che tipo, da quando, e dove sta il file: la
/// risposta a «che cos'e' questo registro», come `registro.riassunto` del
/// Python.
pub fn riassunto_in(f: &Path) -> String {
    let righe = leggi_da(f, 100_000);
    if righe.is_empty() {
        return format!("Il registro e' vuoto.\nSta in {}", f.display());
    }
    let r = nova_registro::riassunto(&righe);
    let quando = if r.prima == r.ultima {
        format!("dal {}", nova_registro::data_italiana(&r.prima))
    } else {
        format!("dal {} al {}", nova_registro::data_italiana(&r.prima), nova_registro::data_italiana(&r.ultima))
    };
    let tipi: Vec<String> = r.tipi.iter().map(|(k, n)| format!("{n} {k}")).collect();
    format!(
        "{} azioni registrate, {quando}.\n  {}\nSta in {}",
        r.quante,
        tipi.join(", "),
        f.display()
    )
}

/// `novad --registro [PAROLE] [--giorni N]`: quello che faceva
/// `python -m nova --registro` (D357).
///
/// Legge un file e lo racconta, senza configurazione e senza cervello: deve
/// rispondere anche quando NOVA non parte piu', perche' e' proprio il momento
/// in cui serve sapere cosa aveva fatto. Senza parole e senza giorni comincia
/// col riassunto; con delle parole che non trovano niente lo dice, invece di
/// un racconto vuoto che sembra un registro vuoto.
pub fn per_chi_chiede(parole: &str, giorni: f64) -> String {
    let t = nova_platform::orologio::adesso();
    per_chi_chiede_in(&percorso(), parole, giorni, t, nova_platform::fuso_secondi(t))
}

/// Come [`per_chi_chiede`], con il file, l'ora e il fuso passati da fuori.
pub fn per_chi_chiede_in(f: &Path, parole: &str, giorni: f64, ora: i64, fuso: i64) -> String {
    let parole = parole.trim();
    let mut fuori = String::new();
    if parole.is_empty() && giorni <= 0.0 {
        fuori.push_str(&riassunto_in(f));
        fuori.push_str("\n\n");
    }
    let non_prima_di = if giorni > 0.0 {
        quando(ora - (giorni * 86_400.0) as i64, fuso)
    } else {
        String::new()
    };
    let filtro = Filtro {
        testo: parole.to_string(),
        non_prima_di,
        quante: 40,
        ..Filtro::default()
    };
    let righe = cerca_in(f, &filtro, 10_000);
    if !parole.is_empty() && righe.is_empty() {
        return format!("Niente che contenga «{parole}».");
    }
    let oggi: String = quando(ora, fuso).chars().take(10).collect();
    let prestate: Vec<&Riga> = righe.iter().collect();
    fuori.push_str(&nova_registro::racconta(&prestate, &oggi));
    fuori
}

#[cfg(test)]
mod prove {
    /// `novad --registro`: il riassunto quando non si cerca niente, il
    /// «niente» quando si cerca e non si trova, e la finestra dei giorni.
    #[test]
    fn per_chi_chiede_riassume_cerca_e_taglia() {
        let dir = std::env::temp_dir().join(format!("nova-reg-cli-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let f = dir.join("azioni.jsonl");
        // 2026-09-28T12:00:00 UTC, fuso zero: le righe hanno l'ora cosi'.
        let ora = 1_790_596_800;
        std::fs::write(
            &f,
            "{\"quando\": \"2026-09-20T10:00:00\", \"tipo\": \"file\", \"azione\": \"cancellato\", \"dove\": \"vecchio.txt\", \"dettagli\": \"\", \"esito\": \"ok\"}\n\
             {\"quando\": \"2026-09-28T09:00:00\", \"tipo\": \"browser\", \"azione\": \"inviato modulo\", \"dove\": \"Societa' Rossi\", \"dettagli\": \"\", \"esito\": \"ok\"}\n",
        )
        .unwrap();
        let tutto = per_chi_chiede_in(&f, "", 0.0, ora, 0);
        assert!(tutto.starts_with("2 azioni registrate, dal 20/09/2026 al 28/09/2026."), "{tutto}");
        assert!(tutto.contains("Sta in "), "{tutto}");
        let cercato = per_chi_chiede_in(&f, "societa", 0.0, ora, 0);
        assert!(!cercato.contains("azioni registrate"), "{cercato}");
        assert!(cercato.contains("inviato modulo") && !cercato.contains("vecchio.txt"), "{cercato}");
        let niente = per_chi_chiede_in(&f, "banca", 0.0, ora, 0);
        assert_eq!(niente, "Niente che contenga «banca».");
        let ultimi = per_chi_chiede_in(&f, "", 2.0, ora, 0);
        assert!(ultimi.contains("inviato modulo") && !ultimi.contains("vecchio.txt"), "{ultimi}");
        let vuoto = per_chi_chiede_in(&dir.join("non_c_e.jsonl"), "", 0.0, ora, 0);
        assert!(vuoto.starts_with("Il registro e' vuoto."), "{vuoto}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    use super::*;
    use std::io::Write;

    /// Una cartella che si cancella da sola.
    ///
    /// Le prove Rust girano **nello stesso processo**, su piu' fili: la prima
    /// stesura di queste prove spostava `APPDATA`, che e' del processo, e
    /// quattro prove su sette sono diventate rosse a caso. Qui ognuna ha il
    /// suo file e non tocca niente di condiviso.
    struct Casa(PathBuf);

    impl Casa {
        fn nuova(nome: &str) -> Casa {
            let p =
                std::env::temp_dir().join(format!("nova-registro-{nome}-{}", std::process::id()));
            let _ = std::fs::remove_dir_all(&p);
            std::fs::create_dir_all(&p).unwrap();
            Casa(p)
        }

        fn file(&self) -> PathBuf {
            self.0.join("azioni.jsonl")
        }
    }

    impl Drop for Casa {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn quel_che_si_scrive_si_rilegge() {
        let casa = Casa::nuova("giro");
        annota_in(
            &casa.file(),
            "eseguito un comando",
            "C:\\lavoro",
            "Get-Process",
            "comando",
            "0",
        );
        let righe = leggi_da(&casa.file(), QUANTE);
        assert_eq!(righe.len(), 1);
        assert_eq!(righe[0].azione, "eseguito un comando");
        assert_eq!(righe[0].tipo, "comando");
        assert_eq!(righe[0].esito, "0");
        assert!(
            !righe[0].quando.is_empty(),
            "senza l'ora una riga non e' un fatto"
        );
    }

    #[test]
    fn dalla_piu_recente() {
        let casa = Casa::nuova("ordine");
        for a in ["prima", "seconda", "terza"] {
            annota_in(&casa.file(), a, "", "", "comando", "");
        }
        let righe = leggi_da(&casa.file(), QUANTE);
        assert_eq!(
            righe.iter().map(|r| r.azione.as_str()).collect::<Vec<_>>(),
            ["terza", "seconda", "prima"]
        );
    }

    #[test]
    fn una_chiave_dentro_un_comando_non_resta_sul_disco() {
        let casa = Casa::nuova("chiave");
        annota_in(
            &casa.file(),
            "eseguito un comando",
            "",
            "curl -H 'Authorization: Bearer sk-abcdefghijklmnopqrst' https://x.it",
            "comando",
            "0",
        );
        let testo = std::fs::read_to_string(casa.file()).unwrap();
        assert!(
            !testo.contains("sk-abcdefghijklmnopqrst"),
            "la chiave e' rimasta: {testo}"
        );
        assert!(testo.contains("[chiave]"));
    }

    #[test]
    fn e_nemmeno_un_valore_che_il_campo_annuncia() {
        let casa = Casa::nuova("etichetta");
        annota_in(
            &casa.file(),
            "scritto in #password",
            "https://banca.it",
            "Tramonto2026!",
            "browser",
            "",
        );
        let testo = std::fs::read_to_string(casa.file()).unwrap();
        assert!(
            !testo.contains("Tramonto2026"),
            "il valore e' rimasto: {testo}"
        );
        // La riga resta: sparisce il valore, non il fatto che sia successo.
        assert!(testo.contains("scritto in #password"));
    }

    #[test]
    fn una_riga_storta_non_ferma_le_altre() {
        let casa = Casa::nuova("storta");
        annota_in(&casa.file(), "buona", "", "", "comando", "");
        let mut f = std::fs::OpenOptions::new()
            .append(true)
            .open(casa.file())
            .unwrap();
        writeln!(f, "{{questa non e' una riga}}").unwrap();
        drop(f);
        annota_in(&casa.file(), "anche questa", "", "", "comando", "");
        let righe = leggi_da(&casa.file(), QUANTE);
        assert_eq!(
            righe.len(),
            2,
            "una riga storta si salta, non ferma la lettura"
        );
    }

    #[test]
    fn si_cerca_come_di_la() {
        let casa = Casa::nuova("cerca");
        annota_in(
            &casa.file(),
            "inviata candidatura",
            "https://lavoro.it/44",
            "Societa' Rossi",
            "browser",
            "ok",
        );
        annota_in(
            &casa.file(),
            "eseguito un comando",
            "",
            "Get-Process",
            "comando",
            "0",
        );
        let trovate = cerca_in(
            &casa.file(),
            &Filtro {
                testo: "ROSSI".into(),
                ..Default::default()
            },
            QUANTE,
        );
        assert_eq!(trovate.len(), 1);
        assert_eq!(trovate[0].azione, "inviata candidatura");
        let per_tipo = cerca_in(
            &casa.file(),
            &Filtro {
                tipo: "comando".into(),
                ..Default::default()
            },
            QUANTE,
        );
        assert_eq!(per_tipo.len(), 1);
        assert_eq!(per_tipo[0].tipo, "comando");
    }

    #[test]
    fn anche_lo_storico_si_legge() {
        // Il giorno della potatura, «cosa ho mandato tre settimane fa?» deve
        // continuare a rispondere.
        let casa = Casa::nuova("storico");
        annota_in(&casa.file(), "vecchissima", "", "", "comando", "");
        std::fs::rename(casa.file(), nova_potatura::precedente(&casa.file())).unwrap();
        annota_in(&casa.file(), "nuova", "", "", "comando", "");
        let righe = leggi_da(&casa.file(), QUANTE);
        assert_eq!(
            righe.iter().map(|r| r.azione.as_str()).collect::<Vec<_>>(),
            ["nuova", "vecchissima"]
        );
    }

    #[test]
    fn lora_e_quella_di_casa_non_quella_di_greenwich() {
        // Il fuso si passa, cosi' la prova dice la stessa cosa su una
        // macchina italiana e su quella della CI, che sta su UTC.
        assert_eq!(quando(0, 3600), "1970-01-01T01:00:00");
        assert_eq!(quando(0, 0), "1970-01-01T00:00:00");
        assert_eq!(quando(1_788_611_696, 7200), "2026-09-05T14:34:56");
        // E l'ora che si scrive davvero e' quella che il sistema dichiara.
        let t = nova_platform::orologio::adesso();
        assert_eq!(adesso(), quando(t, nova_platform::fuso_secondi(t)));
    }

    #[test]
    fn e_ha_la_forma_che_il_python_si_aspetta() {
        // `2026-08-30T14:18:05`: diciannove caratteri, niente fuso in coda.
        // Il Python la rilegge con `datetime.fromisoformat`, e una «Z» di
        // troppo la' dentro e' una riga che sparisce dal filtro per data.
        let r = adesso();
        assert_eq!(r.len(), 19, "{r}");
        assert!(r.as_bytes()[10] == b'T' && !r.ends_with('Z'), "{r}");
    }
}
