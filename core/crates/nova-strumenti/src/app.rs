//! Applicazioni, finestre e processi: le parti che non chiedono niente al
//! sistema.
//!
//! Qui dentro non si avvia, non si chiude e non si porta davanti niente. Si
//! decide **cosa** — quale programma vuol dire «blocco note», quali processi
//! risponderebbero a «notepad», cosa legge chi deve approvare una chiusura —
//! e si scrive la risposta. Chi lo fa davvero sta in `nova-platform`, e chi
//! mette insieme le due meta' sta in `nova-core`.
//!
//! La parte che pesa e' [`bersagli`], e pesa per una ragione precisa (D141):
//! la strada di prima incollava il nome dentro un `-like` di PowerShell, e su
//! una macchina vera `*`, `?` e `[a-z]` selezionavano tutti e 292 i processi.
//! Con «force» voleva dire fermare il sistema intero da un argomento di un
//! carattere. Qui non c'e' un modello: c'e' una sottostringa.

/// Quante applicazioni si mostrano al massimo.
///
/// Non e' un limite tecnico: e' quanto contesto vale la pena spendere in un
/// elenco. Cio' che avanza si **dichiara**, non si taglia in silenzio (D129).
pub const MASSIMO_APP: usize = 250;

/// I nomi comodi, e cosa vogliono dire.
///
/// Sono gli stessi del Python e nello stesso ordine: un alias che c'e' da una
/// parte e non dall'altra e' un «apri il blocco note» che funziona o no a
/// seconda di quale meta' ha risposto.
pub const ALIAS: &[(&str, &str)] = &[
    ("notepad", "notepad.exe"),
    ("blocco note", "notepad.exe"),
    ("calcolatrice", "calc.exe"),
    ("calculator", "calc.exe"),
    ("esplora risorse", "explorer.exe"),
    ("explorer", "explorer.exe"),
    ("file explorer", "explorer.exe"),
    ("cartelle", "explorer.exe"),
    ("terminale", "wt.exe"),
    ("terminal", "wt.exe"),
    ("powershell", "powershell.exe"),
    ("cmd", "cmd.exe"),
    ("chrome", "chrome"),
    ("google chrome", "chrome"),
    ("edge", "msedge"),
    ("firefox", "firefox"),
    ("vscode", "code"),
    ("visual studio code", "code"),
    ("paint", "mspaint.exe"),
    ("word", "winword"),
    ("excel", "excel"),
    ("powerpoint", "powerpnt"),
    ("outlook", "outlook"),
    ("impostazioni", "ms-settings:"),
    ("settings", "ms-settings:"),
    ("task manager", "taskmgr.exe"),
    ("gestione attivita", "taskmgr.exe"),
    ("spotify", "spotify"),
    ("steam", "steam"),
    ("lm studio", "LM Studio"),
];

/// Cosa lanciare per questo nome.
///
/// Se non e' un alias torna **il nome com'e' stato scritto**, spazi compresi:
/// e' cio' che fa il Python, e un percorso con uno spazio in coda e' un
/// percorso diverso da quello senza — non tocca a questa funzione deciderlo.
pub fn risolvi(nome: &str) -> Result<String, String> {
    let chiave = nome.trim().to_lowercase();
    if chiave.is_empty() {
        return Err("nome applicazione vuoto".into());
    }
    Ok(ALIAS
        .iter()
        .find(|(k, _)| *k == chiave)
        .map(|(_, v)| v.to_string())
        .unwrap_or_else(|| nome.to_string()))
}

/// L'elenco delle applicazioni installate, filtrato e **tagliato dicendolo**.
///
/// Prima il taglio era `names[:250]` e basta: su una macchina con trecento
/// applicazioni il modello ne riceveva 250 e non aveva **nessun modo** di
/// sapere che ne mancavano cinquanta — cercava un nome, non lo trovava, e
/// concludeva che non e' installato (D129).
pub fn elenco_installate(nomi: &[String], filtro: &str) -> String {
    let f = filtro.to_lowercase();
    let scelti: Vec<&String> = nomi
        .iter()
        .filter(|n| f.is_empty() || n.to_lowercase().contains(&f))
        .collect();
    if scelti.is_empty() {
        return "Nessuna applicazione trovata.".into();
    }
    if scelti.len() > MASSIMO_APP {
        let quante = scelti.len();
        let mut righe: Vec<String> = scelti[..MASSIMO_APP]
            .iter()
            .map(|s| s.to_string())
            .collect();
        righe.push(format!(
            "[... e altre {} su {quante}: restringi con «filter» per vederle]",
            quante - MASSIMO_APP
        ));
        return righe.join("\n");
    }
    scelti
        .iter()
        .map(|s| s.as_str())
        .collect::<Vec<_>>()
        .join("\n")
}

/// Un processo, per quello che serve a sceglierlo.
#[derive(Debug, Clone, PartialEq)]
pub struct Processo {
    pub pid: u32,
    pub nome: String,
    pub memoria_byte: u64,
}

/// Una finestra, per quello che serve a sceglierla.
#[derive(Debug, Clone, PartialEq)]
pub struct Finestra {
    pub handle: i64,
    pub pid: u32,
    pub titolo: String,
    pub processo: String,
}

/// Un processo che risponderebbe a un nome, con i titoli delle sue finestre.
///
/// I titoli ci sono perche' sono **quello che chi approva deve vedere**:
/// «notepad.exe» non dice niente, «*napoli difesa» dice che c'e' del lavoro
/// non salvato.
#[derive(Debug, Clone, PartialEq)]
pub struct Bersaglio {
    pub pid: u32,
    pub nome: String,
    pub finestre: Vec<String>,
}

/// Quali processi risponderebbero a questo nome, **come sottostringa**.
///
/// Si guarda il nome del processo e i titoli delle sue finestre, senza
/// maiuscole. `*` si cerca alla lettera: non vuol dire «non trova niente» —
/// una finestra del Blocco note si chiamava «*napoli difesa», e quella
/// l'asterisco ce l'ha per davvero. Uno invece di 292 e' la risposta giusta.
///
/// Un testo vuoto non trova **niente**, non tutto. Per l'aritmetica delle
/// sottostringhe sarebbe contenuto in ogni nome, e questa funzione serve a
/// chiudere.
pub fn bersagli(nome: &str, processi: &[Processo], finestre: &[Finestra]) -> Vec<Bersaglio> {
    let t = nome.trim().to_lowercase();
    if t.is_empty() {
        return Vec::new();
    }
    let mut fuori = Vec::new();
    for p in processi {
        let titoli: Vec<String> = finestre
            .iter()
            .filter(|f| f.pid == p.pid)
            .map(|f| f.titolo.clone())
            .collect();
        if p.nome.to_lowercase().contains(&t)
            || titoli.iter().any(|x| x.to_lowercase().contains(&t))
        {
            fuori.push(Bersaglio {
                pid: p.pid,
                nome: p.nome.clone(),
                finestre: titoli,
            });
        }
    }
    fuori
}

/// Cosa legge chi deve approvare una chiusura.
///
/// Prima leggeva «Termina FORZATAMENTE 'notepad'» e basta. Ora legge i
/// nomi, i pid e i **titoli delle finestre** — e l'asterisco davanti a un
/// titolo, che in mezzo mondo di programmi vuol dire «non salvato», arriva
/// sotto gli occhi di chi decide invece di restare nascosto.
///
/// `trovati` e' `None` quando non si e' potuto chiedere al sistema: allora
/// si dice solo cio' che si sa senza guardare, e basta.
pub fn anteprima_chiusura(nome: &str, forza: bool, trovati: Option<&[Bersaglio]>) -> String {
    let semplice = format!(
        "{}'{nome}'",
        if forza {
            "Termina FORZATAMENTE "
        } else {
            "Chiude "
        }
    );
    let Some(trovati) = trovati else {
        return semplice;
    };
    if trovati.is_empty() {
        return format!("{semplice} — al momento non corrisponde nessun processo");
    }
    let verbo = if forza {
        "Termina FORZATAMENTE"
    } else {
        "Chiude"
    };
    let pezzi: Vec<String> = trovati
        .iter()
        .take(6)
        .map(|t| {
            let mut riga = format!("{} (pid {})", t.nome, t.pid);
            if !t.finestre.is_empty() {
                let titoli: Vec<String> = t
                    .finestre
                    .iter()
                    .take(3)
                    .map(|x| format!("«{x}»"))
                    .collect();
                riga.push_str(&format!(": {}", titoli.join(", ")));
            }
            riga
        })
        .collect();
    let quanti = trovati.len();
    let testa = format!(
        "{verbo} {quanti} {}",
        if quanti == 1 { "processo" } else { "processi" }
    );
    let coda = if quanti <= 6 {
        String::new()
    } else {
        format!(" e altri {}", quanti - 6)
    };
    // Tutte le finestre di tutti, non solo le tre che si mostrano: un lavoro
    // non salvato nella quarta finestra e' un lavoro non salvato lo stesso.
    let avviso = if trovati
        .iter()
        .any(|t| t.finestre.iter().any(|x| x.starts_with('*')))
    {
        "\n  ATTENZIONE: un titolo comincia per «*», che in molti programmi \
         vuol dire lavoro NON SALVATO."
    } else {
        ""
    };
    format!("{testa}: {}{coda}{avviso}", pezzi.join("; "))
}

/// Com'e' andata una chiusura, detto tutto.
///
/// Si dice anche cosa **non** e' andato: chiudere meta' di quello che si e'
/// chiesto e rispondere «fatto» e' peggio che fallire (D129). E se non si e'
/// chiuso niente e' un guasto, non una risposta.
pub fn esito_chiusura(
    chiusi: &[String],
    falliti: &[String],
    forza: bool,
) -> Result<String, String> {
    let primi: Vec<&str> = falliti.iter().take(4).map(|s| s.as_str()).collect();
    if chiusi.is_empty() {
        return Err(format!(
            "non sono riuscito a chiudere niente. {}",
            primi.join("; ")
        ));
    }
    let mut detto = format!(
        "{}{}",
        if forza {
            "Terminati: "
        } else {
            "Chiesto di chiudersi a: "
        },
        chiusi.join(", ")
    );
    if !falliti.is_empty() {
        detto.push_str(&format!("\nNon riusciti: {}", primi.join("; ")));
    }
    Ok(detto)
}

/// Quale finestra portare davanti per questo testo.
///
/// La prima che corrisponde **nell'ordine della pila**, cioe' la piu' in
/// alto: e' quasi sempre quella a cui si pensa. Se non ce n'e' nessuna si
/// dice quali ci sono, perche' «non trovata» senza alternative lascia il
/// modello a tentare nomi a caso.
pub fn scegli_finestra<'a>(testo: &str, finestre: &'a [Finestra]) -> Result<&'a Finestra, String> {
    let t = testo.trim().to_lowercase();
    if t.is_empty() {
        return Err("serve un titolo: un testo vuoto corrisponderebbe a tutto".into());
    }
    finestre
        .iter()
        .find(|w| w.titolo.to_lowercase().contains(&t) || w.processo.to_lowercase().contains(&t))
        .ok_or_else(|| {
            let aperte: Vec<String> = finestre
                .iter()
                .take(8)
                .map(|w| w.titolo.chars().take(40).collect())
                .collect();
            format!(
                "nessuna finestra corrispondente a '{testo}'. Aperte: {}",
                aperte.join(", ")
            )
        })
}

/// Cosa si dice quando Windows non lascia portare davanti una finestra.
///
/// Non e' un fallimento da nascondere: e' una regola di Windows, e l'utente
/// vede l'icona lampeggiare. Dirlo gli spiega cosa sta guardando; dire
/// «fatto» lo lascia a chiedersi perche' non e' successo niente (D142).
pub fn avanti_rifiutato(titolo: &str) -> String {
    format!(
        "Windows non ha permesso di portare davanti «{titolo}»: succede quando \
         il primo piano appartiene a un altro programma e l'utente non ha \
         appena interagito. L'icona nella barra sta lampeggiando: un clic la \
         porta avanti."
    )
}

/// I processi, dal piu' pesante, come tabella.
///
/// L'ordine a parita' di memoria e' quello del Python — pid e poi nome, tutti
/// e due a scendere — e non e' pignoleria: i processi di sistema pesano zero
/// quasi tutti, e con un ordine diverso le due meta' mostrerebbero **quali**
/// processi in coda in modo diverso.
pub fn tabella_processi(processi: &[Processo], filtro: &str, quanti: i64) -> String {
    let f = filtro.to_lowercase();
    let mut righe: Vec<(f64, u32, &str)> = processi
        .iter()
        .filter(|p| f.is_empty() || p.nome.to_lowercase().contains(&f))
        .map(|p| {
            (
                p.memoria_byte as f64 / 1024.0 / 1024.0,
                p.pid,
                p.nome.as_str(),
            )
        })
        .collect();
    righe.sort_by(|a, b| {
        b.0.partial_cmp(&a.0)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| b.1.cmp(&a.1))
            .then_with(|| b.2.cmp(a.2))
    });
    let quanti = quanti.max(1) as usize;
    let corpo: Vec<String> = righe
        .iter()
        .take(quanti)
        .map(|(m, pid, nome)| format!("{pid:>7}  {nome:<35} {m:8.0} MB"))
        .collect();
    format!(
        "PID      NOME                                 MEMORIA\n{}",
        corpo.join("\n")
    )
}

#[cfg(test)]
mod prove {
    use super::*;

    fn p(pid: u32, nome: &str) -> Processo {
        Processo {
            pid,
            nome: nome.into(),
            memoria_byte: 0,
        }
    }

    fn f(pid: u32, titolo: &str, processo: &str) -> Finestra {
        Finestra {
            handle: pid as i64 * 10,
            pid,
            titolo: titolo.into(),
            processo: processo.into(),
        }
    }

    #[test]
    fn gli_alias_si_riconoscono_e_il_resto_resta_com_e() {
        assert_eq!(risolvi("  Blocco Note ").unwrap(), "notepad.exe");
        assert_eq!(
            risolvi("C:\\Programmi\\x.exe ").unwrap(),
            "C:\\Programmi\\x.exe "
        );
        assert!(risolvi("   ").is_err());
    }

    #[test]
    fn l_asterisco_si_cerca_alla_lettera_anche_nei_titoli() {
        let processi = vec![
            p(1, "notepad.exe"),
            p(2, "chrome.exe"),
            p(3, "explorer.exe"),
        ];
        let finestre = vec![f(1, "*napoli difesa - Blocco note", "notepad.exe")];
        let t = bersagli("*", &processi, &finestre);
        assert_eq!(t.len(), 1, "uno invece di tutti: {t:?}");
        assert_eq!(t[0].pid, 1);
        assert!(bersagli("?", &processi, &finestre).is_empty());
        assert!(bersagli("[a-z]", &processi, &finestre).is_empty());
    }

    #[test]
    fn un_nome_vuoto_non_trova_niente_perche_serve_a_chiudere() {
        let processi = vec![p(1, "notepad.exe")];
        assert!(bersagli("", &processi, &[]).is_empty());
        assert!(bersagli("   ", &processi, &[]).is_empty());
    }

    #[test]
    fn l_anteprima_avvisa_del_lavoro_non_salvato_anche_oltre_la_terza_finestra() {
        let t = vec![Bersaglio {
            pid: 7,
            nome: "notepad.exe".into(),
            finestre: vec!["a".into(), "b".into(), "c".into(), "*quarta".into()],
        }];
        let a = anteprima_chiusura("notepad", true, Some(&t));
        assert!(
            a.starts_with("Termina FORZATAMENTE 1 processo: notepad.exe (pid 7)"),
            "{a}"
        );
        assert!(!a.contains("quarta»"), "si mostrano tre titoli: {a}");
        assert!(
            a.contains("NON SALVATO"),
            "ma l'avviso le guarda tutte: {a}"
        );
    }

    #[test]
    fn chiudere_meta_si_dice_e_non_chiudere_niente_e_un_guasto() {
        let ok =
            esito_chiusura(&["a (pid 1)".into()], &["b (pid 2): negato".into()], false).unwrap();
        assert_eq!(
            ok,
            "Chiesto di chiudersi a: a (pid 1)\nNon riusciti: b (pid 2): negato"
        );
        assert!(esito_chiusura(&[], &["b".into()], true).is_err());
    }

    #[test]
    fn senza_finestre_corrispondenti_si_dice_quali_ci_sono() {
        let finestre = vec![f(1, "Documento - Word", "WINWORD.EXE")];
        let e = scegli_finestra("excel", &finestre).unwrap_err();
        assert!(e.contains("Aperte: Documento - Word"), "{e}");
        assert_eq!(scegli_finestra("word", &finestre).unwrap().pid, 1);
        assert!(scegli_finestra(" ", &finestre).is_err());
    }

    #[test]
    fn il_taglio_delle_applicazioni_si_dichiara() {
        let nomi: Vec<String> = (0..300).map(|i| format!("App {i:03}")).collect();
        let t = elenco_installate(&nomi, "");
        assert_eq!(t.lines().count(), MASSIMO_APP + 1);
        assert!(t.ends_with("[... e altre 50 su 300: restringi con «filter» per vederle]"));
        assert_eq!(
            elenco_installate(&nomi, "zzz"),
            "Nessuna applicazione trovata."
        );
    }
}
