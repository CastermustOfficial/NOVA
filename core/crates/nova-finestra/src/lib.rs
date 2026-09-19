//! La finestra: quanto della conversazione ci sta nella memoria del modello.
//!
//! Questa cassetta non tocca niente. Guarda le righe di una conversazione e
//! dice **quali tenere** e **quali accorciare**; chi le tiene davvero - che
//! ha per le mani messaggi ben piu' ricchi di un ruolo e un testo, con dentro
//! chiamate a strumenti e identificativi - applica il piano e basta.
//!
//! La ragione di questa divisione e' che il taglio e' un pezzo di
//! ragionamento delicato, pieno di casi che sono gia' andati storti una
//! volta, e va potuto provare senza una conversazione vera intorno.

/// Quanti caratteri vale un token, per stimare senza tokenizzatore.
///
/// Misurato due volte sulla macchina di casa, con prompt veri: 3,88 su una
/// conversazione in italiano e 4,37 su del testo ripetitivo letto da un file.
/// Si tiene il numero **piu' basso** dei due, anzi un filo sotto: sbagliare
/// per eccesso di token vuol dire tagliare un po' presto, che si nota appena;
/// sbagliare per difetto vuol dire sfondare il contesto, e quello e' un
/// errore in faccia all'utente.
pub const CARATTERI_PER_TOKEN: f64 = 3.5;

/// Quanto lasciare libero per la risposta. Il contesto non serve solo a
/// leggere: il modello ci scrive dentro.
pub const RISERVA_RISPOSTA_TOKEN: usize = 1024;

/// Sopra questo numero di righe si taglia.
pub const TETTO_MESSAGGI: usize = 60;

/// E si scende fino a qui. La distanza fra i due e' il punto: senza, si
/// taglia a ogni turno.
pub const FONDO_MESSAGGI: usize = 40;

/// Sotto questa lunghezza un messaggio non si accorcia piu': quel che resta
/// e' gia' solo l'inizio e la fine, e continuare vorrebbe dire toglierne il
/// senso invece che il peso.
pub const MINIMO_ACCORCIABILE: usize = 400;

/// Una riga di conversazione, ridotta a cio' che serve per decidere.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Riga {
    pub ruolo: String,
    pub contenuto: String,
}

impl Riga {
    pub fn nuova(ruolo: &str, contenuto: &str) -> Riga {
        Riga { ruolo: ruolo.to_string(), contenuto: contenuto.to_string() }
    }
}

/// Le misure entro cui stare.
#[derive(Debug, Clone, Copy)]
pub struct Misure {
    pub tetto: usize,
    pub fondo: usize,
    /// Quanti token restano alla conversazione, tolto tutto il resto.
    /// **Zero vuol dire «non lo so»**, e allora il taglio a token non si fa:
    /// meglio quello a righe da solo che uno inventato.
    pub disponibili: usize,
}

impl Default for Misure {
    fn default() -> Misure {
        Misure { tetto: TETTO_MESSAGGI, fondo: FONDO_MESSAGGI, disponibili: 0 }
    }
}

/// Una riga che sopravvive: da dove viene, e - se e' stata accorciata - come
/// va riscritta. `contenuto` a `None` vuol dire «lasciala com'e'».
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tenuta {
    pub da: usize,
    pub contenuto: Option<String>,
}

/// Quanti token vale un testo, senza tokenizzatore.
///
/// Serve una stima e non una misura: il tokenizzatore vero sta nel modello,
/// cambia con il modello, e chiederglielo costerebbe un giro di rete per ogni
/// messaggio a ogni turno solo per decidere se tagliare.
pub fn stima_token(testo: &str) -> usize {
    (testo.chars().count() as f64 / CARATTERI_PER_TOKEN) as usize + 1
}

/// Quanto resta alla conversazione, tolto il prefisso fisso.
///
/// Il conto non e' un dettaglio contabile, e' la scoperta che ha fatto
/// nascere tutto questo. Sulla macchina di casa, con il contesto a 16.384:
///
/// ```text
/// messaggio di sistema     ~5.200 token
/// schemi dei sessanta tool ~6.900 token   (il 42% del contesto)
/// riserva per la risposta   1.024 token
/// ------------------------------------
/// resta alla conversazione ~3.300 token   (il 20%)
/// ```
///
/// `ctx` a zero vuol dire che il contesto non lo decide questa
/// configurazione - un cervello dietro una API non lo dice - e allora si
/// torna zero, cioe' «non lo so».
pub fn spazio_per_la_conversazione(ctx: usize, sistema: &str, strumenti: &str) -> usize {
    if ctx == 0 {
        return 0;
    }
    let fissi = stima_token(sistema)
        + RISERVA_RISPOSTA_TOKEN
        + if strumenti.is_empty() { 0 } else { stima_token(strumenti) };
    ctx.saturating_sub(fissi)
}

/// Il fondo davvero usabile, dato il tetto.
///
/// Un fondo troppo vicino al tetto riporta al difetto di prima senza dirlo, e
/// «un turno di distanza» non basta: con `tetto - 2` si taglierebbe a turni
/// alterni invece che a ogni turno, che e' meta' del difetto e non la sua
/// assenza. La distanza minima e' un quarto del tetto, cioe' una decina di
/// turni di respiro.
pub fn fondo_sicuro(tetto: usize, fondo: usize) -> usize {
    fondo.min(tetto * 3 / 4).max(2)
}

fn token_delle(righe: &[Riga], indici: &[usize]) -> usize {
    indici.iter().map(|&i| stima_token(&righe[i].contenuto)).sum()
}

/// Il piano di taglio.
///
/// Si butta cio' che sta **subito dopo la prima riga**, e quello e' il posto
/// peggiore: la cache del prefisso di llama.cpp vale finche' i token in testa
/// sono gli stessi, quindi spostare la seconda riga invalida tutto il resto e
/// si rielabora l'intera conversazione. Per questo si taglia di rado e si
/// scende fino a un fondo, invece di fermarsi sul filo del tetto.
///
/// Misurato con `banco_taglio.py` su Gemma 4 26B-A4B, ottantuno messaggi,
/// 15.379 token di prefisso:
///
/// ```text
/// a caldo, prefisso intatto           175 ms
/// fermandosi sul filo del tetto     1.771 ms
/// e il turno seguente               1.731 ms   <- non guarisce
/// scendendo fino al fondo           1.217 ms
/// e il turno seguente                 226 ms   <- guarito
/// ```
pub fn taglia(righe: &[Riga], m: &Misure) -> Vec<Tenuta> {
    if righe.is_empty() {
        return Vec::new();
    }
    let mut indici: Vec<usize> = (0..righe.len()).collect();

    // -- taglio a numero -------------------------------------------------
    if righe.len() > m.tetto {
        let giu = fondo_sicuro(m.tetto, m.fondo);
        let da = righe.len() - (giu - 1).min(righe.len() - 1);
        let mut coda: Vec<usize> = (da..righe.len()).collect();
        // Una risposta di strumento senza la chiamata che l'ha prodotta non
        // e' leggibile da nessun modello: si scarta finche' la coda non
        // comincia da qualcosa di sensato.
        while coda.first().is_some_and(|&i| righe[i].ruolo == "tool") {
            coda.remove(0);
        }
        indici = std::iter::once(0).chain(coda).collect();
    }
    // Sotto il tetto non si taglia per numero - ma il taglio a token va
    // fatto lo stesso, ed e' proprio questo il caso che conta: dodici scambi
    // con dentro il contenuto di un file sono venticinque messaggi, quindi
    // passano di qui, e sono centomila token. La prima versione metteva il
    // taglio a token **dopo** l'uscita anticipata, cioe' non lo eseguiva mai
    // nel solo caso per cui era stato scritto.

    // -- taglio a token --------------------------------------------------
    // `< 2` e non `<= 2`: con esattamente due righe - sistema piu' una
    // risposta enorme - non c'e' niente da **togliere**, ma c'e' ancora da
    // **accorciare**, ed e' il caso che ha fatto scrivere l'accorciamento.
    if m.disponibili == 0 || indici.len() < 2 {
        return indici.into_iter().map(|da| Tenuta { da, contenuto: None }).collect();
    }
    let testa = indici[0];
    let mut coda: Vec<usize> = indici[1..].to_vec();
    if token_delle(righe, &coda) <= m.disponibili {
        return indici.into_iter().map(|da| Tenuta { da, contenuto: None }).collect();
    }
    // Stessa idea del fondo: si scende sotto la soglia, non ci si ferma
    // sopra, o si ritaglia a ogni turno e la cache non si riforma mai.
    let obiettivo = m.disponibili * 3 / 4;
    while !coda.is_empty() && token_delle(righe, &coda) > obiettivo {
        coda.remove(0);
    }
    while coda.first().is_some_and(|&i| righe[i].ruolo == "tool") {
        coda.remove(0);
    }
    // Non si resta mai senza l'ultimo scambio: una conversazione vuota non e'
    // una conversazione accorciata, e' una amnesia.
    if coda.is_empty() {
        coda.push(*indici.last().unwrap());
    }

    let mut fuori: Vec<(usize, Option<String>)> = coda.iter().map(|&i| (i, None)).collect();
    if token_delle(righe, &coda) > m.disponibili {
        // E se cio' che resta non ci sta **comunque**, vuol dire che una sola
        // riga e' piu' grande di tutto lo spazio: il contenuto di un file
        // letto, una pagina web intera. Buttarla vorrebbe dire perdere
        // proprio la cosa di cui l'utente ha chiesto conto; tenerla intera
        // vuol dire sfondare il contesto. Si accorcia, e lo si dice nel testo
        // - un taglio dichiarato il modello lo capisce, uno silenzioso gli fa
        // credere che il file finisca li'.
        accorcia_le_piu_grosse(righe, &mut fuori, obiettivo);
    }

    std::iter::once(Tenuta { da: testa, contenuto: None })
        .chain(fuori.into_iter().map(|(da, contenuto)| Tenuta { da, contenuto }))
        .collect()
}

/// Accorcia le righe piu' grosse finche' la coda non ci sta.
///
/// **La lunghezza da togliere si calcola, non si indovina.** La prima
/// versione tagliava a una misura fissa - millecinquecento caratteri in testa
/// e altrettanti in coda - e non terminava: la scritta che dichiara il taglio
/// e' lunga quanto i caratteri che alla seconda passata restavano da
/// togliere, quindi il testo si accorciava di ottanta caratteri e ricresceva
/// di ottanta, per sempre. Un ciclo che «ovviamente» finisce e non finisce.
/// Adesso a ogni passata si punta alla lunghezza che serve, e si esce se non
/// si e' guadagnato niente: due condizioni invece di una, perche' una si e'
/// gia' vista sbagliare.
fn accorcia_le_piu_grosse(
    righe: &[Riga],
    fuori: &mut Vec<(usize, Option<String>)>,
    obiettivo: usize,
) {
    let testo_di = |f: &(usize, Option<String>)| -> String {
        f.1.clone().unwrap_or_else(|| righe[f.0].contenuto.clone())
    };
    let tetto_giri = fuori.len() + 8;
    for _ in 0..tetto_giri {
        let ora: usize = fuori.iter().map(|f| stima_token(&testo_di(f))).sum();
        if ora <= obiettivo {
            return;
        }
        let eccesso = ora - obiettivo;
        let i = match (0..fuori.len()).max_by_key(|&k| testo_di(&fuori[k]).chars().count()) {
            Some(i) => i,
            None => return,
        };
        let testo = testo_di(&fuori[i]);
        let lunga = testo.chars().count();
        if lunga <= MINIMO_ACCORCIABILE {
            return; // non c'e' piu' niente di grosso da accorciare
        }
        // Quanto deve diventare lunga, piu' un margine per la scritta.
        let da_togliere = (eccesso as f64 * CARATTERI_PER_TOKEN) as usize + 200;
        let voluta = lunga.saturating_sub(da_togliere).max(MINIMO_ACCORCIABILE);
        match accorcia(&testo, voluta) {
            Some(nuovo) => fuori[i].1 = Some(nuovo),
            None => return, // non si guadagna niente: si smette
        }
    }
}

/// Accorcia un testo alla lunghezza voluta, tenendo l'inizio e la fine.
///
/// L'inizio dice cos'era, la fine spesso porta la conclusione, ed e' il mezzo
/// che si puo' perdere. Torna `None` se non ci si guadagna niente.
pub fn accorcia(testo: &str, voluta: usize) -> Option<String> {
    let c: Vec<char> = testo.chars().collect();
    let meta = (voluta / 2).max(60);
    if meta * 2 >= c.len() {
        return None;
    }
    let tolti = c.len() - meta * 2;
    let nuovo = format!(
        "{}\n\n[...tagliati {} caratteri perche' non ci stavano nella memoria del modello...]\n\n{}",
        c[..meta].iter().collect::<String>(),
        tolti,
        c[c.len() - meta..].iter().collect::<String>(),
    );
    if nuovo.chars().count() >= c.len() {
        None
    } else {
        Some(nuovo)
    }
}

/// Applica un piano. Comodo per provare, e per chi ha davvero solo righe.
pub fn applica(righe: &[Riga], piano: &[Tenuta]) -> Vec<Riga> {
    piano
        .iter()
        .map(|t| Riga {
            ruolo: righe[t.da].ruolo.clone(),
            contenuto: t.contenuto.clone().unwrap_or_else(|| righe[t.da].contenuto.clone()),
        })
        .collect()
}

#[cfg(test)]
mod prove {
    use super::*;

    fn conv(quante: usize) -> Vec<Riga> {
        (0..quante)
            .map(|i| {
                let ruolo = if i == 0 { "system" } else if i % 2 == 1 { "user" } else { "assistant" };
                Riga::nuova(ruolo, &format!("riga {i}"))
            })
            .collect()
    }

    fn tenute(righe: &[Riga], m: &Misure) -> Vec<usize> {
        taglia(righe, m).into_iter().map(|t| t.da).collect()
    }

    #[test]
    fn sotto_il_tetto_non_si_butta_niente() {
        let r = conv(30);
        assert_eq!(tenute(&r, &Misure::default()), (0..30).collect::<Vec<_>>());
    }

    #[test]
    fn la_prima_riga_resta_sempre_la_prima() {
        let r = conv(200);
        let p = tenute(&r, &Misure::default());
        assert_eq!(p[0], 0, "il messaggio di sistema non si sposta mai");
    }

    #[test]
    fn si_scende_fino_al_fondo_non_sul_filo_del_tetto() {
        let r = conv(61);
        let p = tenute(&r, &Misure::default());
        // fondo 40: una testa piu' trentanove di coda.
        assert_eq!(p.len(), 40, "tagliare a tetto-1 farebbe ritagliare ogni turno");
        assert_eq!(*p.last().unwrap(), 60);
    }

    #[test]
    fn e_poi_per_dieci_turni_non_si_ritaglia_piu() {
        // Il punto della distanza fra tetto e fondo: dopo un taglio ci
        // vogliono venti righe - una decina di turni - per tornare al tetto.
        let r = conv(61);
        let dopo = applica(&r, &taglia(&r, &Misure::default()));
        let mut cresciuta = dopo.clone();
        for i in 0..20 {
            cresciuta.push(Riga::nuova("user", &format!("nuova {i}")));
            let p = taglia(&cresciuta, &Misure::default());
            if i < 19 {
                assert_eq!(p.len(), cresciuta.len(), "al giro {i} ha ritagliato troppo presto");
            }
        }
    }

    #[test]
    fn un_fondo_appiccicato_al_tetto_viene_allontanato() {
        assert_eq!(fondo_sicuro(60, 59), 45);
        assert_eq!(fondo_sicuro(60, 58), 45, "a turni alterni e' meta' del difetto, non la sua assenza");
        assert_eq!(fondo_sicuro(60, 40), 40);
        assert_eq!(fondo_sicuro(4, 1), 2, "sotto due non si scende");
    }

    #[test]
    fn la_coda_non_comincia_mai_da_una_risposta_di_strumento() {
        let mut r = conv(61);
        for i in 22..25 {
            r[i].ruolo = "tool".to_string();
        }
        let p = tenute(&r, &Misure::default());
        assert_ne!(r[p[1]].ruolo, "tool", "una risposta senza la sua chiamata non la legge nessuno");
        assert_eq!(p[1], 25);
    }

    #[test]
    fn senza_sapere_il_contesto_non_si_taglia_a_token() {
        let r = vec![Riga::nuova("system", "s"), Riga::nuova("user", &"x".repeat(400_000))];
        let p = taglia(&r, &Misure::default());
        assert_eq!(p.len(), 2);
        assert!(p[1].contenuto.is_none(), "zero vuol dire «non lo so», non «taglia a zero»");
    }

    #[test]
    fn venticinque_righe_pesantissime_si_tagliano_lo_stesso() {
        // Il caso per cui il taglio a token e' stato scritto, e che la prima
        // versione non raggiungeva mai: sotto il tetto di righe, sopra ogni
        // tetto di token.
        let mut r = vec![Riga::nuova("system", "sistema")];
        for i in 0..24 {
            r.push(Riga::nuova("user", &format!("{i}{}", "z".repeat(20_000))));
        }
        let m = Misure { disponibili: 3_300, ..Misure::default() };
        let p = taglia(&r, &m);
        assert!(p.len() < 25, "venticinque righe non superano il tetto: il taglio deve essere quello a token");
        let dopo = applica(&r, &p);
        let peso: usize = dopo[1..].iter().map(|x| stima_token(&x.contenuto)).sum();
        assert!(peso <= 3_300, "restano {peso} token contro 3.300");
    }

    #[test]
    fn il_taglio_a_token_scende_sotto_la_soglia_non_sul_filo() {
        let mut r = vec![Riga::nuova("system", "sistema")];
        for i in 0..20 {
            r.push(Riga::nuova("user", &format!("{i}{}", "z".repeat(3_500))));
        }
        let m = Misure { disponibili: 10_000, ..Misure::default() };
        let dopo = applica(&r, &taglia(&r, &m));
        let peso: usize = dopo[1..].iter().map(|x| stima_token(&x.contenuto)).sum();
        assert!(peso <= 7_500, "doveva scendere al tre quarti, sta a {peso}");
    }

    #[test]
    fn non_si_resta_mai_senza_lultimo_scambio() {
        let r = vec![
            Riga::nuova("system", "s"),
            Riga::nuova("user", &"a".repeat(80_000)),
            Riga::nuova("assistant", &"b".repeat(80_000)),
        ];
        let m = Misure { disponibili: 100, ..Misure::default() };
        let p = taglia(&r, &m);
        assert_eq!(p.len(), 2, "una conversazione vuota non e' accorciata, e' amnesica");
        assert_eq!(p[1].da, 2);
        assert!(p[1].contenuto.is_some(), "quel che resta va accorciato, non buttato");
    }

    #[test]
    fn una_sola_riga_enorme_si_accorcia_invece_di_sparire() {
        let r = vec![Riga::nuova("system", "s"), Riga::nuova("user", &"f".repeat(50_000))];
        let m = Misure { disponibili: 2_000, ..Misure::default() };
        let p = taglia(&r, &m);
        assert_eq!(p.len(), 2, "con due righe non c'e' niente da togliere, c'e' da accorciare");
        let nuovo = p[1].contenuto.as_ref().expect("doveva accorciarla");
        assert!(nuovo.contains("[...tagliati "), "un taglio silenzioso fa credere che il file finisca li'");
        assert!(nuovo.chars().count() < 50_000);
    }

    #[test]
    fn laccorciatura_tiene_linizio_e_la_fine() {
        let testo = format!("INIZIO{}FINE", "m".repeat(5_000));
        let nuovo = accorcia(&testo, 400).expect("accorciabile");
        assert!(nuovo.starts_with("INIZIO"));
        assert!(nuovo.ends_with("FINE"));
    }

    #[test]
    fn accorciare_quel_che_non_si_guadagna_non_si_fa() {
        // La scritta e' lunga quanto i caratteri che restavano da togliere:
        // il testo si accorciava di ottanta e ricresceva di ottanta, per
        // sempre. Qui deve dire di no invece di girare.
        assert!(accorcia("corto", 400).is_none());
        assert!(accorcia(&"x".repeat(130), 400).is_none());
        // E il caso vero, quello che girava: il testo e' piu' lungo della
        // meta' voluta - quindi il primo controllo lo lascia passare - ma i
        // caratteri da togliere sono meno della scritta che dichiara il
        // taglio. Accorciarlo lo **allunga**.
        assert!(accorcia(&"x".repeat(500), 480).is_none(),
                "venti caratteri tolti e ottanta di scritta: si cresce, non si accorcia");
    }

    #[test]
    fn accorciare_non_allunga_mai() {
        for lunghezza in [130usize, 400, 500, 600, 900, 5_000] {
            for voluta in [120usize, 400, 480, 560, 800] {
                let testo = "x".repeat(lunghezza);
                if let Some(n) = accorcia(&testo, voluta) {
                    assert!(n.chars().count() < lunghezza,
                            "da {lunghezza} volendo {voluta} sono usciti {} caratteri", n.chars().count());
                }
            }
        }
    }

    #[test]
    fn laccorciatura_finisce_sempre() {
        // Tante righe grosse e un obiettivo irraggiungibile: deve tornare,
        // non girare. Il tetto dei giri e' l'unica rete sotto.
        let mut r = vec![Riga::nuova("system", "s")];
        for _ in 0..30 {
            r.push(Riga::nuova("user", &"q".repeat(2_000)));
        }
        let m = Misure { disponibili: 1, ..Misure::default() };
        let p = taglia(&r, &m);
        assert!(!p.is_empty());
    }

    #[test]
    fn lo_spazio_si_conta_togliendo_sistema_strumenti_e_riserva() {
        let sistema = "s".repeat(3_500); // 1.001 token
        let tool = "t".repeat(3_500);    // 1.001 token
        assert_eq!(spazio_per_la_conversazione(16_384, &sistema, &tool), 16_384 - 1_001 - 1_024 - 1_001);
        assert_eq!(spazio_per_la_conversazione(0, &sistema, &tool), 0, "contesto ignoto resta ignoto");
        assert_eq!(spazio_per_la_conversazione(100, &sistema, &tool), 0, "non si scende sotto zero");
    }

    #[test]
    fn gli_strumenti_assenti_non_si_contano() {
        let s = "sistema";
        assert!(spazio_per_la_conversazione(16_384, s, "") > spazio_per_la_conversazione(16_384, s, "[]"));
    }

    #[test]
    fn una_conversazione_vuota_non_fa_saltare_niente() {
        assert!(taglia(&[], &Misure::default()).is_empty());
        assert_eq!(taglia(&conv(1), &Misure { disponibili: 10, ..Misure::default() }).len(), 1);
    }

    #[test]
    fn quel_che_non_si_tocca_resta_identico() {
        let r = conv(61);
        let p = taglia(&r, &Misure::default());
        assert!(p.iter().all(|t| t.contenuto.is_none()), "senza taglio a token nessuna riga va riscritta");
    }
}
