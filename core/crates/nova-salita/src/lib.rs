//! Due modi di non farcela, e cosa si fa in ciascuno.
//!
//! **Sbattere contro un muro**: N chiamate di fila che falliscono. Qui si sale
//! di gradino, cioe' si passa il compito a un modello piu' capace. E' la
//! decisione piu' cara che NOVA prende da sola — un gradino piu' su spesso
//! vuol dire che il compito **esce dal PC** — quindi va presa con dei numeri,
//! non con un'impressione.
//!
//! **Girare a vuoto**: la stessa chiamata, con gli stessi argomenti, piu' e
//! piu' volte, e magari riuscendo ogni volta. Qui non c'e' niente da far
//! salire: c'e' da far **notare**. La differenza e' importante e sta nel
//! codice, non nei commenti: la ripetizione produce un promemoria, mai un
//! divieto. La decisione — riprovare diversamente, cercare altrove, o
//! concludere — resta al modello, e una ripetizione legittima non viene
//! bloccata da niente.
//!
//! ## Cosa sta fuori, e perche'
//!
//! Gli argomenti di una chiamata arrivano qui **gia' resi in testo**. Non e'
//! pigrizia: renderli sarebbe serializzazione, non decisione, e due
//! serializzatori diversi — `json.dumps` di Python e `serde_json` — scrivono
//! lo stesso oggetto con spaziature diverse. Inseguire l'uguaglianza a byte
//! di una stringa che non esce mai dal processo sarebbe lavoro speso per
//! niente, e sarebbe anche il posto sbagliato dove metterlo. Qui si decide
//! *se* e *cosa* dire; chi chiama porta il testo.

/// Le manopole del routing, con i valori di fabbrica di `routing_predefinito()`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Manopole {
    pub automatica: bool,
    pub fallimenti_prima_di_salire: u32,
    pub passi_prima_di_salire: u32,
    pub salite_massime: u32,
}

impl Default for Manopole {
    fn default() -> Self {
        Self {
            automatica: true,
            fallimenti_prima_di_salire: 2,
            passi_prima_di_salire: 4,
            salite_massime: 2,
        }
    }
}

/// Se e' il momento di passare la palla a un gradino piu' alto.
///
/// `passi` e' il numero di giri fatti finora: e' il secondo sintomo, quello
/// che fa davvero il modello locale — continuare a chiamare strumenti senza
/// mai arrivare a una risposta. Un `passi_prima_di_salire` a zero lo spegne,
/// e va spento e non «messo molto alto», se no la soglia esiste comunque.
pub fn serve_salire(m: &Manopole, fallimenti: u32, salite: u32, passi: u32) -> bool {
    if !m.automatica {
        return false;
    }
    if salite >= m.salite_massime {
        return false;
    }
    if fallimenti >= m.fallimenti_prima_di_salire {
        return true;
    }
    m.passi_prima_di_salire != 0 && passi >= m.passi_prima_di_salire
}

/// Alla terza, alla quinta e all'ottava di fila si dice qualcosa. In mezzo no.
///
/// Non a ogni ripetizione: un promemoria a ogni giro diventa rumore, e il
/// rumore si impara a saltare. Tre soglie e poi silenzio.
pub const SOGLIE_RIPETIZIONE: [u32; 3] = [3, 5, 8];

/// Gli strumenti di servizio non spezzano la catena.
///
/// Se contassero, basterebbe un `get_datetime` in mezzo per ripulire un ciclo
/// e renderlo invisibile — cioe' il modo piu' facile di girare a vuoto senza
/// che nessuno lo dica.
pub const TRASPARENTI: [&str; 3] = ["get_datetime", "kb_stats", "modelli"];

pub fn trasparente(nome: &str) -> bool {
    TRASPARENTI.contains(&nome)
}

/// Il promemoria per la `n`-esima ripetizione di fila, se ne va detto uno.
///
/// `breve` sono gia' gli argomenti resi in testo e accorciati da chi chiama.
pub fn promemoria(n: u32, nome: &str, breve: &str) -> Option<String> {
    if !SOGLIE_RIPETIZIONE.contains(&n) {
        return None;
    }
    if n == SOGLIE_RIPETIZIONE[0] {
        return Some(format!(
            "\n\n[nota di sistema] Hai chiamato {n} volte di fila la stessa \
             cosa con gli stessi argomenti. Rileggi il risultato che hai \
             gia': se non ti sta dando quello che cerchi, cambia strada o \
             concludi con quello che sai."
        ));
    }
    Some(format!(
        "\n\n[nota di sistema] «{nome}» con gli stessi argomenti per la \
         {n}ª volta di fila ({breve}). Continuare a ripeterla non cambiera' \
         il risultato. Rileggi cosa ti ha gia' risposto, poi prova un \
         approccio diverso oppure rispondi all'utente con quello che hai."
    ))
}

// ------------------------------------------------------------- il giro
//
// Il contatore qui sotto ha sempre saputo riconoscere **un** modo di girare a
// vuoto: la stessa chiamata, identica, piu' volte di fila. E' il giro piu'
// stupido, ed e' l'unico che si vedeva.
//
// Quello vero e' un altro. Un modello che non sa come uscirne alterna:
// cerca, leggi, cerca, leggi, cerca, leggi. Ogni chiamata e' diversa dalla
// precedente, quindi la catena si azzerava a ogni passo e il contatore
// restava a uno **per sempre**. Dodici passi di lavoro inutile, nessun
// promemoria, e l'utente che guarda NOVA girare.
//
// La regola resta quella scritta in testa a questo file: si fa **notare**,
// non si vieta. Un giro riconosciuto produce una frase, e la decisione —
// cambiare strada o concludere — resta al modello.

/// Quante impronte si tengono per riconoscere un giro.
///
/// Il giro piu' lungo che si riconosce e' di quattro chiamate, ripetuto
/// cinque volte: venti. Tenerne di piu' non servirebbe a niente.
pub const MEMORIA_DEL_GIRO: usize = 20;

/// Il giro piu' lungo che si riconosce.
///
/// Oltre le quattro chiamate non e' piu' un giro: e' un piano. Riconoscere
/// cicli lunghissimi vorrebbe dire chiamare «giro a vuoto» un lavoro vero che
/// per caso si ripete, ed e' il modo piu' rapido di far ignorare l'avviso.
pub const PERIODO_MASSIMO: usize = 4;

/// Dopo quanti giri completi si dice qualcosa.
pub const GIRI_PRIMA_DI_DIRLO: [usize; 2] = [3, 5];

/// Il giro in fondo a questa storia: (quante chiamate lo compongono, quante
/// volte si e' ripetuto).
///
/// Si cerca il periodo **piu' corto** che spieghi la coda: `A B A B A B` e'
/// un giro di due ripetuto tre volte, non uno di sei fatto una volta. E un
/// periodo dove tutte le chiamate sono uguali non conta: quello e' il giro
/// stupido, e lo dice gia' il contatore delle ripetizioni di fila.
pub fn ciclo(storia: &[String]) -> Option<(usize, usize)> {
    for periodo in 2..=PERIODO_MASSIMO {
        if storia.len() < periodo * 2 {
            break;
        }
        let coda = &storia[storia.len() - periodo..];
        if coda.iter().all(|x| x == &coda[0]) {
            continue;
        }
        let mut giri = 1;
        while storia.len() >= periodo * (giri + 1) {
            let fine = storia.len() - periodo * giri;
            let prima = &storia[fine - periodo..fine];
            if prima != coda {
                break;
            }
            giri += 1;
        }
        if giri >= GIRI_PRIMA_DI_DIRLO[0] {
            return Some((periodo, giri));
        }
    }
    None
}

/// Lo stesso giro visto da un punto diverso e' lo stesso giro.
///
/// `A B A B A B A` contiene `AB` e anche `BA`: sono la stessa ruota, girata
/// di un passo. Si ruota finche' la piu' piccola sta davanti, cosi' due letture
/// dello stesso giro si riconoscono uguali.
pub fn canonico(giro: &[String]) -> String {
    let Some(minimo) = giro.iter().enumerate().min_by(|a, b| a.1.cmp(b.1)).map(|(i, _)| i) else {
        return String::new();
    };
    let mut ruotato: Vec<&String> = giro[minimo..].iter().collect();
    ruotato.extend(giro[..minimo].iter());
    ruotato
        .into_iter()
        .map(|s| s.as_str())
        .collect::<Vec<_>>()
        .join("\u{1}")
}

/// Cosa si dice a chi sta girando in tondo, se va detto adesso.
///
/// `nomi` sono le chiamate del giro, in ordine: servono a farglielo vedere.
/// Dirgli «stai girando» senza dirgli **in cosa** e' un rimprovero; dirgli
/// «cerca → leggi → cerca → leggi» e' un'informazione.
pub fn promemoria_del_giro(periodo: usize, giri: usize, nomi: &[String]) -> Option<String> {
    if !GIRI_PRIMA_DI_DIRLO.contains(&giri) {
        return None;
    }
    let catena = nomi.join(" → ");
    Some(format!(
        "\n\n[nota di sistema] Stai girando in tondo: le stesse {periodo} \
         chiamate nello stesso ordine, {giri} volte di fila ({catena}). \
         Ripeterle non cambiera' il risultato. Rileggi cosa ti hanno gia' \
         risposto, poi cambia strada oppure rispondi con quello che hai."
    ))
}

// ------------------------------------------------------ i passi finiti
//
// Il terzo modo di non farcela, e finora non stava scritto da nessuna parte:
// **finire i passi**. Il ciclo ne ha un tetto — dodici — e quando li esaurisce
// diceva questo:
//
//     Ho raggiunto il numero massimo di passaggi consentiti.
//     Dimmi come vuoi che proceda.
//
// e tornava **quella frase al posto di cio' che aveva gia' scritto**. Dodici
// passi di lavoro: pagine lette, file aperti, pezzi di risposta messi giu'
// lungo la strada — e all'utente arriva una riga burocratica che non dice
// nemmeno cosa aveva trovato.
//
// E' la stessa forma del taglio dei risultati, che questo progetto ha gia'
// curato una volta: una perdita silenziosa deve diventare un rinvio. Quel che
// c'e' si consegna, e si dice perche' ci si e' fermati.

/// Cosa si dice quando i passi sono finiti.
///
/// `gia_detto` e' l'ultimo testo che il modello aveva scritto: se c'e', si
/// consegna, e il tetto diventa una nota accanto invece di una sostituzione.
pub fn passi_finiti(quanti: u32, gia_detto: &str) -> String {
    let avanzo = gia_detto.trim();
    if avanzo.is_empty() {
        return format!(
            "Ho fatto {quanti} passaggi senza arrivare a una risposta, e mi \
             fermo qui invece di continuare all'infinito. Dimmi come vuoi che \
             proceda."
        );
    }
    format!(
        "{avanzo}\n\n[Mi sono fermato dopo {quanti} passaggi: e' il tetto che \
         ho da solo. Quello qui sopra e' quanto sono riuscito a mettere \
         insieme. Se non basta, dimmi come vuoi che proceda.]"
    )
}

/// Quante volte di fila e' arrivata la stessa identica chiamata.
#[derive(Debug, Default, Clone)]
pub struct Contatore {
    ultima: Option<String>,
    quante: u32,
    /// Le ultime impronte, per riconoscere i giri che non sono di fila.
    storia: Vec<String>,
    /// I nomi corrispondenti: servono solo a scrivere la frase.
    nomi: Vec<String>,
    /// L'ultimo giro gia' segnalato, in forma canonica, e a quale conto.
    ///
    /// Senza questo il promemoria arriva a **ogni passo**: `A B A B A B A`
    /// contiene il giro `AB` tre volte e anche il giro `BA` tre volte, e da
    /// li' in poi ogni chiamata ne chiude uno. Un avviso che arriva sempre e'
    /// un avviso che non legge piu' nessuno.
    detto: Option<(String, usize)>,
}

impl Contatore {
    pub fn nuovo() -> Self {
        Self::default()
    }

    /// L'impronta e' nome piu' argomenti resi: due chiamate con gli stessi
    /// argomenti in ordine diverso devono avere la stessa impronta, e questo
    /// e' compito di chi rende gli argomenti (in Python, `sort_keys=True`).
    pub fn impronta(nome: &str, argomenti_resi: &str) -> String {
        format!("{nome}\u{0}{argomenti_resi}")
    }

    /// Guarda una chiamata e dice cosa eventualmente ricordare al modello.
    ///
    /// Uno strumento trasparente esce **prima** di toccare il contatore: non
    /// azzera la catena e non la fa nemmeno avanzare.
    pub fn guarda(&mut self, nome: &str, argomenti_resi: &str, breve: &str) -> Option<String> {
        if trasparente(nome) {
            return None;
        }
        let impronta = Self::impronta(nome, argomenti_resi);
        if self.ultima.as_deref() == Some(impronta.as_str()) {
            self.quante += 1;
        } else {
            self.ultima = Some(impronta.clone());
            self.quante = 1;
        }
        self.storia.push(impronta);
        self.nomi.push(nome.to_string());
        if self.storia.len() > MEMORIA_DEL_GIRO {
            self.storia.remove(0);
            self.nomi.remove(0);
        }
        // Prima il giro di fila: e' il caso piu' preciso, e due promemoria
        // nello stesso passo sarebbero rumore.
        if let Some(p) = promemoria(self.quante, nome, breve) {
            return Some(p);
        }
        let (periodo, giri) = ciclo(&self.storia)?;
        let quale = canonico(&self.storia[self.storia.len() - periodo..]);
        if self.detto.as_ref() == Some(&(quale.clone(), giri)) {
            return None;
        }
        let nomi = &self.nomi[self.nomi.len() - periodo..];
        let frase = promemoria_del_giro(periodo, giri, nomi)?;
        self.detto = Some((quale, giri));
        Some(frase)
    }

    pub fn quante(&self) -> u32 {
        self.quante
    }
}

#[cfg(test)]
mod prove {
    use super::*;

    #[test]
    fn si_sale_dopo_i_fallimenti_di_fila() {
        let m = Manopole::default();
        assert!(!serve_salire(&m, 0, 0, 0));
        assert!(!serve_salire(&m, 1, 0, 0));
        assert!(serve_salire(&m, 2, 0, 0), "due fallimenti bastano");
    }

    #[test]
    fn si_sale_anche_solo_girando_a_vuoto() {
        let m = Manopole::default();
        assert!(!serve_salire(&m, 0, 0, 3));
        assert!(serve_salire(&m, 0, 0, 4), "quattro passi senza risposta");
    }

    #[test]
    fn ma_non_oltre_le_salite_massime() {
        let m = Manopole::default();
        assert!(serve_salire(&m, 9, 1, 9));
        assert!(!serve_salire(&m, 9, 2, 9), "due salite sono il tetto");
    }

    #[test]
    fn spenta_vuol_dire_spenta() {
        let m = Manopole { automatica: false, ..Manopole::default() };
        assert!(!serve_salire(&m, 99, 0, 99));
    }

    #[test]
    fn zero_passi_spegne_la_soglia_dei_passi_non_la_accende() {
        // Il caso che rovina un `>=` scritto senza pensarci: con la soglia a
        // zero, «passi >= 0» sarebbe sempre vero e si salirebbe al primo giro.
        let m = Manopole { passi_prima_di_salire: 0, ..Manopole::default() };
        assert!(!serve_salire(&m, 0, 0, 0));
        assert!(!serve_salire(&m, 0, 0, 100));
        assert!(serve_salire(&m, 2, 0, 0), "i fallimenti contano ancora");
    }

    #[test]
    fn si_parla_alla_terza_alla_quinta_e_allottava() {
        for n in 0..=10 {
            let detto = promemoria(n, "list_directory", "{}").is_some();
            assert_eq!(detto, [3, 5, 8].contains(&n), "n = {n}");
        }
    }

    #[test]
    fn la_catena_conta_solo_le_chiamate_identiche() {
        let mut c = Contatore::nuovo();
        assert!(c.guarda("list_directory", "{\"path\":\"C\"}", "…").is_none());
        assert!(c.guarda("list_directory", "{\"path\":\"C\"}", "…").is_none());
        assert!(c.guarda("list_directory", "{\"path\":\"C\"}", "…").is_some(), "terza");
        // argomenti diversi: si riparte da uno
        assert!(c.guarda("list_directory", "{\"path\":\"D\"}", "…").is_none());
        assert_eq!(c.quante(), 1);
    }

    #[test]
    fn uno_strumento_di_servizio_non_ripulisce_il_ciclo() {
        let mut c = Contatore::nuovo();
        c.guarda("list_directory", "a", "…");
        c.guarda("list_directory", "a", "…");
        // in mezzo passa un tool trasparente: non deve azzerare niente
        assert!(c.guarda("get_datetime", "{}", "…").is_none());
        assert_eq!(c.quante(), 2, "il contatore non si e' mosso");
        assert!(c.guarda("list_directory", "a", "…").is_some(), "e la terza arriva");
    }

    #[test]
    fn dopo_lottava_si_tace() {
        let mut c = Contatore::nuovo();
        let mut detti = 0;
        for _ in 0..15 {
            if c.guarda("read_file", "x", "…").is_some() {
                detti += 1;
            }
        }
        assert_eq!(detti, 3, "tre promemoria in quindici giri, poi silenzio");
    }
}

#[cfg(test)]
mod prove_del_giro {
    use super::*;

    fn seq(c: &mut Contatore, chiamate: &[&str]) -> Vec<Option<String>> {
        chiamate
            .iter()
            .map(|n| {
                let (nome, arg) = n.split_once(':').unwrap_or((n, ""));
                c.guarda(nome, arg, arg)
            })
            .collect()
    }

    #[test]
    fn il_giro_di_due_si_riconosce_alla_terza_volta() {
        // Il caso vero: cerca, leggi, cerca, leggi, cerca, leggi. Ogni
        // chiamata e' diversa dalla precedente, quindi il contatore delle
        // ripetizioni di fila resta a uno per sempre.
        let mut c = Contatore::nuovo();
        let r = seq(&mut c, &["cerca:x", "leggi:y", "cerca:x", "leggi:y",
                              "cerca:x", "leggi:y"]);
        assert!(r[..5].iter().all(|x| x.is_none()), "{r:?}");
        let detto = r[5].as_deref().expect("il giro non e' stato visto");
        assert!(detto.contains("girando in tondo"), "{detto}");
        assert!(detto.contains("cerca → leggi"), "deve far vedere il giro: {detto}");
        assert_eq!(c.quante(), 1, "di fila non si e' ripetuto niente");
    }

    #[test]
    fn e_lo_ridice_al_quinto_giro_non_a_ogni_passo() {
        let mut c = Contatore::nuovo();
        let mut quanti = 0;
        for i in 0..12 {
            let nome = if i % 2 == 0 { "cerca" } else { "leggi" };
            if c.guarda(nome, "x", "x").is_some() {
                quanti += 1;
            }
        }
        assert_eq!(quanti, 2, "un promemoria al terzo giro e uno al quinto");
    }

    #[test]
    fn un_lavoro_vero_con_argomenti_diversi_non_e_un_giro() {
        // leggi(a) scrivi(a) leggi(b) scrivi(b): i nomi si ripetono, gli
        // argomenti no. Non e' un giro, e' lavoro.
        let mut c = Contatore::nuovo();
        let r = seq(&mut c, &["leggi:a", "scrivi:a", "leggi:b", "scrivi:b",
                              "leggi:c", "scrivi:c", "leggi:d", "scrivi:d"]);
        assert!(r.iter().all(|x| x.is_none()), "{r:?}");
    }

    #[test]
    fn due_giri_soli_non_bastano() {
        let mut c = Contatore::nuovo();
        let r = seq(&mut c, &["cerca:x", "leggi:y", "cerca:x", "leggi:y"]);
        assert!(r.iter().all(|x| x.is_none()), "quattro chiamate non sono un giro: {r:?}");
    }

    #[test]
    fn il_giro_di_tre_si_riconosce() {
        let mut c = Contatore::nuovo();
        let r = seq(&mut c, &["a:1", "b:2", "c:3", "a:1", "b:2", "c:3",
                              "a:1", "b:2", "c:3"]);
        let detto = r[8].as_deref().expect("giro di tre non visto");
        assert!(detto.contains("le stesse 3"), "{detto}");
        assert!(detto.contains("a → b → c"), "{detto}");
    }

    #[test]
    fn la_stessa_chiamata_di_fila_resta_il_caso_di_prima() {
        // Non deve arrivare il promemoria del giro: quello di fila e' piu'
        // preciso, e due frasi nello stesso passo sono rumore.
        let mut c = Contatore::nuovo();
        let r = seq(&mut c, &["cerca:x", "cerca:x", "cerca:x"]);
        let detto = r[2].as_deref().expect("la terza di fila si dice");
        assert!(detto.contains("volte di fila"), "{detto}");
        assert!(!detto.contains("girando in tondo"), "{detto}");
    }

    #[test]
    fn uscire_dal_giro_lo_spegne() {
        let mut c = Contatore::nuovo();
        seq(&mut c, &["cerca:x", "leggi:y", "cerca:x", "leggi:y", "cerca:x", "leggi:y"]);
        // Cambia strada davvero: il giro si interrompe e non si ridice piu'.
        let r = seq(&mut c, &["scrivi:z", "finisci:w", "manda:v", "chiudi:u"]);
        assert!(r.iter().all(|x| x.is_none()), "{r:?}");
    }

    #[test]
    fn ciclo_trova_il_periodo_piu_corto() {
        let s: Vec<String> = ["a", "b", "a", "b", "a", "b"]
            .iter().map(|x| x.to_string()).collect();
        assert_eq!(ciclo(&s), Some((2, 3)));
        let vuota: Vec<String> = Vec::new();
        assert_eq!(ciclo(&vuota), None);
    }

    #[test]
    fn uno_strumento_trasparente_non_spezza_il_giro() {
        // `get_datetime` esce prima di toccare qualunque contatore: chiederlo
        // in mezzo a un giro non deve renderlo invisibile.
        let mut c = Contatore::nuovo();
        let r = seq(&mut c, &["cerca:x", "get_datetime:", "leggi:y",
                              "cerca:x", "get_datetime:", "leggi:y",
                              "cerca:x", "get_datetime:", "leggi:y"]);
        assert!(r.iter().any(|x| x.is_some()), "il giro si e' nascosto dietro l'ora");
    }
}

#[cfg(test)]
mod prove_dei_passi {
    use super::*;

    #[test]
    fn quel_che_ha_gia_scritto_si_consegna() {
        // Il difetto: dodici passi di lavoro, e la riga del tetto prendeva il
        // posto di cio' che aveva trovato.
        let d = passi_finiti(12, "Ho trovato tre listoni aggiornati a ieri.");
        assert!(d.starts_with("Ho trovato tre listoni"), "{d}");
        assert!(d.contains("12 passaggi"), "{d}");
        assert!(d.contains("Se non basta"), "{d}");
    }

    #[test]
    fn e_se_non_ha_scritto_niente_lo_dice_e_basta() {
        let d = passi_finiti(12, "   \n  ");
        assert!(d.contains("12 passaggi"), "{d}");
        assert!(d.contains("senza arrivare a una risposta"), "{d}");
        assert!(!d.contains("qui sopra"), "non c'e' niente sopra: {d}");
    }

    #[test]
    fn il_numero_e_quello_vero_non_una_costante() {
        assert!(passi_finiti(4, "").contains("4 passaggi"));
        assert!(passi_finiti(30, "x").contains("30 passaggi"));
    }

    #[test]
    fn non_e_un_rimprovero_e_lascia_la_mano_allutente() {
        for testo in [passi_finiti(12, ""), passi_finiti(12, "qualcosa")] {
            let b = testo.to_lowercase();
            for vietato in ["non posso", "errore", "fallito", "impossibile"] {
                assert!(!b.contains(vietato), "{testo}");
            }
            assert!(b.contains("dimmi come vuoi"), "{testo}");
        }
    }
}
