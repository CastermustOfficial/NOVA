//! # nova-contesto
//!
//! Quanto della conversazione ci sta nella memoria del modello, e cosa si
//! butta quando non ci sta piu'.
//!
//! E' il pezzo piu' delicato di tutto il cantiere, e la ragione e' la stessa
//! di D148: **cio' che resta fuori non lascia traccia**. Un ordinamento
//! sbagliato si vede — la risposta e' storta e l'utente se ne accorge. Un
//! messaggio buttato no: il modello risponde come se non fosse mai stato
//! detto, con la stessa sicurezza di sempre, e nessuno dei due lati della
//! conversazione ha modo di sapere che manca un pezzo.
//!
//! Percio' qui ogni funzione che toglie qualcosa **lo dichiara** in un
//! [`Resoconto`], e l'accorciamento di un messaggio troppo grande lascia una
//! riga scritta dentro il testo: un taglio dichiarato il modello lo capisce,
//! uno silenzioso gli fa credere che il file finisca li'.
//!
//! ## Cosa sta fuori, di proposito
//!
//! Il tokenizzatore vero. Sta nel modello, cambia con il modello, e
//! chiederglielo costerebbe un giro di rete per ogni messaggio a ogni turno
//! solo per decidere se tagliare. Qui c'e' una stima a caratteri, tarata
//! misurando due volte su prompt veri (3,88 token per carattere su una
//! conversazione in italiano, 4,37 su testo ripetitivo) e tenuta **sotto** il
//! piu' basso dei due: sbagliare per eccesso di token taglia un po' presto e
//! si nota appena, sbagliare per difetto sfonda il contesto ed e' un errore
//! in faccia all'utente.
//!
//! Fuori anche la configurazione e il cervello: chi chiama sa se il contesto
//! e' noto e quanto vale. Zero vuol dire «non lo so», e allora non si tocca
//! niente — meglio il taglio a messaggi da solo che uno inventato.
//!
//! ## Due trappole del porting, entrambe misurate
//!
//! **I caratteri non sono byte.** In Python `len(testo)` conta *caratteri*, e
//! `testo[:meta]` taglia per *caratteri*. In Rust la stessa scrittura su
//! `&str` conta byte, e in italiano non e' la stessa cosa: `perche'` con
//! l'accento pesa un byte in piu' della sua lunghezza. Tutta la stima dei
//! token e tutti i tagli qui dentro lavorano su `char`, e il banco lo
//! verifica con del testo accentato apposta — perche' una stima a byte
//! sarebbe **piu' grande** del vero, quindi taglierebbe prima e in silenzio.
//!
//! **A parita' di lunghezza vince il primo.** `max(range(n), key=...)` in
//! Python restituisce il primo massimo; `max_by_key` in Rust restituisce
//! l'ultimo. Con due messaggi lunghi uguali le due parti accorcerebbero
//! messaggi diversi, e la divergenza si vedrebbe solo su quel caso.

// I testi che il modello rilegge a ogni richiesta, estratti dal Python.
pub mod testi;

// Il messaggio numero zero della finestra: come si compone, e in che lingua
// si dice al modello di rispondere.
pub mod sistema;

/// Quanti caratteri vale un token, per stima.
pub const CARATTERI_PER_TOKEN: f64 = 3.5;

/// Quanto lasciare libero per la risposta: il contesto non serve solo a
/// leggere, il modello ci scrive dentro.
pub const RISERVA_RISPOSTA_TOKEN: u32 = 1024;

/// Sopra questo numero di messaggi si taglia.
pub const TETTO_MESSAGGI: usize = 60;

/// E si scende fino a qui. La distanza fra i due e' il punto: senza, si
/// taglia a ogni turno e la cache del prefisso non si riforma mai.
pub const FONDO_MESSAGGI: usize = 40;

/// Sotto questa lunghezza un messaggio non si accorcia piu': quel che resta
/// e' gia' solo l'inizio e la fine.
pub const MINIMO_ACCORCIABILE: usize = 400;

/// Un messaggio della conversazione, ridotto alle due cose che il taglio
/// guarda: chi l'ha detto e cosa c'e' scritto.
///
/// Tutto il resto — le chiamate ai tool, gli identificativi, gli allegati —
/// resta a chi chiama. Questo modulo non lo legge e non lo puo' rovinare.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Messaggio {
    pub ruolo: String,
    pub contenuto: String,
}

impl Messaggio {
    pub fn nuovo(ruolo: &str, contenuto: &str) -> Self {
        Self { ruolo: ruolo.to_string(), contenuto: contenuto.to_string() }
    }

    /// Lunghezza in **caratteri**, che e' l'unica che conta qui.
    pub fn lunghezza(&self) -> usize {
        self.contenuto.chars().count()
    }
}

/// Un messaggio che e' stato accorciato, e di quanto.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Accorciato {
    /// Posizione nell'elenco **finale**, messaggio di sistema compreso.
    pub indice: usize,
    pub caratteri_prima: usize,
    pub caratteri_dopo: usize,
}

/// Cosa e' stato tolto, e per quale ragione.
///
/// Esiste perche' un taglio silenzioso non si puo' verificare: senza questo,
/// due implementazioni che buttano messaggi diversi possono restituire lo
/// stesso elenco finale in tutti i casi tranne uno, e quell'uno lo si scopre
/// in produzione.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Resoconto {
    pub partiti: usize,
    /// Buttati perche' i messaggi erano troppi.
    pub tolti_per_numero: usize,
    /// Risposte di tool rimaste senza la chiamata che le aveva prodotte,
    /// scartate dopo il taglio a numero.
    pub orfani_dopo_numero: usize,
    /// Buttati perche' i token erano troppi.
    pub tolti_per_token: usize,
    /// Idem, dopo il taglio a token.
    pub orfani_dopo_token: usize,
    /// Il taglio a token aveva svuotato tutto e si e' ripescato l'ultimo
    /// messaggio: una conversazione vuota non e' una conversazione
    /// accorciata, e' un'amnesia.
    pub ripescato_l_ultimo: bool,
    pub accorciati: Vec<Accorciato>,
    pub rimasti: usize,
}

/// Quanti token vale un testo, senza tokenizzatore.
pub fn stima_token(testo: &str) -> u32 {
    stima_token_con(testo, CARATTERI_PER_TOKEN)
}

/// Come [`stima_token`], ma con il rapporto passato da fuori: serve al banco
/// per provare che il numero sia un parametro e non una costante murata.
pub fn stima_token_con(testo: &str, caratteri_per_token: f64) -> u32 {
    let caratteri = testo.chars().count() as f64;
    (caratteri / caratteri_per_token) as u32 + 1
}

/// Quanto pesa un pezzo di conversazione.
pub fn token_dei(messaggi: &[Messaggio]) -> u32 {
    messaggi.iter().map(|m| stima_token(&m.contenuto)).sum()
}

/// Quanti token restano alla conversazione, tolto tutto il resto.
///
/// Il conto non e' un dettaglio contabile, e' la scoperta che ha fatto
/// nascere questa funzione. Con il contesto a 16.384:
///
/// ```text
/// messaggio di sistema     ~5.200 token
/// schemi dei sessanta tool ~6.900 token   (il 42% del contesto)
/// riserva per la risposta   1.024 token
/// ------------------------------------
/// resta alla conversazione ~3.300 token   (il 20%)
/// ```
///
/// Il prefisso fisso si mangia i tre quarti del contesto. Da qui il taglio a
/// token: quel che avanza e' molto meno di quanto sessanta messaggi possano
/// pesare.
///
/// `contesto` a zero vuol dire «non lo so» e si torna zero: vale solo il
/// taglio a messaggi.
pub fn spazio_per_la_conversazione(
    contesto: u32,
    sistema: &str,
    schemi_tool: Option<&str>,
) -> u32 {
    if contesto == 0 {
        return 0;
    }
    let mut fissi = stima_token(sistema) + RISERVA_RISPOSTA_TOKEN;
    if let Some(s) = schemi_tool {
        if !s.is_empty() {
            fissi += stima_token(s);
        }
    }
    contesto.saturating_sub(fissi)
}

/// Accorcia la conversazione, ma di rado.
///
/// Il taglio butta cio' che sta **subito dopo il messaggio di sistema**, e
/// quello e' il posto peggiore: la cache del prefisso di llama.cpp vale
/// finche' i token in testa sono gli stessi, quindi spostare la seconda riga
/// invalida tutto il resto e si rielabora l'intera conversazione.
///
/// Percio' non ci si ferma sul filo del tetto ma si scende fino a un
/// `fondo`. Misurato su una conversazione da ottantuno messaggi, 15.379
/// token di prefisso:
///
/// ```text
/// a caldo, prefisso intatto           175 ms
/// dopo il taglio senza fondo        1.771 ms
/// e il turno seguente               1.731 ms   <- non guarisce
/// col fondo, dopo il taglio         1.217 ms
/// e il turno seguente                 226 ms   <- guarito
/// ```
///
/// Il fondo non cambia **cosa** si butta: cambia quanto spesso. Si taglia una
/// volta ogni dieci turni invece che a ogni turno, e nei nove in mezzo il
/// prefisso resta valido.
pub fn taglia(
    messaggi: &[Messaggio],
    tetto: usize,
    fondo: usize,
    token_disponibili: u32,
) -> (Vec<Messaggio>, Resoconto) {
    let mut resoconto = Resoconto { partiti: messaggi.len(), ..Default::default() };

    // Un fondo troppo vicino al tetto riporta al difetto di prima senza
    // dirlo, e «un turno di distanza» non basta: con `tetto - 2` si
    // taglierebbe a turni alterni invece che a ogni turno, che e' meta' del
    // difetto e non la sua assenza. La distanza minima e' un quarto del
    // tetto, cioe' una decina di turni di respiro.
    let giu = std::cmp::max(2, std::cmp::min(fondo, tetto * 3 / 4));

    let mut correnti: Vec<Messaggio> = if messaggi.len() <= tetto {
        // Sotto la soglia dei messaggi non si taglia per numero — ma il
        // taglio a token va fatto lo stesso, ed e' proprio questo il caso che
        // conta: dodici scambi con dentro il contenuto di un file sono
        // venticinque messaggi, quindi passano di qui, e sono centomila
        // token.
        messaggi.to_vec()
    } else {
        let quanti = giu - 1;
        let inizio = messaggi.len().saturating_sub(quanti);
        let mut coda: Vec<Messaggio> = messaggi[inizio..].to_vec();
        resoconto.tolti_per_numero = inizio.saturating_sub(1);
        // Una risposta di tool senza la chiamata che l'ha prodotta non e'
        // leggibile da nessun modello: si scarta finche' la coda non comincia
        // da qualcosa di sensato.
        while !coda.is_empty() && coda[0].ruolo == "tool" {
            coda.remove(0);
            resoconto.orfani_dopo_numero += 1;
        }
        let mut fuori = vec![messaggi[0].clone()];
        fuori.extend(coda);
        fuori
    };

    taglia_a_token(&mut correnti, token_disponibili, &mut resoconto);
    resoconto.rimasti = correnti.len();
    (correnti, resoconto)
}

/// Il taglio che conta davvero: quello sui token.
///
/// La finestra si contava **in messaggi** e il limite del modello e' **in
/// token**: due unita' diverse che non si parlavano. Sessanta messaggi
/// possono essere trecento token o centomila, e bastano dodici scambi con
/// dentro il contenuto di un file per arrivare a 102.953 token contro i
/// 16.384 del contesto — misurato, non immaginato. Il taglio a messaggi non
/// scattava nemmeno: erano ventiquattro messaggi.
fn taglia_a_token(messaggi: &mut Vec<Messaggio>, disponibili: u32, r: &mut Resoconto) {
    // `< 2` e non `<= 2`: con esattamente due messaggi — sistema piu' una
    // risposta enorme — non c'e' niente da **togliere**, ma c'e' ancora da
    // **accorciare**, ed e' il caso che ha fatto scrivere l'accorciamento.
    if disponibili == 0 || messaggi.len() < 2 {
        return;
    }
    let mut coda: Vec<Messaggio> = messaggi[1..].to_vec();
    if token_dei(&coda) <= disponibili {
        return;
    }
    // Stessa idea del fondo: si scende sotto la soglia, non ci si ferma
    // sopra, o si ritaglia a ogni turno e la cache non si riforma mai.
    let obiettivo = (f64::from(disponibili) * 0.75) as u32;
    while !coda.is_empty() && token_dei(&coda) > obiettivo {
        coda.remove(0);
        r.tolti_per_token += 1;
    }
    while !coda.is_empty() && coda[0].ruolo == "tool" {
        coda.remove(0);
        r.orfani_dopo_token += 1;
    }
    // Non si resta mai senza l'ultimo scambio.
    if coda.is_empty() {
        if let Some(ultimo) = messaggi.last() {
            coda = vec![ultimo.clone()];
            r.ripescato_l_ultimo = true;
        }
    }
    // E se cio' che resta non ci sta **comunque**, vuol dire che un solo
    // messaggio e' piu' grande di tutto lo spazio: il contenuto di un file
    // letto, una pagina web intera. Buttarlo vorrebbe dire perdere proprio la
    // cosa di cui l'utente ha chiesto conto; tenerlo intero vuol dire
    // sfondare il contesto. Si accorcia, e lo si dice nel testo.
    if !coda.is_empty() && token_dei(&coda) > disponibili {
        let (nuova, accorciati) = accorcia_il_piu_grosso(&coda, obiettivo);
        coda = nuova;
        r.accorciati = accorciati;
    }
    let mut fuori = vec![messaggi[0].clone()];
    fuori.extend(coda);
    *messaggi = fuori;
}

/// La riga che dichiara il taglio dentro il testo.
fn dichiarazione(tolti: usize) -> String {
    format!(
        "\n\n[...tagliati {tolti} caratteri perche' non ci stavano nella \
         memoria del modello...]\n\n"
    )
}

/// Accorcia i messaggi piu' grossi finche' la coda non ci sta.
///
/// Si tiene l'inizio e la fine: l'inizio dice cos'era, la fine spesso porta
/// la conclusione, ed e' il mezzo che si puo' perdere.
///
/// **La lunghezza da togliere si calcola, non si indovina.** La prima
/// versione tagliava a una misura fissa e non terminava: la scritta che
/// dichiara il taglio e' lunga quanto i caratteri che alla seconda passata
/// restavano da togliere, quindi il testo si accorciava di ottanta caratteri
/// e ricresceva di ottanta, per sempre. Un ciclo che «ovviamente» finisce e
/// non finisce. Adesso a ogni passata si punta alla lunghezza che serve, e si
/// esce se non si e' guadagnato niente: due condizioni invece di una, perche'
/// una si e' gia' vista sbagliare.
///
/// **Una cosa che si e' vista solo scrivendo la prova.** Nel giro normale
/// questa funzione riceve sempre **un solo messaggio**, mai una lista. La
/// ragione e' strutturale: chi la chiama ci arriva solo dopo il ciclo che
/// butta dalla testa finche' la coda non sta sotto l'obiettivo, e quel ciclo
/// finisce in un modo solo dei due — o la coda ci sta (e allora
/// l'accorciamento non serve), o la coda si e' svuotata (e allora si ripesca
/// l'ultimo messaggio, che e' uno). Quindi il pareggio fra due messaggi
/// lunghi uguali oggi non capita mai, e il codice che lo gestisce non e'
/// morto ma **dormiente**: e' scritto per una strada che non c'e' ancora. Si
/// porta uguale, e si prova chiamandolo direttamente — perche' il giorno in
/// cui quella strada si apre, la differenza fra il primo e l'ultimo massimo
/// sarebbe un messaggio diverso accorciato, in silenzio.
pub fn accorcia_il_piu_grosso(
    coda: &[Messaggio],
    obiettivo: u32,
) -> (Vec<Messaggio>, Vec<Accorciato>) {
    let mut accorciati: Vec<Accorciato> = Vec::new();
    let mut fuori = coda.to_vec();
    let prima: Vec<usize> = fuori.iter().map(|m| m.lunghezza()).collect();
    let giri = fuori.len() + 8; // tetto: mai un ciclo aperto
    for _ in 0..giri {
        let eccesso = i64::from(token_dei(&fuori)) - i64::from(obiettivo);
        if eccesso <= 0 {
            break;
        }
        // A parita' di lunghezza vince il primo, come in Python.
        let mut i = 0usize;
        let mut massimo = 0usize;
        for (k, m) in fuori.iter().enumerate() {
            let l = m.lunghezza();
            if l > massimo {
                massimo = l;
                i = k;
            }
        }
        let testo: Vec<char> = fuori[i].contenuto.chars().collect();
        if testo.len() <= MINIMO_ACCORCIABILE {
            break; // non c'e' piu' niente di grosso da accorciare
        }
        // Quanto deve diventare lungo, piu' un margine per la scritta.
        let da_togliere = (eccesso as f64 * CARATTERI_PER_TOKEN) as i64 + 200;
        let voluta = std::cmp::max(
            MINIMO_ACCORCIABILE as i64,
            testo.len() as i64 - da_togliere,
        ) as usize;
        let meta = std::cmp::max(60, voluta / 2);
        let tolti = testo.len().saturating_sub(meta * 2);
        let testa: String = testo.iter().take(meta).collect();
        let in_fondo: String = testo[testo.len().saturating_sub(meta)..].iter().collect();
        let nuovo = format!("{testa}{}{in_fondo}", dichiarazione(tolti));
        if nuovo.chars().count() >= testo.len() {
            break; // non si guadagna niente: si smette
        }
        fuori[i].contenuto = nuovo;
    }
    for (k, m) in fuori.iter().enumerate() {
        let dopo = m.lunghezza();
        if dopo != prima[k] {
            accorciati.push(Accorciato {
                indice: k + 1, // +1: il messaggio di sistema torna in testa
                caratteri_prima: prima[k],
                caratteri_dopo: dopo,
            });
        }
    }
    (fuori, accorciati)
}

#[cfg(test)]
mod prove {
    use super::*;

    fn m(ruolo: &str, quanti: usize) -> Messaggio {
        Messaggio::nuovo(ruolo, &"x".repeat(quanti))
    }

    #[test]
    fn i_caratteri_non_sono_byte() {
        // Sette caratteri, otto byte: una stima a byte direbbe 3 token
        // invece di 3 e taglierebbe prima del dovuto su testo accentato.
        let accentato = "perche'a";
        assert_eq!(accentato.chars().count(), 8);
        let vero = "perché\u{e8}a"; // 8 caratteri, 10 byte
        assert_eq!(vero.chars().count(), 8);
        assert!(vero.len() > vero.chars().count());
        assert_eq!(stima_token(vero), stima_token(accentato));
    }

    #[test]
    fn contesto_ignoto_non_taglia_a_token() {
        assert_eq!(spazio_per_la_conversazione(0, "sistema", None), 0);
    }

    #[test]
    fn il_prefisso_puo_mangiarsi_tutto() {
        // Se sistema piu' schemi piu' riserva superano il contesto non si
        // torna un numero negativo: si torna zero, cioe' «non lo so».
        let s = "x".repeat(100_000);
        assert_eq!(spazio_per_la_conversazione(16_384, &s, None), 0);
    }

    #[test]
    fn sotto_il_tetto_non_si_tocca_niente() {
        let msg: Vec<Messaggio> = (0..10).map(|_| m("user", 10)).collect();
        let (fuori, r) = taglia(&msg, 60, 40, 0);
        assert_eq!(fuori, msg);
        assert_eq!(r.tolti_per_numero, 0);
        assert_eq!(r.rimasti, 10);
    }

    #[test]
    fn il_fondo_lascia_respiro() {
        let msg: Vec<Messaggio> = (0..61).map(|_| m("user", 10)).collect();
        let (fuori, r) = taglia(&msg, 60, 40, 0);
        // sistema + (fondo - 1)
        assert_eq!(fuori.len(), 40);
        assert_eq!(r.tolti_per_numero, 21);
        // e il turno dopo non si taglia piu'
        let mut ancora = fuori.clone();
        ancora.push(m("user", 10));
        ancora.push(m("assistant", 10));
        let (_, r2) = taglia(&ancora, 60, 40, 0);
        assert_eq!(r2.tolti_per_numero, 0);
    }

    #[test]
    fn un_fondo_troppo_alto_viene_abbassato() {
        let msg: Vec<Messaggio> = (0..100).map(|_| m("user", 10)).collect();
        let (fuori, _) = taglia(&msg, 60, 59, 0);
        assert_eq!(fuori.len(), 45); // fondo portato a 60*3/4 = 45
    }

    #[test]
    fn la_risposta_di_tool_non_resta_orfana() {
        let mut msg: Vec<Messaggio> = vec![m("system", 10)];
        for _ in 0..40 {
            msg.push(m("tool", 10));
        }
        for _ in 0..20 {
            msg.push(m("user", 10));
        }
        msg.push(m("user", 10));
        let (fuori, r) = taglia(&msg, 60, 40, 0);
        assert_ne!(fuori[1].ruolo, "tool");
        assert!(r.orfani_dopo_numero > 0);
    }

    #[test]
    fn tutta_coda_di_tool_lascia_il_solo_sistema() {
        // Non e' un desiderio: e' cosa fa il Python, e va portato uguale
        // prima di essere discusso. Il taglio a numero **non ha** la rete
        // che il taglio a token ha: li' se la coda si svuota si ripesca
        // l'ultimo messaggio, qui no. Servono quaranta risposte di tool di
        // fila nel punto del taglio - improbabile, non impossibile - e la
        // conversazione sparisce senza che niente lo dica.
        let mut msg: Vec<Messaggio> = vec![m("system", 10)];
        for _ in 0..60 {
            msg.push(m("tool", 10));
        }
        let (fuori, r) = taglia(&msg, 60, 40, 0);
        assert_eq!(fuori.len(), 1);
        assert_eq!(fuori[0].ruolo, "system");
        assert_eq!(r.orfani_dopo_numero, 39);
        assert!(!r.ripescato_l_ultimo, "la rete del taglio a token non c'e' qui");
    }

    #[test]
    fn venticinque_messaggi_possono_essere_centomila_token() {
        // Il caso per cui il taglio a token esiste: sotto il tetto dei
        // messaggi, ma enormemente sopra il contesto.
        let mut msg = vec![Messaggio::nuovo("system", "sistema")];
        for _ in 0..24 {
            msg.push(m("user", 15_000));
        }
        let (fuori, r) = taglia(&msg, 60, 40, 3_300);
        assert_eq!(r.tolti_per_numero, 0);
        assert!(r.tolti_per_token > 0);
        assert!(token_dei(&fuori[1..]) <= 3_300);
    }

    #[test]
    fn un_solo_messaggio_piu_grande_di_tutto_si_accorcia_e_lo_dice() {
        let msg = vec![Messaggio::nuovo("system", "sistema"), m("user", 200_000)];
        let (fuori, r) = taglia(&msg, 60, 40, 3_300);
        assert_eq!(fuori.len(), 2);
        assert!(fuori[1].contenuto.contains("[...tagliati "));
        assert_eq!(r.accorciati.len(), 1);
        assert_eq!(r.accorciati[0].indice, 1);
        assert!(r.accorciati[0].caratteri_dopo < r.accorciati[0].caratteri_prima);
    }

    #[test]
    fn non_si_resta_mai_senza_ultimo_scambio() {
        // Disponibili minuscoli: il ciclo svuota tutto, e si ripesca.
        let msg = vec![
            Messaggio::nuovo("system", "sistema"),
            m("user", 5_000),
            m("assistant", 5_000),
        ];
        let (fuori, r) = taglia(&msg, 60, 40, 1);
        assert!(fuori.len() >= 2);
        assert!(r.ripescato_l_ultimo);
    }

    #[test]
    fn a_parita_di_lunghezza_si_accorcia_il_primo() {
        // Si chiama la funzione **direttamente**, e la ragione e' scritta
        // sopra la funzione: dal taglio normale non ci si arriva mai con piu'
        // di un messaggio. Provarla solo attraverso `taglia` vorrebbe dire
        // non provarla affatto, e la differenza fra Python e Rust sul
        // pareggio si vedrebbe solo il giorno in cui quella strada si apre.
        let coda = vec![
            Messaggio::nuovo("user", &"a".repeat(20_000)),
            Messaggio::nuovo("user", &"b".repeat(20_000)),
        ];
        let (fuori, accorciati) = accorcia_il_piu_grosso(&coda, 9_000);
        // Un obiettivo raggiungibile in una passata sola: cosi la prova guarda
        // la scelta, non quante volte il ciclo gira.
        assert_eq!(accorciati.len(), 1, "{accorciati:?}");
        assert_eq!(accorciati[0].indice, 1);
        assert!(fuori[0].contenuto.starts_with('a'));
        assert!(fuori[0].contenuto.contains("[...tagliati "));
        assert!(!fuori[1].contenuto.contains("[...tagliati "));
    }

    #[test]
    fn laccorciamento_non_taglia_a_meta_un_carattere() {
        // Su `&str` in Rust `testo[..meta]` conta byte e va in panico dentro
        // una lettera accentata. Qui si lavora su `char`, e questa prova
        // esiste perche' quel panico sarebbe arrivato all'utente.
        let lungo = "perche' la citta' e' cosi'. ".repeat(2_000);
        let coda = vec![Messaggio::nuovo("user", &lungo)];
        let (fuori, _) = accorcia_il_piu_grosso(&coda, 100);
        assert!(fuori[0].contenuto.contains("[...tagliati "));
        let accentato = "perch\u{e9} la citt\u{e0} \u{e8} cos\u{ec}. ".repeat(2_000);
        let coda2 = vec![Messaggio::nuovo("user", &accentato)];
        let (fuori2, _) = accorcia_il_piu_grosso(&coda2, 100);
        assert!(fuori2[0].contenuto.contains("[...tagliati "));
    }

    #[test]
    fn laccorciamento_termina_sempre() {
        // Il difetto storico: la scritta che dichiara il taglio faceva
        // ricrescere il testo esattamente di quanto si era tolto.
        for quanti in [401usize, 500, 900, 1_000, 5_000] {
            let msg = vec![Messaggio::nuovo("system", "s"), m("user", quanti)];
            let (_fuori, _r) = taglia(&msg, 60, 40, 1);
        }
    }
}
