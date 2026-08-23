//! Altoparlanti e microfono.
//!
//! Un solo modo di parlare all'audio su tre sistemi operativi: cpal sta sopra
//! WASAPI, CoreAudio e ALSA. Qui dentro non c'e' niente di specifico per
//! Windows, ed e' voluto.
//!
//! Il punto delicato e' la frequenza. Kokoro produce 24000 campioni al
//! secondo; quasi tutte le schede audio vogliono 48000. Chiedere alla scheda
//! di suonare a 24000 a volte funziona e a volte no, quindi si ricampiona
//! sempre noi: e' prevedibile, e un'interpolazione lineare su una voce non si
//! sente.

use std::sync::{Arc, Condvar, Mutex};
use std::time::Duration;

use anyhow::{anyhow, Context, Result};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};

/// Ricampiona per interpolazione lineare.
pub fn ricampiona(campioni: &[f32], da: u32, a: u32) -> Vec<f32> {
    if da == a || campioni.is_empty() {
        return campioni.to_vec();
    }
    let rapporto = da as f64 / a as f64;
    let quanti = ((campioni.len() as f64) / rapporto).floor() as usize;
    let mut fuori = Vec::with_capacity(quanti);
    for i in 0..quanti {
        let posizione = i as f64 * rapporto;
        let sotto = posizione.floor() as usize;
        let frazione = (posizione - sotto as f64) as f32;
        let a0 = campioni[sotto.min(campioni.len() - 1)];
        let a1 = campioni[(sotto + 1).min(campioni.len() - 1)];
        fuori.push(a0 + (a1 - a0) * frazione);
    }
    fuori
}

/// Distribuisce un mono su piu' canali.
fn a_canali(campioni: &[f32], canali: usize) -> Vec<f32> {
    if canali <= 1 {
        return campioni.to_vec();
    }
    let mut fuori = Vec::with_capacity(campioni.len() * canali);
    for c in campioni {
        for _ in 0..canali {
            fuori.push(*c);
        }
    }
    fuori
}

/// Suona dei campioni mono e ritorna quando ha finito.
pub fn riproduci(campioni: &[f32], frequenza: u32) -> Result<()> {
    if campioni.is_empty() {
        return Ok(());
    }
    let host = cpal::default_host();
    let uscita = host
        .default_output_device()
        .ok_or_else(|| anyhow!("nessun dispositivo di uscita audio"))?;
    let configurazione = uscita
        .default_output_config()
        .context("configurazione dell'uscita audio")?;
    let canali = configurazione.channels() as usize;
    let frequenza_scheda = configurazione.sample_rate().0;

    let pronti = a_canali(&ricampiona(campioni, frequenza, frequenza_scheda), canali);
    let durata = Duration::from_secs_f64(
        pronti.len() as f64 / (frequenza_scheda as f64 * canali as f64),
    );

    // Il flusso gira su un thread suo e ci dice quando ha svuotato tutto:
    // senza l'attesa esplicita, la funzione tornerebbe e il flusso verrebbe
    // chiuso a meta' frase.
    let coda = Arc::new(Mutex::new((pronti, 0usize)));
    let finito = Arc::new((Mutex::new(false), Condvar::new()));
    let coda_stream = Arc::clone(&coda);
    let finito_stream = Arc::clone(&finito);

    let errore = |e| tracing::warn!(errore = %e, "flusso audio");
    let flusso = match configurazione.sample_format() {
        cpal::SampleFormat::F32 => uscita.build_output_stream(
            &configurazione.clone().into(),
            move |buffer: &mut [f32], _| {
                let mut c = coda_stream.lock().unwrap();
                let (dati, letti) = &mut *c;
                for posto in buffer.iter_mut() {
                    *posto = if *letti < dati.len() {
                        let v = dati[*letti];
                        *letti += 1;
                        v
                    } else {
                        0.0
                    };
                }
                if *letti >= dati.len() {
                    let (m, cv) = &*finito_stream;
                    *m.lock().unwrap() = true;
                    cv.notify_all();
                }
            },
            errore,
            None,
        )?,
        cpal::SampleFormat::I16 => uscita.build_output_stream(
            &configurazione.clone().into(),
            move |buffer: &mut [i16], _| {
                let mut c = coda_stream.lock().unwrap();
                let (dati, letti) = &mut *c;
                for posto in buffer.iter_mut() {
                    *posto = if *letti < dati.len() {
                        let v = dati[*letti];
                        *letti += 1;
                        (v.clamp(-1.0, 1.0) * 32767.0) as i16
                    } else {
                        0
                    };
                }
                if *letti >= dati.len() {
                    let (m, cv) = &*finito_stream;
                    *m.lock().unwrap() = true;
                    cv.notify_all();
                }
            },
            errore,
            None,
        )?,
        altro => return Err(anyhow!("formato audio non gestito: {altro:?}")),
    };
    flusso.play().context("avvio della riproduzione")?;

    // Si aspetta il segnale, con un tetto: se la scheda si pianta non si
    // resta bloccati per sempre a meta' di una frase.
    let (m, cv) = &*finito;
    let mut fatto = m.lock().unwrap();
    let limite = durata + Duration::from_secs(3);
    let inizio = std::time::Instant::now();
    while !*fatto && inizio.elapsed() < limite {
        let (nuovo, _) = cv.wait_timeout(fatto, Duration::from_millis(100)).unwrap();
        fatto = nuovo;
    }
    drop(fatto);
    // Un filo di coda: il buffer della scheda ha ancora qualcosa dentro.
    std::thread::sleep(Duration::from_millis(120));
    Ok(())
}

/// I dispositivi che ci sono, per poterli scegliere.
pub fn dispositivi() -> (Vec<String>, Vec<String>) {
    let host = cpal::default_host();
    let ingressi = host
        .input_devices()
        .map(|d| d.filter_map(|x| x.name().ok()).collect())
        .unwrap_or_default();
    let uscite = host
        .output_devices()
        .map(|d| d.filter_map(|x| x.name().ok()).collect())
        .unwrap_or_default();
    (ingressi, uscite)
}

// ------------------------------------------------------------ registrazione

/// Quanto forte deve essere il segnale per contare come parlato.
///
/// Non e' una soglia di volume assoluta: il rumore di fondo cambia da stanza a
/// stanza. Si misura il fondo nei primi istanti e si parla di «sopra il
/// fondo», altrimenti in una stanza silenziosa non parte mai e in una rumorosa
/// non si ferma mai.
const SOPRA_IL_FONDO: f32 = 3.5;
/// Il minimo assoluto: sotto questo e' silenzio anche in una camera anecoica.
const PAVIMENTO: f32 = 0.004;

/// Un pezzo di ascolto: campioni mono a 16 kHz, che e' cio' che vuole Whisper.
pub struct Ascolto {
    pub campioni: Vec<f32>,
    pub frequenza: u32,
    pub picco: f32,
    pub fermato_dal_silenzio: bool,
    pub microfono: String,
    /// Falso = l'attesa e' scaduta senza che nessuno dicesse niente.
    pub ha_parlato: bool,
    /// Di quanto e' stato alzato il volume. 1.0 = non serviva.
    pub guadagno: f32,
}

/// Il microfono non consegna niente.
///
/// Non e' un errore come gli altri: e' uno *stato*, e passa da solo quando si
/// abbassa il braccetto o si smuta il dispositivo. Distinguerlo serve a chi
/// ascolta in continuazione — spegnere il risveglio perche' il microfono e'
/// muto vuol dire che l'utente abbassa il braccetto, chiama NOVA e non
/// succede niente, senza sapere perche'.
///
/// Il messaggio elenca le cause vere, in ordine di frequenza. «Non ha sentito
/// niente» da solo manda a cercare un bug nel software, che e' il posto
/// sbagliato: su queste cuffie il braccetto alzato muta il microfono, ed e' la
/// spiegazione giusta nove volte su dieci.
#[derive(Debug, Clone)]
pub struct MicrofonoMuto {
    pub microfono: String,
    pub picco: f32,
}

impl std::fmt::Display for MicrofonoMuto {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "il microfono «{}» non ha sentito niente (picco {:.5}). Di solito e' una di \
             queste: il braccetto delle cuffie e' alzato (su molte cuffie da gioco alzarlo \
             muta il microfono), il dispositivo e' mutato in Windows, oppure e' l'ingresso \
             sbagliato — cambialo con voice.microfono.",
            self.microfono, self.picco
        )
    }
}

impl std::error::Error for MicrofonoMuto {}

/// Ascolta finche' non cala il silenzio, o finche' non scade il tempo.
///
/// `silenzio_s` e' quanto silenzio serve per considerare finita la frase: e'
/// il parametro che decide se l'assistente ti interrompe a meta' pensiero o
/// se ti fa aspettare dopo che hai finito.
/// Il microfono da usare: quello scelto per nome, o il predefinito.
///
/// Serve poterlo scegliere perche' il predefinito di sistema spesso non e'
/// quello giusto — su questa macchina era un dispositivo virtuale che non
/// sente niente, e undici secondi di ascolto avevano picco zero.
fn microfono(nome: Option<&str>) -> Result<cpal::Device> {
    let host = cpal::default_host();
    if let Some(cercato) = nome.map(str::trim).filter(|s| !s.is_empty()) {
        let atteso = cercato.to_lowercase();
        if let Ok(elenco) = host.input_devices() {
            for d in elenco {
                if d.name().map(|n| n.to_lowercase().contains(&atteso)).unwrap_or(false) {
                    return Ok(d);
                }
            }
        }
        tracing::warn!(microfono = cercato, "non trovato, uso il predefinito");
    }
    host.default_input_device()
        .ok_or_else(|| anyhow!("nessun microfono disponibile"))
}

pub fn ascolta(massimo_s: f32, silenzio_s: f32, frequenza: u32) -> Result<Ascolto> {
    ascolta_da(None, massimo_s, silenzio_s, frequenza)
}

pub fn ascolta_da(nome: Option<&str>, massimo_s: f32, silenzio_s: f32, frequenza: u32)
    -> Result<Ascolto> {
    // Senza attesa dichiarata: si comincia a contare subito, com'era prima.
    ascolta_con_attesa(nome, 0.0, massimo_s, silenzio_s, frequenza)
}

/// Aspetta che si cominci a parlare, poi registra fino al silenzio.
///
/// `attesa_inizio_s` e' il pezzo che mancava: senza, chi ascolta parte a
/// cronometro e chi parla deve indovinare il momento giusto. Con una finestra
/// da 30 secondi in cui *non succede niente finche' non parli*, il momento lo
/// decide chi parla — che e' l'unico modo sensato.
pub fn ascolta_con_attesa(nome: Option<&str>, attesa_inizio_s: f32, massimo_s: f32,
                          silenzio_s: f32, frequenza: u32) -> Result<Ascolto> {
    use cpal::traits::StreamTrait;

    let ingresso = microfono(nome)?;
    let nome_usato = ingresso.name().unwrap_or_else(|_| "?".into());
    let configurazione = ingresso
        .default_input_config()
        .context("configurazione del microfono")?;
    let canali = configurazione.channels() as usize;
    let frequenza_scheda = configurazione.sample_rate().0;

    let raccolti: Arc<Mutex<Vec<f32>>> = Arc::new(Mutex::new(Vec::new()));
    let raccolti_stream = Arc::clone(&raccolti);
    let errore = |e| tracing::warn!(errore = %e, "flusso del microfono");

    let flusso = match configurazione.sample_format() {
        cpal::SampleFormat::F32 => ingresso.build_input_stream(
            &configurazione.clone().into(),
            move |dati: &[f32], _| {
                let mut v = raccolti_stream.lock().unwrap();
                // media dei canali: un microfono stereo non serve a capire
                for pezzo in dati.chunks(canali.max(1)) {
                    v.push(pezzo.iter().sum::<f32>() / pezzo.len() as f32);
                }
            },
            errore,
            None,
        )?,
        cpal::SampleFormat::I16 => ingresso.build_input_stream(
            &configurazione.clone().into(),
            move |dati: &[i16], _| {
                let mut v = raccolti_stream.lock().unwrap();
                for pezzo in dati.chunks(canali.max(1)) {
                    let somma: f32 = pezzo.iter().map(|x| *x as f32 / 32768.0).sum();
                    v.push(somma / pezzo.len() as f32);
                }
            },
            errore,
            None,
        )?,
        altro => return Err(anyhow!("formato del microfono non gestito: {altro:?}")),
    };
    flusso.play().context("avvio dell'ascolto")?;

    let passo = Duration::from_millis(100);
    let mut fondo: Option<f32> = None;
    let mut misurati = 0usize;
    let mut letti = 0usize;
    let mut silenzio_da = 0.0f32;
    let mut ha_parlato = false;
    let mut picco_totale = 0.0f32;
    let inizio = std::time::Instant::now();
    let mut inizio_parlato: Option<std::time::Instant> = None;
    let mut fermato_dal_silenzio = false;
    let mut scartati = 0usize;
    let limite_totale = massimo_s + attesa_inizio_s;

    while inizio.elapsed().as_secs_f32() < limite_totale {
        std::thread::sleep(passo);
        let nuovi = {
            let v = raccolti.lock().unwrap();
            let n = v[letti.min(v.len())..].to_vec();
            letti = v.len();
            n
        };
        if nuovi.is_empty() {
            continue;
        }
        let energia = (nuovi.iter().map(|x| x * x).sum::<f32>() / nuovi.len() as f32).sqrt();
        picco_totale = picco_totale.max(energia);

        // I primi mezzo secondo servono a misurare la stanza, non a decidere.
        if misurati < 5 {
            misurati += 1;
            fondo = Some(match fondo {
                Some(f) => f.max(energia),
                None => energia,
            });
            continue;
        }
        let soglia = (fondo.unwrap_or(PAVIMENTO) * SOPRA_IL_FONDO).max(PAVIMENTO);
        if energia > soglia {
            if !ha_parlato {
                // Il primo suono: da qui parte il cronometro vero, e cio' che
                // e' stato registrato aspettando si butta — sono i secondi in
                // cui non stavi ancora parlando.
                ha_parlato = true;
                inizio_parlato = Some(std::time::Instant::now());
                scartati = letti.saturating_sub(nuovi.len());
            }
            silenzio_da = 0.0;
        } else if ha_parlato {
            silenzio_da += passo.as_secs_f32();
            if silenzio_da >= silenzio_s {
                fermato_dal_silenzio = true;
                break;
            }
        }
        if let Some(t) = inizio_parlato {
            if t.elapsed().as_secs_f32() >= massimo_s {
                break;      // parli da troppo: si chiude comunque
            }
        } else if attesa_inizio_s > 0.0 && inizio.elapsed().as_secs_f32() >= attesa_inizio_s {
            break;          // nessuno ha parlato entro l'attesa
        }
    }
    drop(flusso);

    let tutti = raccolti.lock().unwrap().clone();
    // Prima dell'inizio si tiene quasi un secondo intero. Non e' prudenza
    // generica: misurato, con un quarto di secondo la parola di risveglio
    // spariva del tutto — «Nova, che ore sono» arrivava a whisper come «Che
    // ore sono?». La parola che fa scattare tutto e' esattamente quella che
    // sta sotto la soglia mentre la voce parte.
    let margine = (frequenza_scheda as f32 * 0.9) as usize;
    let da = scartati.saturating_sub(margine).min(tutti.len());
    let grezzi = if ha_parlato { tutti[da..].to_vec() } else { tutti };
    let campioni = ricampiona(&grezzi, frequenza_scheda, frequenza);
    // Un picco praticamente nullo non e' «hai parlato piano»: e' un
    // microfono che non sente. Dirlo qui evita di mandare a trascrivere del
    // silenzio e di ricevere indietro parole inventate.
    if picco_totale < PAVIMENTO / 4.0 {
        return Err(MicrofonoMuto { microfono: nome_usato, picco: picco_totale }.into());
    }
    let mut campioni = campioni;
    let guadagno = normalizza(&mut campioni, 0.75);
    Ok(Ascolto {
        campioni,
        frequenza,
        picco: picco_totale,
        fermato_dal_silenzio,
        microfono: nome_usato,
        ha_parlato,
        guadagno,
    })
}

/// Porta il parlato a un livello utile, se c'e' parlato.
///
/// I microfoni consegnano volumi diversissimi: sulle cuffie di questa
/// macchina una frase normale arriva a 0.008, un trentesimo di quello che
/// darebbe un microfono da tavolo. Whisper su un sussurro non sente male:
/// **inventa** — davanti al quasi-silenzio ha restituito «[Musica]».
/// Normalizzare e' piu' onesto che chiedere all'utente di andare a cercare
/// il cursore del volume nelle impostazioni di Windows.
///
/// La soglia sotto cui non si tocca niente e' il punto: amplificare il
/// silenzio significa amplificare il rumore, e dare in pasto rumore forte a
/// un modello e' il modo migliore per farsi raccontare cose mai dette.
pub fn normalizza(campioni: &mut [f32], obiettivo: f32) -> f32 {
    let picco = campioni.iter().fold(0.0f32, |m, c| m.max(c.abs()));
    if picco < PAVIMENTO / 2.0 || picco >= obiettivo {
        return 1.0;
    }
    // Tetto al guadagno: oltre, si sta solo tirando su il fruscio.
    let guadagno = (obiettivo / picco).min(40.0);
    for c in campioni.iter_mut() {
        *c = (*c * guadagno).clamp(-1.0, 1.0);
    }
    guadagno
}

/// Campioni mono in un WAV a 16 bit: e' cio' che ogni motore di trascrizione
/// sa aprire, e non vale una dipendenza in piu'.
pub fn in_wav(campioni: &[f32], frequenza: u32) -> Vec<u8> {
    let byte_dati = (campioni.len() * 2) as u32;
    let mut f = Vec::with_capacity(44 + byte_dati as usize);
    f.extend_from_slice(b"RIFF");
    f.extend_from_slice(&(36 + byte_dati).to_le_bytes());
    f.extend_from_slice(b"WAVEfmt ");
    f.extend_from_slice(&16u32.to_le_bytes());
    f.extend_from_slice(&1u16.to_le_bytes());
    f.extend_from_slice(&1u16.to_le_bytes());
    f.extend_from_slice(&frequenza.to_le_bytes());
    f.extend_from_slice(&(frequenza * 2).to_le_bytes());
    f.extend_from_slice(&2u16.to_le_bytes());
    f.extend_from_slice(&16u16.to_le_bytes());
    f.extend_from_slice(b"data");
    f.extend_from_slice(&byte_dati.to_le_bytes());
    for c in campioni {
        f.extend_from_slice(&((c.clamp(-1.0, 1.0) * 32767.0) as i16).to_le_bytes());
    }
    f
}
