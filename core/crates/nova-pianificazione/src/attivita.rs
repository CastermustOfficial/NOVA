//! Le attivita' pianificate di Windows, descritte come **dato**.
//!
//! Gemello di `nova/attivita.py` e delle parti di `nova/tools/tempo.py` e
//! `create_reminder` che decidono cosa scrivere: qui si compone l'XML che
//! l'Utilita' di pianificazione legge, si capisce «quando» e «ogni quanto»,
//! e si da' il nome all'attivita'. Tutto questo si prova su qualunque
//! macchina, e un banco lo confronta col Python carattere per carattere.
//!
//! Registrare l'attivita' (`schtasks /Create /XML`) e' una riga sola, e sta
//! in fondo: solo su Windows.
//!
//! **Perche' un XML e non `schtasks /TR`** (D146, D149): nell'XML il
//! programma e i suoi argomenti sono due campi distinti, non una riga di
//! comando dentro un'altra; e l'orario e' ISO 8601, non la data nella lingua
//! del sistema — `03/09` e' il 3 settembre in Italia e il 9 marzo negli
//! Stati Uniti.

use nova_calendario::DataOra;

/// I giorni della settimana come li scrive l'Utilita' di pianificazione.
pub const GIORNI_XML: [(&str, &str); 7] = [
    ("lunedi", "Monday"),
    ("martedi", "Tuesday"),
    ("mercoledi", "Wednesday"),
    ("giovedi", "Thursday"),
    ("venerdi", "Friday"),
    ("sabato", "Saturday"),
    ("domenica", "Sunday"),
];

/// Il nome inglese del giorno, lunedi' = 0 (`strftime("%A")`).
const IN_INGLESE: [&str; 7] = [
    "Monday",
    "Tuesday",
    "Wednesday",
    "Thursday",
    "Friday",
    "Saturday",
    "Sunday",
];

/// Il prefisso dei compiti che NOVA si da' da fare: solo quelli si possono
/// togliere dal modello.
pub const PREFISSO_COMPITI: &str = "NOVA_Compito_";
/// Il prefisso dei promemoria.
pub const PREFISSO_PROMEMORIA: &str = "NOVA_Promemoria_";

/// I cinque caratteri che in XML vogliono dire qualcos'altro.
pub fn escape(testo: &str) -> String {
    testo
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

/// Ogni quanto si ripete: mai, ogni giorno, ogni settimana (in un giorno),
/// ogni mese.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Ripeti {
    Mai,
    Giorno,
    Settimana(String),
    Mese,
}

fn trigger(dt: DataOra, ripeti: &Ripeti) -> String {
    let inizio = dt.iso();
    let dentro = match ripeti {
        Ripeti::Mai => {
            // Una volta sola: si mette anche una fine, cosi' l'attivita' si
            // cancella da sola.
            let fine = dt.piu_giorni(1).iso();
            return format!(
                "<TimeTrigger><StartBoundary>{inizio}</StartBoundary>\
                 <EndBoundary>{fine}</EndBoundary><Enabled>true</Enabled></TimeTrigger>"
            );
        }
        Ripeti::Giorno => {
            "<ScheduleByDay><DaysInterval>1</DaysInterval></ScheduleByDay>".to_string()
        }
        Ripeti::Settimana(giorno) => {
            let quale = GIORNI_XML
                .iter()
                .find(|(g, _)| g == giorno)
                .map(|(_, e)| *e)
                .unwrap_or(IN_INGLESE[dt.giorno_settimana() as usize % 7]);
            format!(
                "<ScheduleByWeek><WeeksInterval>1</WeeksInterval>\
                 <DaysOfWeek><{quale} /></DaysOfWeek></ScheduleByWeek>"
            )
        }
        Ripeti::Mese => format!(
            "<ScheduleByMonth><DaysOfMonth><Day>{}</Day></DaysOfMonth>\
             <Months><January /><February /><March /><April /><May /><June />\
             <July /><August /><September /><October /><November /><December />\
             </Months></ScheduleByMonth>",
            dt.giorno
        ),
    };
    format!(
        "<CalendarTrigger><StartBoundary>{inizio}</StartBoundary>\
         <Enabled>true</Enabled>{dentro}</CalendarTrigger>"
    )
}

/// L'attivita' come la descrive Windows, byte per byte come `attivita.xml`.
pub fn xml(
    dt: DataOra,
    comando: &str,
    argomenti: &str,
    descrizione: &str,
    ripeti: &Ripeti,
    durata_massima: &str,
) -> String {
    let scadenza = if *ripeti == Ripeti::Mai {
        "<DeleteExpiredTaskAfter>PT1M</DeleteExpiredTaskAfter>"
    } else {
        ""
    };
    let descrizione: String = descrizione.chars().take(400).collect();
    format!(
        r#"<?xml version="1.0" encoding="UTF-16"?>
<Task version="1.2" xmlns="http://schemas.microsoft.com/windows/2004/02/mit/task">
  <RegistrationInfo>
    <Description>{}</Description>
    <Author>NOVA</Author>
  </RegistrationInfo>
  <Triggers>{}</Triggers>
  <Principals>
    <Principal id="Author">
      <LogonType>InteractiveToken</LogonType>
      <RunLevel>LeastPrivilege</RunLevel>
    </Principal>
  </Principals>
  <Settings>
    <MultipleInstancesPolicy>IgnoreNew</MultipleInstancesPolicy>
    <DisallowStartIfOnBatteries>false</DisallowStartIfOnBatteries>
    <StopIfGoingOnBatteries>false</StopIfGoingOnBatteries>
    <StartWhenAvailable>true</StartWhenAvailable>
    <Enabled>true</Enabled>
    <Hidden>false</Hidden>
    <ExecutionTimeLimit>{}</ExecutionTimeLimit>
    {}
  </Settings>
  <Actions Context="Author">
    <Exec>
      <Command>{}</Command>
      <Arguments>{}</Arguments>
    </Exec>
  </Actions>
</Task>
"#,
        escape(&descrizione),
        trigger(dt, ripeti),
        durata_massima,
        scadenza,
        escape(comando),
        escape(argomenti)
    )
}

/// «HH:MM» (oggi, o domani se e' gia' passato) o «YYYY-MM-DD HH:MM».
///
/// `None` quando non si legge: il messaggio lo sceglie chi chiama, perche'
/// il promemoria e il compito lo dicono con parole diverse.
pub fn quando(testo: &str, adesso: DataOra) -> Option<DataOra> {
    let t = testo.trim();
    if t.chars().count() <= 5 {
        let (h, m) = t.split_once(':')?;
        let ok = |s: &str| !s.is_empty() && s.len() <= 2 && s.bytes().all(|b| b.is_ascii_digit());
        if !ok(h) || !ok(m) {
            return None;
        }
        let (h, m): (u32, u32) = (h.parse().ok()?, m.parse().ok()?);
        if h > 23 || m > 59 {
            return None;
        }
        let dt = adesso.con_orario(h, m);
        return Some(if dt < adesso { dt.piu_giorni(1) } else { dt });
    }
    let t = t.replacen('T', " ", 1);
    let (data, ora) = match t.split_once(' ') {
        Some((d, o)) => (d, Some(o)),
        None => (t.as_str(), None),
    };
    let d: Vec<&str> = data.split('-').collect();
    if d.len() != 3 || d[0].len() != 4 || d[1].len() != 2 || d[2].len() != 2 {
        return None;
    }
    let (anno, mese, giorno): (i32, u32, u32) =
        (d[0].parse().ok()?, d[1].parse().ok()?, d[2].parse().ok()?);
    if !(1..=12).contains(&mese)
        || giorno == 0
        || giorno > nova_calendario::giorni_del_mese(anno, mese)
    {
        return None;
    }
    let (h, m, s) = match ora {
        None => (0, 0, 0),
        Some(o) => {
            let p: Vec<&str> = o.split(':').collect();
            if !(2..=3).contains(&p.len()) || p.iter().any(|x| x.len() != 2) {
                return None;
            }
            let n = |x: &str| x.parse::<u32>().ok();
            (n(p[0])?, n(p[1])?, if p.len() == 3 { n(p[2])? } else { 0 })
        }
    };
    if h > 23 || m > 59 || s > 59 {
        return None;
    }
    Some(DataOra::nuova(anno, mese, giorno, h, m, s))
}

/// Il «ripeti» di `pianifica`, o perche' non si capisce.
pub fn ripeti(testo: &str) -> Result<Ripeti, String> {
    let r = testo.trim().to_lowercase();
    if r.is_empty() {
        return Ok(Ripeti::Mai);
    }
    match r.as_str() {
        "ogni giorno" | "giornaliero" | "ogni giorni" => return Ok(Ripeti::Giorno),
        "ogni settimana" | "settimanale" => return Ok(Ripeti::Settimana(String::new())),
        "ogni mese" | "mensile" => return Ok(Ripeti::Mese),
        _ => {}
    }
    if let Some(g) = r.strip_prefix("ogni ") {
        let g = g.replace('\'', "");
        let g = g.trim();
        if GIORNI_XML.iter().any(|(x, _)| *x == g) {
            return Ok(Ripeti::Settimana(g.to_string()));
        }
    }
    Err(format!(
        "non capisco «{testo}». Usa: vuoto, 'ogni giorno', 'ogni settimana', 'ogni mese' \
         oppure 'ogni lunedi' (o un altro giorno)."
    ))
}

/// Il nome di un compito: il prefisso e un pezzo dell'etichetta, con solo
/// lettere, cifre, spazi, trattini e sottolineature.
pub fn nome_compito(etichetta: &str, dt: DataOra) -> String {
    let corta: String = etichetta.chars().take(40).collect();
    let pulita: String = corta
        .trim()
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || " -_".contains(c) {
                c
            } else {
                '_'
            }
        })
        .collect();
    let pulita = pulita.trim();
    if pulita.is_empty() {
        format!("{PREFISSO_COMPITI}{}", compatta(dt))
    } else {
        format!("{PREFISSO_COMPITI}{pulita}")
    }
}

/// `strftime("%Y%m%d%H%M%S")`.
pub fn compatta(dt: DataOra) -> String {
    format!(
        "{:04}{:02}{:02}{:02}{:02}{:02}",
        dt.anno, dt.mese, dt.giorno, dt.ora, dt.minuto, dt.secondo
    )
}

/// `strftime("%d/%m/%Y alle %H:%M")`, `strftime("%d/%m/%Y %H:%M")`.
pub fn leggibile(dt: DataOra, alle: bool) -> String {
    format!(
        "{:02}/{:02}/{:04}{}{:02}:{:02}",
        dt.giorno,
        dt.mese,
        dt.anno,
        if alle { " alle " } else { " " },
        dt.ora,
        dt.minuto
    )
}

/// Un compito che NOVA si da' da fare (`pianifica`): come si chiama, quando
/// parte, e ogni quanto.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Compito {
    pub nome: String,
    pub quando: DataOra,
    pub ripeti: Ripeti,
}

/// Le regole di `pianifica`, nell'ordine in cui le controlla il Python.
pub fn compito(
    istruzione: &str,
    quando_testo: &str,
    ripeti_testo: &str,
    nome: &str,
    adesso: DataOra,
) -> Result<Compito, String> {
    let istruzione = istruzione.trim();
    if istruzione.is_empty() {
        return Err("serve l'istruzione: cosa deve fare NOVA".into());
    }
    let dt = quando(quando_testo, adesso)
        .ok_or("l'ora va scritta come 'HH:MM' oppure 'YYYY-MM-DD HH:MM'")?;
    let etichetta = if nome.is_empty() { istruzione } else { nome };
    Ok(Compito {
        nome: nome_compito(etichetta, dt),
        quando: dt,
        ripeti: ripeti(ripeti_testo)?,
    })
}

/// Le regole di `create_reminder`: il nome del promemoria e quando suona.
pub fn promemoria(quando_testo: &str, adesso: DataOra) -> Result<(String, DataOra), String> {
    let t = quando_testo.trim();
    let dt =
        quando(t, adesso).ok_or("formato ora non valido, usa 'YYYY-MM-DD HH:MM' oppure 'HH:MM'")?;
    if dt <= adesso {
        return Err(format!(
            "«{t}» e' gia' passato: un promemoria per il passato non suonerebbe mai"
        ));
    }
    Ok((format!("{PREFISSO_PROMEMORIA}{}", compatta(dt)), dt))
}

/// Un'attivita' letta da `schtasks /Query /FO LIST /V`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Letta {
    pub nome: String,
    pub prossima: String,
    pub azione: String,
}

/// Le attivita' di NOVA nell'elenco di `schtasks`.
///
/// I nomi dei campi cambiano con la lingua di Windows — «Prossima
/// esecuzione» e «Next Run Time» — e si riconoscono per pezzi di parola: una
/// corrispondenza esatta darebbe un elenco vuoto senza dire perche'.
pub fn elenco(uscita: &str, prefisso: &str) -> Vec<Letta> {
    let uscita = uscita.replace("\r\n", "\n");
    uscita
        .split("\n\n")
        .filter(|b| b.contains(prefisso))
        .map(|b| {
            let mut l = Letta {
                nome: String::new(),
                prossima: String::new(),
                azione: String::new(),
            };
            for riga in b.lines() {
                let Some((k, v)) = riga.split_once(':') else {
                    continue;
                };
                let (k, v) = (k.trim().to_lowercase(), v.trim());
                if k.contains("nome attivit") || k.contains("taskname") {
                    l.nome = v.trim_matches('\\').to_string();
                } else if k.contains("prossima esecuzione") || k.contains("next run") {
                    l.prossima = v.to_string();
                } else if k.contains("da eseguire") || k.contains("task to run") {
                    l.azione = v.to_string();
                }
            }
            l
        })
        .collect()
}

// --------------------------------------------------------- Windows

/// Registra l'attivita', o dice perche' Windows l'ha rifiutata.
#[cfg(windows)]
pub fn registra(nome: &str, testo_xml: &str) -> Result<(), String> {
    use std::os::windows::process::CommandExt;
    let dove = std::env::temp_dir().join("nova-attivita");
    std::fs::create_dir_all(&dove).map_err(|e| e.to_string())?;
    let file = dove.join(format!("{nome}.xml"));
    // `schtasks /XML` vuole UTF-16 col suo segno davanti.
    let mut byte = vec![0xFF, 0xFE];
    for u in testo_xml.encode_utf16() {
        byte.extend_from_slice(&u.to_le_bytes());
    }
    std::fs::write(&file, byte).map_err(|e| e.to_string())?;
    let esito = std::process::Command::new("schtasks")
        .args(["/Create", "/TN", nome, "/XML"])
        .arg(&file)
        .arg("/F")
        .creation_flags(0x0800_0000)
        .output();
    let _ = std::fs::remove_file(&file);
    let o = esito.map_err(|e| e.to_string())?;
    if o.status.success() {
        return Ok(());
    }
    let detto = String::from_utf8_lossy(if o.stderr.is_empty() {
        &o.stdout
    } else {
        &o.stderr
    })
    .trim()
    .chars()
    .take(400)
    .collect();
    Err(detto)
}

#[cfg(not(windows))]
pub fn registra(_nome: &str, _testo_xml: &str) -> Result<(), String> {
    Err(NON_QUI.into())
}

/// Toglie l'attivita'. `Err` col motivo se non si e' potuta togliere.
#[cfg(windows)]
pub fn togli(nome: &str) -> Result<(), String> {
    use std::os::windows::process::CommandExt;
    let o = std::process::Command::new("schtasks")
        .args(["/Delete", "/TN", nome, "/F"])
        .creation_flags(0x0800_0000)
        .output()
        .map_err(|e| e.to_string())?;
    if o.status.success() {
        return Ok(());
    }
    Err(String::from_utf8_lossy(if o.stderr.is_empty() {
        &o.stdout
    } else {
        &o.stderr
    })
    .trim()
    .chars()
    .take(300)
    .collect())
}

#[cfg(not(windows))]
pub fn togli(_nome: &str) -> Result<(), String> {
    Err(NON_QUI.into())
}

/// L'uscita di `schtasks /Query /FO LIST /V`.
#[cfg(windows)]
pub fn interroga() -> Result<String, String> {
    use std::os::windows::process::CommandExt;
    let o = std::process::Command::new("schtasks")
        .args(["/Query", "/FO", "LIST", "/V"])
        .creation_flags(0x0800_0000)
        .output()
        .map_err(|e| e.to_string())?;
    if !o.status.success() {
        return Err("non riesco a leggere le attivita' pianificate".into());
    }
    Ok(String::from_utf8_lossy(&o.stdout).into_owned())
}

#[cfg(not(windows))]
pub fn interroga() -> Result<String, String> {
    Err(NON_QUI.into())
}

/// Quel che si dice fuori da Windows.
pub const NON_QUI: &str =
    "le attivita' pianificate per ora le so creare solo su Windows (l'Utilita' di pianificazione)";

#[cfg(test)]
mod prove {
    use super::*;

    fn d(s: &str) -> DataOra {
        DataOra::da_iso(s).unwrap()
    }

    #[test]
    fn quando_si_legge() {
        let adesso = d("2026-09-27T20:30:00");
        assert_eq!(quando("21:05", adesso), Some(d("2026-09-27T21:05:00")));
        assert_eq!(
            quando("9:05", adesso),
            Some(d("2026-09-28T09:05:00")),
            "passato: domani"
        );
        assert_eq!(
            quando("20:30", adesso),
            Some(d("2026-09-27T20:30:00")),
            "adesso non e' passato"
        );
        assert_eq!(
            quando("2026-12-31 23:59", adesso),
            Some(d("2026-12-31T23:59:00"))
        );
        assert_eq!(
            quando("2026-12-31T08:00:30", adesso),
            Some(d("2026-12-31T08:00:30"))
        );
        assert_eq!(quando("2026-12-31", adesso), Some(d("2026-12-31T00:00:00")));
        for no in [
            "",
            "domani",
            "25:00",
            "2026-02-30 10:00",
            "2026-9-3 10:00",
            "10:0x",
        ] {
            assert_eq!(quando(no, adesso), None, "{no}");
        }
    }

    #[test]
    fn ripeti_si_legge() {
        assert_eq!(ripeti(""), Ok(Ripeti::Mai));
        assert_eq!(ripeti("Ogni Giorno"), Ok(Ripeti::Giorno));
        assert_eq!(
            ripeti("ogni lunedi'"),
            Ok(Ripeti::Settimana("lunedi".into()))
        );
        assert_eq!(ripeti("settimanale"), Ok(Ripeti::Settimana(String::new())));
        assert_eq!(ripeti("mensile"), Ok(Ripeti::Mese));
        assert!(ripeti("ogni tanto")
            .unwrap_err()
            .starts_with("non capisco «ogni tanto»"));
    }

    #[test]
    fn il_nome_di_un_compito() {
        let dt = d("2026-09-27T08:00:00");
        assert_eq!(
            nome_compito("Controlla l'agenda è", dt),
            "NOVA_Compito_Controlla l_agenda è"
        );
        assert_eq!(nome_compito("   ", dt), "NOVA_Compito_20260927080000");
        assert_eq!(nome_compito("!!!", dt), "NOVA_Compito____");
    }

    #[test]
    fn l_xml_ha_il_programma_e_gli_argomenti_separati() {
        let x = xml(
            d("2026-09-27T08:00:00"),
            "C:\\NOVA\\bin\\nova.exe",
            "chiedi --da-file \"C:\\t\\x.txt\"",
            "controlla l'agenda & <tutto>",
            &Ripeti::Settimana(String::new()),
            "PT30M",
        );
        assert!(x.contains("<Command>C:\\NOVA\\bin\\nova.exe</Command>"));
        assert!(x.contains("<Arguments>chiedi --da-file &quot;C:\\t\\x.txt&quot;</Arguments>"));
        assert!(x.contains("controlla l&apos;agenda &amp; &lt;tutto&gt;"));
        assert!(
            x.contains("<DaysOfWeek><Sunday /></DaysOfWeek>"),
            "il 27/9/2026 e' domenica"
        );
        assert!(!x.contains("DeleteExpired"));
        let una = xml(d("2026-12-31T23:00:00"), "a", "", "", &Ripeti::Mai, "PT5M");
        assert!(una.contains("<EndBoundary>2027-01-01T23:00:00</EndBoundary>"));
        assert!(una.contains("<DeleteExpiredTaskAfter>PT1M</DeleteExpiredTaskAfter>"));
    }

    #[test]
    fn l_elenco_si_legge_in_due_lingue() {
        let it = "Nome host: PC\r\nNome attività: \\NOVA_Compito_backup\r\nProssima esecuzione: 28/09/2026 09:00:00\r\n\
                  Attività da eseguire: C:\\n\\nova.exe chiedi\r\n\r\nNome attività: \\Altro\r\n";
        let en = "TaskName: \\NOVA_Compito_x\nNext Run Time: 9/28/2026 9:00:00 AM\nTask To Run: nova.exe\n";
        assert_eq!(
            elenco(it, PREFISSO_COMPITI),
            vec![Letta {
                nome: "NOVA_Compito_backup".into(),
                prossima: "28/09/2026 09:00:00".into(),
                azione: "C:\\n\\nova.exe chiedi".into()
            }]
        );
        assert_eq!(
            elenco(en, PREFISSO_COMPITI)[0].prossima,
            "9/28/2026 9:00:00 AM"
        );
    }
}
