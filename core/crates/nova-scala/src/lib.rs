//! # nova-scala
//!
//! Chi risponde a cosa.
//!
//! L'idea di NOVA e' che **il modello locale orchestra**: e' gratis, e'
//! privato, sta gia' in VRAM, e per capire cosa vuole l'utente e chiamare i
//! tool giusti basta e avanza. Quando il compito lo supera non ci prova lo
//! stesso: passa la palla a un gradino piu' alto.
//!
//! Questo modulo e' la parte che decide, e va detto perche' e' il pezzo del
//! cantiere con le conseguenze piu' pesanti: **niente esce dal PC finche'
//! qualcuno non delega davvero**. Se le ricette sbagliano, sbagliano un
//! suggerimento; se il BM25 sbaglia, sbaglia un ordinamento; se questo
//! sbaglia, un compito che doveva restare in casa prende la porta.
//!
//! Fuori restano di proposito due cose, perche' non sono decisioni:
//! costruire davvero un cervello e chiedergli se e' a consumo, e la delega
//! vera e propria. Qui c'e' solo la matematica della scala.

use std::collections::BTreeMap;

pub mod parole;
pub use parole::parola_presente;

/// Un gradino: quale cervello, con quale modello, e quanto costa.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Gradino {
    pub nome: String,
    /// `locale` | `claude` | `api` | un nome dentro `brains.cli`.
    pub brain: String,
    pub model: String,
    pub descrizione: String,
    /// Vero = non esce niente dal PC.
    pub locale: bool,
    pub a_pagamento: bool,
}

/// Una categoria di compiti che sale per regola, non per auto-valutazione.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Categoria {
    pub nome: String,
    pub attiva: bool,
    pub gradino_minimo: String,
    pub parole: Vec<String>,
    pub min_file: i64,
    pub descrizione: String,
}

/// Com'e' configurata la scala.
#[derive(Debug, Clone, Default)]
pub struct Configurazione {
    /// I gradini, nell'ordine in cui stanno scritti nel file.
    pub tiers: Vec<Gradino>,
    /// L'ordine dichiarato a parte. Vedi [`scala`].
    pub scala_dichiarata: Vec<String>,
    pub escalation_automatica: bool,
    pub solo_locale: bool,
    pub categorie: Vec<Categoria>,
    pub tetto_usd_sessione: f64,
    pub costo_stimato_delega: f64,
}

impl Configurazione {
    pub fn gradino(&self, nome: &str) -> Option<&Gradino> {
        self.tiers.iter().find(|t| t.nome == nome)
    }
}

/// I gradini in ordine di potenza.
///
/// L'ordine sta scritto a parte e **non** si deduce dall'ordine delle chiavi
/// dei gradini. Sembra una ripetizione e invece e' la differenza fra
/// un'escalation che funziona e una che non parte mai: basta che un qualunque
/// programma riscriva il file ordinando le chiavi - e ne esistono, il
/// pannello lo faceva - perche' «standard» finisca dopo «difficile» e non ci
/// sia piu' niente sopra a cui salire. Un ordine che conta non puo' dipendere
/// da com'e' fatto un dizionario.
///
/// I gradini non elencati non spariscono: si accodano, cosi' uno aggiunto a
/// mano resta l'ultimo invece di svanire.
pub fn scala(cfg: &Configurazione) -> Vec<String> {
    let nomi: Vec<&str> = cfg.tiers.iter().map(|t| t.nome.as_str()).collect();
    let mut fuori: Vec<String> = cfg
        .scala_dichiarata
        .iter()
        .filter(|n| nomi.contains(&n.as_str()))
        .cloned()
        .collect();
    for n in &nomi {
        if !fuori.iter().any(|x| x == n) {
            fuori.push((*n).to_string());
        }
    }
    fuori
}

/// Il gradino subito sopra, se c'e'.
pub fn successivo(cfg: &Configurazione, nome: &str) -> Option<String> {
    let s = scala(cfg);
    let i = s.iter().position(|x| x == nome)?;
    s.get(i + 1).cloned()
}

/// Posizione nella scala, `-1` se il gradino non esiste.
pub fn indice(cfg: &Configurazione, nome: &str) -> i64 {
    scala(cfg)
        .iter()
        .position(|x| x == nome)
        .map(|i| i as i64)
        .unwrap_or(-1)
}

/// Il gradino minimo imposto dalle categorie, e perche'.
///
/// Un modello che non conosce il codice non puo' sapere quanto e' profondo il
/// fondo: si giudica capace, risponde, e quello che non ha visto non lo
/// segnala nessuno. Per le categorie in cui l'errore non si vede - revisione
/// su piu' file, perdita di dati, architettura - il gradino minimo lo decide
/// la configurazione. L'auto-valutazione puo' ancora alzare, mai abbassare.
///
/// Si guarda **solo il compito**, mai il contesto: quello e' quasi sempre il
/// sorgente dei file allegati, e far scattare una categoria su una parola
/// dentro un commento manderebbe su tutto.
pub fn gradino_minimo(cfg: &Configurazione, compito: &str, allegati: i64) -> (String, String) {
    if !cfg.escalation_automatica {
        // Chi ha spento le salite automatiche le ha spente.
        return (String::new(), String::new());
    }
    let testo = compito.to_lowercase();
    let s = scala(cfg);
    let mut migliore = String::new();
    let mut motivo = String::new();
    let mut posizione: i64 = -1;

    for c in &cfg.categorie {
        if !c.attiva {
            continue;
        }
        if !s.iter().any(|x| *x == c.gradino_minimo) {
            continue;
        }
        // Una categoria senza parole e senza soglia scatterebbe sempre: e'
        // scritta male, e una configurazione scritta male non deve mandare
        // fuori casa ogni compito.
        if c.parole.is_empty() && c.min_file <= 0 {
            continue;
        }
        if allegati < c.min_file {
            continue;
        }
        if !c.parole.is_empty()
            && !c.parole.iter().any(|p| parola_presente(&p.to_lowercase(), &testo))
        {
            continue;
        }
        let i = s.iter().position(|x| *x == c.gradino_minimo).unwrap() as i64;
        if i > posizione {
            posizione = i;
            migliore = c.gradino_minimo.clone();
            motivo = if c.descrizione.is_empty() {
                c.nome.clone()
            } else {
                c.descrizione.clone()
            };
        }
    }
    (migliore, motivo)
}

/// Lo stato delle pause: gradino -> secondi che mancano.
pub type Pause = BTreeMap<String, i64>;

/// Vale la pena provarci?
///
/// `a_consumo` arriva da fuori perche' saperlo vuol dire costruire un
/// cervello e interrogarlo, che non e' una decisione: e' un effetto.
pub fn utilizzabile(
    cfg: &Configurazione,
    nome: &str,
    pause: &Pause,
    speso_usd: f64,
    prenotato_usd: f64,
    a_consumo: &dyn Fn(&Gradino) -> bool,
) -> bool {
    let t = match cfg.gradino(nome) {
        Some(t) => t,
        None => return false,
    };
    if cfg.solo_locale && !t.locale {
        return false;
    }
    if pause.get(nome).copied().unwrap_or(0) > 0 {
        return false;
    }
    // Se il tetto lo rifiuterebbe comunque, salire vuol dire solo perdere il
    // ripiego: meglio restare dov'e' il compito.
    if cfg.tetto_usd_sessione > 0.0 && a_consumo(t) {
        let stima = if cfg.costo_stimato_delega > 0.0 {
            cfg.costo_stimato_delega
        } else {
            0.10
        };
        if speso_usd + prenotato_usd + stima > cfg.tetto_usd_sessione {
            return false;
        }
    }
    true
}

/// Chi puo' sostituire un gradino a quota esaurita.
///
/// Un altro modello dello **stesso fornitore** non serve: il limite e' sul
/// conto, non sul modello. Si cambia fornitore, e in ultimo si torna a casa -
/// per questo i locali vengono in coda e non spariscono.
pub fn ripieghi(cfg: &Configurazione, fallito: &str, pause: &Pause) -> Vec<String> {
    let brand_fallito = cfg.gradino(fallito).map(|t| t.brain.clone()).unwrap_or_default();
    let libero = |n: &str| pause.get(n).copied().unwrap_or(0) == 0;

    let altri: Vec<String> = cfg
        .tiers
        .iter()
        .filter(|t| t.nome != fallito && t.brain != brand_fallito && !t.locale && libero(&t.nome))
        .map(|t| t.nome.clone())
        .collect();
    // Anche il locale puo' essere in pausa (llama-server giu'): filtrarlo qui
    // evita di riproporre un candidato che rifiuterebbe comunque.
    let locali: Vec<String> = cfg
        .tiers
        .iter()
        .filter(|t| t.locale && t.nome != fallito && libero(&t.nome))
        .map(|t| t.nome.clone())
        .collect();
    [altri, locali].concat()
}

/// Quanto dura davvero una pausa: mai meno di un minuto.
///
/// Un fornitore che dice «riprova fra tre secondi» manderebbe NOVA a
/// sbatterci contro in un ciclo stretto.
pub const PAUSA_MINIMA_S: i64 = 60;

pub fn durata_pausa(secondi_chiesti: i64) -> i64 {
    secondi_chiesti.max(PAUSA_MINIMA_S)
}

/// Un server che sta **sulla stessa macchina**.
///
/// Sta qui perche' e' la frase su cui NOVA sta in piedi ridotta a una
/// domanda sola: niente esce dal PC finche' qualcuno non delega davvero.
/// Ollama, LM Studio, llama.cpp, KoboldCpp non chiedono nessuna chiave, e
/// pretenderne una vorrebbe dire rifiutarsi di parlare con un cervello che
/// e' li', acceso e gratuito.
///
/// Si guarda **solo l'host**, che e' l'unica cosa che distingue «in casa» da
/// «su internet» — dove invece la chiave serve davvero. Non la porta, non lo
/// schema: un `https://` verso `localhost` e' comunque in casa, e un `http://`
/// verso un dominio non lo e'.
pub fn e_in_casa(base_url: &str) -> bool {
    const IN_CASA: [&str; 5] = [
        "localhost",
        "127.0.0.1",
        "::1",
        "0.0.0.0",
        "host.docker.internal",
    ];
    IN_CASA.contains(&host_di(base_url).as_str())
}

/// L'host di un URL, in minuscolo, come lo estrae `urlparse().hostname`.
///
/// Scritto a mano: portarsi dietro un analizzatore di URL per una domanda
/// sola vorrebbe dire aggiungere una dipendenza al pezzo del progetto che
/// decide **cosa esce dal PC**, ed e' il posto dove si vuole meno codice di
/// qualcun altro, non di piu'.
///
/// La regola non e' «quello che sta prima della prima barra». E' quella di
/// `urlsplit`, che ha una conseguenza che a occhio non si indovina:
/// **l'autorita' esiste solo dopo `//`**. `localhost:8080` senza schema non
/// ha host — `localhost` viene letto come **schema** — quindi non e' «in
/// casa», e la chiave verrebbe chiesta. Sembra sbagliato e non lo e': una
/// stringa cosi' non e' un URL, e indovinare cosa intendesse chi l'ha scritta
/// e' il genere di gentilezza che qui non si fa.
pub fn host_di(url: &str) -> String {
    // Python toglie prima gli spazi ai bordi e i caratteri di controllo
    // ovunque (tab e a capo), perche' un URL con dentro un a capo e' un modo
    // noto di far leggere due cose diverse a due programmi diversi.
    let pulito: String = url
        .trim_matches(|c: char| c.is_whitespace() || (c as u32) <= 0x20)
        .chars()
        .filter(|c| *c != '\t' && *c != '\n' && *c != '\r')
        .collect();

    // Lo schema c'e' solo se prima dei due punti c'e' un nome di schema
    // valido: comincia con una lettera, poi lettere, cifre, `+`, `-`, `.`.
    let resto = match pulito.find(':') {
        Some(i) if i > 0 => {
            let prefisso = &pulito[..i];
            let valido = prefisso.starts_with(|c: char| c.is_ascii_alphabetic())
                && prefisso.chars().all(|c| {
                    c.is_ascii_alphanumeric() || c == '+' || c == '-' || c == '.'
                });
            if valido { &pulito[i + 1..] } else { &pulito[..] }
        }
        _ => &pulito[..],
    };

    // L'autorita' esiste **solo** dopo `//`.
    let Some(dopo) = resto.strip_prefix("//") else {
        return String::new();
    };
    let autorita = dopo.split(['/', '?', '#']).next().unwrap_or("");

    // Le credenziali stanno prima dell'ultima chiocciola.
    let senza_credenziali = match autorita.rfind('@') {
        Some(i) => &autorita[i + 1..],
        None => autorita,
    };

    // Un indirizzo IPv6 sta fra parentesi quadre, e dentro ha i due punti.
    if let Some(r) = senza_credenziali.strip_prefix('[') {
        return match r.find(']') {
            Some(i) => r[..i].to_lowercase(),
            None => r.to_lowercase(),
        };
    }
    // Altrimenti i due punti separano la porta.
    senza_credenziali.split(':').next().unwrap_or("").to_lowercase()
}

#[cfg(test)]
mod prove {
    use super::*;

    fn g(nome: &str, brain: &str, locale: bool) -> Gradino {
        Gradino {
            nome: nome.into(),
            brain: brain.into(),
            locale,
            ..Default::default()
        }
    }

    fn cfg() -> Configurazione {
        Configurazione {
            tiers: vec![
                g("locale", "locale", true),
                g("standard", "claude", false),
                g("difficile", "claude", false),
                g("alternativo", "gemini", false),
            ],
            scala_dichiarata: vec![
                "locale".into(),
                "standard".into(),
                "difficile".into(),
                "alternativo".into(),
            ],
            escalation_automatica: true,
            ..Default::default()
        }
    }

    #[test]
    fn lordine_non_dipende_dalle_chiavi() {
        // Il difetto vero: un programma riscrive il file in ordine
        // alfabetico, «standard» finisce dopo «difficile», e sopra non c'e'
        // piu' niente a cui salire.
        let mut c = cfg();
        c.tiers.sort_by(|a, b| a.nome.cmp(&b.nome));
        assert_eq!(
            scala(&c),
            vec!["locale", "standard", "difficile", "alternativo"]
        );
    }

    #[test]
    fn un_gradino_aggiunto_a_mano_si_accoda_e_non_sparisce() {
        let mut c = cfg();
        c.tiers.push(g("mio", "api", false));
        let s = scala(&c);
        assert_eq!(s.last().unwrap(), "mio");
        assert_eq!(s.len(), 5);
    }

    #[test]
    fn sopra_lultimo_non_c_e_niente() {
        let c = cfg();
        assert_eq!(successivo(&c, "locale").as_deref(), Some("standard"));
        assert_eq!(successivo(&c, "alternativo"), None);
        assert_eq!(successivo(&c, "inesistente"), None);
        assert_eq!(indice(&c, "difficile"), 2);
        assert_eq!(indice(&c, "inesistente"), -1);
    }

    #[test]
    fn una_categoria_scritta_male_non_manda_fuori_tutto() {
        // Senza parole e senza soglia scatterebbe su qualunque compito: e' il
        // caso in cui una configurazione sbagliata farebbe uscire dal PC ogni
        // cosa. Si ignora.
        let mut c = cfg();
        c.categorie.push(Categoria {
            nome: "vuota".into(),
            attiva: true,
            gradino_minimo: "difficile".into(),
            ..Default::default()
        });
        assert_eq!(gradino_minimo(&c, "che ore sono", 0).0, "");
    }

    #[test]
    fn vince_il_gradino_piu_alto_fra_le_categorie_che_scattano() {
        let mut c = cfg();
        c.categorie.push(Categoria {
            nome: "dati".into(),
            attiva: true,
            gradino_minimo: "standard".into(),
            parole: vec!["cancella".into()],
            descrizione: "perdita di dati".into(),
            ..Default::default()
        });
        c.categorie.push(Categoria {
            nome: "architettura".into(),
            attiva: true,
            gradino_minimo: "difficile".into(),
            parole: vec!["architettura".into()],
            ..Default::default()
        });
        let (gr, _) = gradino_minimo(&c, "cancella e rivedi l'architettura", 0);
        assert_eq!(gr, "difficile");
        let (gr, motivo) = gradino_minimo(&c, "cancella il file", 0);
        assert_eq!(gr, "standard");
        assert_eq!(motivo, "perdita di dati");
    }

    #[test]
    fn spente_le_salite_non_sale_niente() {
        let mut c = cfg();
        c.escalation_automatica = false;
        c.categorie.push(Categoria {
            nome: "dati".into(),
            attiva: true,
            gradino_minimo: "difficile".into(),
            parole: vec!["cancella".into()],
            ..Default::default()
        });
        assert_eq!(gradino_minimo(&c, "cancella tutto", 0).0, "");
    }

    #[test]
    fn i_ripieghi_cambiano_fornitore_e_tornano_a_casa_per_ultimo() {
        let c = cfg();
        let pause = Pause::new();
        // «standard» e' claude: «difficile» e' lo stesso fornitore e non
        // serve, perche' il limite e' sul conto e non sul modello.
        let r = ripieghi(&c, "standard", &pause);
        assert_eq!(r, vec!["alternativo", "locale"]);
    }

    #[test]
    fn un_ripiego_in_pausa_non_si_propone() {
        let c = cfg();
        let mut pause = Pause::new();
        pause.insert("alternativo".into(), 300);
        assert_eq!(ripieghi(&c, "standard", &pause), vec!["locale"]);
    }

    #[test]
    fn solo_locale_chiude_la_porta() {
        let mut c = cfg();
        c.solo_locale = true;
        let pause = Pause::new();
        let mai = |_: &Gradino| false;
        assert!(utilizzabile(&c, "locale", &pause, 0.0, 0.0, &mai));
        assert!(!utilizzabile(&c, "standard", &pause, 0.0, 0.0, &mai));
    }

    #[test]
    fn il_tetto_ferma_prima_di_salire_non_dopo() {
        let mut c = cfg();
        c.tetto_usd_sessione = 1.0;
        c.costo_stimato_delega = 0.10;
        let pause = Pause::new();
        let paga = |t: &Gradino| !t.locale;
        assert!(utilizzabile(&c, "standard", &pause, 0.5, 0.0, &paga));
        assert!(!utilizzabile(&c, "standard", &pause, 0.95, 0.0, &paga));
        // Il locale non si paga: il tetto non lo tocca.
        assert!(utilizzabile(&c, "locale", &pause, 99.0, 0.0, &paga));
    }

    #[test]
    fn una_pausa_non_scende_mai_sotto_il_minuto() {
        // Un fornitore che dice «riprova fra tre secondi» manderebbe NOVA a
        // sbatterci contro in un ciclo stretto.
        assert_eq!(durata_pausa(3), 60);
        assert_eq!(durata_pausa(3600), 3600);
    }

    #[test]
    fn in_casa_e_su_internet() {
        assert!(e_in_casa("http://localhost:8080/v1"));
        assert!(e_in_casa("http://127.0.0.1:11434"));
        assert!(e_in_casa("https://LOCALHOST/v1"));
        assert!(e_in_casa("http://[::1]:8080/v1"));
        assert!(e_in_casa("http://0.0.0.0:5000"));
        assert!(e_in_casa("http://host.docker.internal:1234/v1"));
        assert!(!e_in_casa("https://api.openai.com/v1"));
        assert!(!e_in_casa("https://openrouter.ai/api/v1"));
        assert!(!e_in_casa(""));
        // Il caso che un confronto per sottostringa sbaglierebbe: un dominio
        // che **contiene** «localhost» non e' localhost.
        assert!(!e_in_casa("https://localhost.evil.example.com/v1"));
        assert!(!e_in_casa("https://notlocalhost/v1"));
    }

    #[test]
    fn lhost_si_estrae_come_lo_estrae_python() {
        assert_eq!(host_di("http://localhost:8080/v1"), "localhost");
        assert_eq!(host_di("https://API.OpenAI.com/v1"), "api.openai.com");
        assert_eq!(host_di("http://utente:parola@127.0.0.1:11434/x"), "127.0.0.1");
        assert_eq!(host_di("http://[::1]:8080/v1"), "::1");
        // Senza le due barre non c'e' autorita': «localhost» e' letto
        // come schema, e l'host resta vuoto.
        assert_eq!(host_di("localhost:8080"), "");
        assert_eq!(host_di("//localhost:8080/v1"), "localhost");
        assert_eq!(host_di(""), "");
        assert_eq!(host_di("http://esempio.it?a=1"), "esempio.it");
        assert_eq!(host_di("http://esempio.it#frammento"), "esempio.it");
    }
}
