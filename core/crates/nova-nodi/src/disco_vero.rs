//! Il disco vero: `std::fs` dietro il tratto `Disco`.
//!
//! E' l'unico file di questo crate che parla col sistema operativo, ed e'
//! apposta: tutto il resto ragiona su percorsi relativi e stringhe, e si prova
//! senza toccare niente. Qui invece si guarda la cartella, si legge, e
//! soprattutto **si scrive senza poter lasciare un file a meta'**.

use std::path::{Path, PathBuf};

use crate::deposito::{da_leggere, Disco, DiscoScrivibile, Impronta};

/// Una cartella vera sul disco.
pub struct Cartella {
    radice: PathBuf,
}

impl Cartella {
    pub fn nuova(radice: impl Into<PathBuf>) -> Self {
        Cartella { radice: radice.into() }
    }

    pub fn radice(&self) -> &Path {
        &self.radice
    }

    /// Il percorso vero di un file del vault.
    ///
    /// I percorsi arrivano relativi e con la barra normale; qui si rimettono
    /// i pezzi uno per uno, che e' l'unico modo di ottenere il separatore
    /// giusto senza saperlo. E si rifiuta tutto cio' che uscirebbe dal vault:
    /// un `..` in un percorso e' una nota che scrive fuori dalla cartella
    /// dell'utente, e non c'e' nessun caso legittimo in cui serva.
    fn intero(&self, dove: &str) -> Option<PathBuf> {
        let mut p = self.radice.clone();
        for pezzo in dove.replace('\\', "/").split('/') {
            if pezzo.is_empty() || pezzo == "." {
                continue;
            }
            if pezzo == ".." {
                return None;
            }
            p.push(pezzo);
        }
        Some(p)
    }

    /// Tutti i `.md`, con percorso relativo e barra normale, **in ordine**.
    ///
    /// L'ordine non e' un vezzo: quando due file con lo stesso nome si
    /// contendono uno slug vince chi arriva prima, e «prima» dev'essere la
    /// stessa cosa a ogni avvio o il nodo visibile cambierebbe da solo.
    fn cammina(&self, da: &Path, dentro: &str, fuori: &mut Vec<String>) {
        let Ok(voci) = std::fs::read_dir(da) else {
            return;
        };
        let mut elenco: Vec<_> = voci.filter_map(|v| v.ok()).collect();
        elenco.sort_by_key(|v| v.file_name());
        for voce in elenco {
            let nome = voce.file_name().to_string_lossy().to_string();
            // Le cartelle col punto davanti sono di servizio: dentro `.nova`
            // ci sta il registro delle azioni, che non e' un ricordo.
            if nome.starts_with('.') {
                continue;
            }
            let relativo = if dentro.is_empty() {
                nome.clone()
            } else {
                format!("{dentro}/{nome}")
            };
            match voce.file_type() {
                Ok(t) if t.is_dir() => self.cammina(&voce.path(), &relativo, fuori),
                Ok(t) if t.is_file() => {
                    if da_leggere(&relativo) {
                        fuori.push(relativo);
                    }
                }
                // Un collegamento simbolico non si segue: seguirlo vorrebbe
                // dire leggere come nodo un file che sta fuori dal vault, e
                // poi riscriverlo li'.
                _ => {}
            }
        }
    }
}

impl Disco for Cartella {
    fn elenca(&self) -> Vec<String> {
        let mut fuori = Vec::new();
        self.cammina(&self.radice.clone(), "", &mut fuori);
        fuori
    }

    fn impronta(&self, dove: &str) -> Option<Impronta> {
        let p = self.intero(dove)?;
        let m = std::fs::metadata(&p).ok()?;
        // Data **e** dimensione: su NTFS il timestamp avanza a scatti di
        // circa quindici millisecondi, e due scritture dentro lo stesso
        // scatto sarebbero indistinguibili.
        let quando = m
            .modified()
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_secs_f64())
            .unwrap_or(0.0);
        Some(Impronta { quando, quanto: m.len() as i64 })
    }

    fn leggi(&self, dove: &str) -> Option<String> {
        let p = self.intero(dove)?;
        // Un file illeggibile — permessi, byte non UTF-8, sparito un istante
        // fa — non deve fermare la lettura del vault: si salta quello, non
        // tutto il resto.
        std::fs::read_to_string(p).ok()
    }
}

impl DiscoScrivibile for Cartella {
    /// **Di fianco e poi si rinomina.** E' il contratto del tratto, e qui e'
    /// dove viene mantenuto.
    ///
    /// Scrivere dritto sul file dell'utente vuol dire troncarlo prima di
    /// riempirlo: se qualcosa si mette di mezzo fra le due cose — NOVA
    /// chiusa, il PC spento, un errore — resta una nota vuota, che alla
    /// ricerca dopo c'e' ancora e non dice piu' niente.
    ///
    /// Il temporaneo sta **nella stessa cartella**, perche' la rinomina e'
    /// atomica solo dentro lo stesso filesystem; porta il pid, perche' due
    /// processi che salvano insieme non devono litigarselo; e finisce per
    /// `.parte-...`, quindi non e' un `.md` e chi legge il vault non lo vede
    /// nemmeno mentre esiste.
    fn scrivi(&self, dove: &str, testo: &str) -> Result<Impronta, String> {
        let p = self
            .intero(dove)
            .ok_or_else(|| format!("percorso che esce dal vault: {dove}"))?;
        if let Some(cartella) = p.parent() {
            std::fs::create_dir_all(cartella).map_err(|e| e.to_string())?;
        }
        let nome_tmp = format!(
            "{}.parte-{}",
            p.file_name().and_then(|n| n.to_str()).unwrap_or("nodo"),
            std::process::id()
        );
        let tmp = p.with_file_name(nome_tmp);
        let esito = (|| -> std::io::Result<()> {
            use std::io::Write;
            let mut f = std::fs::File::create(&tmp)?;
            f.write_all(testo.as_bytes())?;
            // Il `sync_all` copre anche la corrente che va via, non solo il
            // processo che muore. Su file di questa taglia costa una frazione
            // di millisecondo.
            f.sync_all()?;
            drop(f);
            std::fs::rename(&tmp, &p)
        })();
        if let Err(e) = esito {
            // Il temporaneo non resta li' a confondere chi guarda la
            // cartella. Il file vero non e' stato toccato: e' tutto il punto.
            let _ = std::fs::remove_file(&tmp);
            return Err(e.to_string());
        }
        self.impronta(dove)
            .ok_or_else(|| format!("scritto ma non rileggibile: {dove}"))
    }
}

/// Dove sta il registro delle azioni, dentro il vault.
///
/// Sotto una cartella col punto davanti, quindi fuori da cio' che si legge
/// come nodo: e' una traccia di cosa NOVA ha fatto, non un ricordo.
pub const REGISTRO: &str = ".nova/registro.jsonl";

impl Cartella {
    /// Accoda una riga al registro, ruotandolo se e' cresciuto troppo.
    ///
    /// Accodare non tronca niente, quindi qui non serve la scrittura di
    /// fianco: il file di prima non e' mai in pericolo. Al massimo si perde
    /// l'ultima riga, ed e' il caso in cui c'e' poco da salvare comunque.
    ///
    /// La riga la compone chi chiama: cosa vada scritto in un registro non lo
    /// decide chi tiene i file.
    pub fn accoda_al_registro(&self, riga: &str) -> Result<(), String> {
        let Some(p) = self.intero(REGISTRO) else {
            return Err("percorso del registro fuori dal vault".into());
        };
        self.ruota_se_serve(&p);
        if let Some(cartella) = p.parent() {
            std::fs::create_dir_all(cartella).map_err(|e| e.to_string())?;
        }
        use std::io::Write;
        let mut f = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&p)
            .map_err(|e| e.to_string())?;
        writeln!(f, "{riga}").map_err(|e| e.to_string())
    }

    /// Un file solo di storico, poi si ricomincia.
    ///
    /// Ogni salvataggio e ogni ricerca scrivono una riga: senza rotazione il
    /// file cresce per sempre e non c'e' nessuno che lo poti. Il precedente si
    /// tiene — chi ripara guarda proprio li' — ma uno solo.
    fn ruota_se_serve(&self, p: &Path) {
        let Ok(m) = std::fs::metadata(p) else {
            return;
        };
        // Una riga sola, e vale la pena dire cosa **non** la copre: la
        // decisione e' provata per intero in `si_ruota`, ma che qui le si
        // passi `m.is_file()` e non `true` nessuna prova lo vede. Per
        // vederlo servirebbe una cartella che da sola misura piu' di due
        // megabyte, cioe' decine di migliaia di file creati a ogni giro.
        // Resta scritto qui invece di far finta che sia coperto.
        if !si_ruota(m.is_file(), m.len()) {
            return;
        }
        let precedente = nova_potatura::precedente(p);
        let _ = std::fs::remove_file(&precedente);
        let _ = std::fs::rename(p, &precedente);
    }
}

/// Se questo va messo da parte.
///
/// **Solo i file normali**, e la riga esiste per questo. Su Windows una
/// cartella misura zero byte, quindi finiva sotto il tetto e usciva da sola;
/// su Linux e macOS ne misura 4096, cioe' passa il controllo e arriva al
/// `rename` — che una cartella la **sposta**. Da Windows non si sarebbe visto
/// mai, ed e' la stessa riga che dalla parte Python poteva spostare una
/// cartella dell'utente senza dire niente a nessuno.
///
/// Prende i **due fatti** invece del `Metadata`, e non e' pedanteria. Con il
/// `Metadata` in mano la prova puo' solo passargli una cartella vera, che su
/// questo disco misura 4096 byte: sotto il tetto, quindi la risposta sarebbe
/// «no» lo stesso anche togliendo il controllo sul tipo, e la mutazione
/// resterebbe verde. Cosi' invece la domanda si puo' fare per intero — una
/// cartella **oltre** il tetto — che e' esattamente il caso che si vuole
/// escludere.
pub fn si_ruota(e_un_file: bool, byte: u64) -> bool {
    nova_potatura::si_ruota(e_un_file, byte)
}

#[cfg(test)]
mod prove {
    use super::*;

    fn cartella_di_prova(nome: &str) -> PathBuf {
        let p = std::env::temp_dir().join(format!("nova-disco-{}-{}", nome, std::process::id()));
        let _ = std::fs::remove_dir_all(&p);
        std::fs::create_dir_all(&p).unwrap();
        p
    }

    #[test]
    fn si_scrive_si_rilegge_e_non_resta_niente_per_terra() {
        let radice = cartella_di_prova("giro");
        let c = Cartella::nuova(&radice);
        c.scrivi("02-persone/anna.md", "ciao\n").unwrap();
        assert_eq!(c.leggi("02-persone/anna.md").as_deref(), Some("ciao\n"));
        assert_eq!(c.elenca(), vec!["02-persone/anna.md"]);
        // nessun `.parte-` sopravvissuto
        let residui: Vec<String> = std::fs::read_dir(radice.join("02-persone"))
            .unwrap()
            .filter_map(|v| v.ok())
            .map(|v| v.file_name().to_string_lossy().to_string())
            .filter(|n| n.contains(".parte-"))
            .collect();
        assert!(residui.is_empty(), "{residui:?}");
        let _ = std::fs::remove_dir_all(&radice);
    }

    #[test]
    fn gli_a_capo_restano_quelli_del_testo() {
        // In modo testo su Windows un `\n` diventerebbe `\r\n`, e il file su
        // disco non sarebbe piu' quello che gli e' stato dato: il formato del
        // vault e' un contratto, e un contratto non si riscrive per conto suo.
        let radice = cartella_di_prova("acapo");
        let c = Cartella::nuova(&radice);
        c.scrivi("x.md", "una\ndue\n").unwrap();
        let grezzo = std::fs::read(radice.join("x.md")).unwrap();
        assert!(!grezzo.windows(2).any(|w| w == b"\r\n"), "{grezzo:?}");
        let _ = std::fs::remove_dir_all(&radice);
    }

    #[test]
    fn un_percorso_che_esce_dal_vault_viene_rifiutato() {
        let radice = cartella_di_prova("fuga");
        let c = Cartella::nuova(&radice);
        assert!(c.scrivi("../fuori.md", "x").is_err());
        assert!(c.scrivi("a/../../fuori.md", "x").is_err());
        assert!(c.leggi("../fuori.md").is_none());
        let _ = std::fs::remove_dir_all(&radice);
    }

    #[test]
    fn lelenco_salta_lindice_e_le_cartelle_di_servizio() {
        let radice = cartella_di_prova("elenco");
        let c = Cartella::nuova(&radice);
        c.scrivi("06-fatti/uno.md", "x").unwrap();
        c.scrivi("_INDICE.md", "x").unwrap();
        std::fs::create_dir_all(radice.join(".nova")).unwrap();
        std::fs::write(radice.join(".nova/audit.jsonl"), "{}").unwrap();
        std::fs::write(radice.join("06-fatti/appunti.txt"), "x").unwrap();
        assert_eq!(c.elenca(), vec!["06-fatti/uno.md"]);
        let _ = std::fs::remove_dir_all(&radice);
    }

    #[test]
    fn limpronta_cambia_quando_cambia_il_contenuto() {
        let radice = cartella_di_prova("impronta");
        let c = Cartella::nuova(&radice);
        let a = c.scrivi("x.md", "corto").unwrap();
        let b = c.scrivi("x.md", "molto piu' lungo di prima").unwrap();
        assert_ne!(a, b);
        let _ = std::fs::remove_dir_all(&radice);
    }

    #[test]
    fn una_cartella_che_non_esiste_non_e_un_errore_ma_un_vault_vuoto() {
        let c = Cartella::nuova(std::env::temp_dir().join("nova-questa-non-esiste-di-sicuro"));
        assert!(c.elenca().is_empty());
        assert!(c.leggi("x.md").is_none());
        assert!(c.impronta("x.md").is_none());
    }

    #[test]
    fn il_registro_si_accoda_e_non_e_un_nodo() {
        let radice = cartella_di_prova("registro");
        let c = Cartella::nuova(&radice);
        c.accoda_al_registro("{\"azione\":\"salva\"}").unwrap();
        c.accoda_al_registro("{\"azione\":\"cerca\"}").unwrap();
        let testo = std::fs::read_to_string(radice.join(".nova/registro.jsonl")).unwrap();
        assert_eq!(testo.lines().count(), 2, "{testo:?}");
        // E non compare fra i nodi: sta sotto una cartella col punto.
        assert!(c.elenca().is_empty());
        let _ = std::fs::remove_dir_all(&radice);
    }

    #[test]
    fn il_registro_ruota_quando_diventa_grosso_e_ne_resta_uno_solo() {
        let radice = cartella_di_prova("rotazione");
        let c = Cartella::nuova(&radice);
        std::fs::create_dir_all(radice.join(".nova")).unwrap();
        // Un registro gia' oltre la soglia: la prossima riga lo fa ruotare.
        let grosso = "x".repeat(crate::deposito::MAX_REGISTRO_BYTE as usize + 1);
        std::fs::write(radice.join(".nova/registro.jsonl"), &grosso).unwrap();
        c.accoda_al_registro("{\"azione\":\"dopo\"}").unwrap();
        let nuovo = std::fs::read_to_string(radice.join(".nova/registro.jsonl")).unwrap();
        assert!(nuovo.contains("dopo") && nuovo.len() < 100, "non ha ruotato");
        let vecchio = std::fs::read_to_string(radice.join(".nova/registro.1.jsonl")).unwrap();
        assert_eq!(vecchio.len(), grosso.len(), "lo storico e' andato perso");
        let _ = std::fs::remove_dir_all(&radice);
    }

    #[test]
    fn una_cartella_non_si_ruota_mai_per_quanto_grossa_sia() {
        let tetto = crate::deposito::MAX_REGISTRO_BYTE;
        assert!(!si_ruota(false, tetto + 1), "una cartella non si mette da parte: la si sposterebbe");
        assert!(!si_ruota(false, u64::MAX), "nemmeno enorme");
        // E un file oltre il tetto deve dire di si', o le righe qui sopra
        // sarebbero verdi perche' non ruota mai niente.
        assert!(si_ruota(true, tetto + 1));
        assert!(!si_ruota(true, 0), "e uno vuoto no");
    }

    #[test]
    fn e_una_cartella_vera_passa_di_li_senza_essere_spostata() {
        let radice = cartella_di_prova("cartella-non-si-sposta");
        let c = Cartella::nuova(&radice);
        let finto = radice.join(REGISTRO);
        std::fs::create_dir_all(&finto).unwrap();
        c.ruota_se_serve(&finto);
        assert!(finto.is_dir(), "la cartella e' stata spostata");
        let _ = std::fs::remove_dir_all(&radice);
    }
}
