//! La data di un file, come la legge una persona.
//!
//! Il fuso arriva da fuori. E' la stessa scelta di `oggi` nel vault e in
//! `nova-pianificazione`: una funzione che si legge l'orologio o il fuso da
//! sola non si prova due volte con lo stesso risultato, e questa in
//! particolare cambierebbe risposta a marzo e a ottobre senza che nessuna
//! prova se ne accorga.

/// `2026-09-05 14:34`, che e' il formato che il modello si trova davanti in
/// ogni elenco di cartella.
pub fn locale(secondi: u64, fuso_secondi: i64) -> String {
    let d = nova_calendario::da_istante(secondi as i64, fuso_secondi);
    format!(
        "{:04}-{:02}-{:02} {:02}:{:02}",
        d.anno, d.mese, d.giorno, d.ora, d.minuto
    )
}

#[cfg(test)]
mod prove {
    use super::*;

    #[test]
    fn la_data_si_scrive_come_la_legge_una_persona() {
        assert_eq!(locale(1_788_611_696, 2 * 3600), "2026-09-05 14:34");
        assert_eq!(locale(0, 0), "1970-01-01 00:00");
    }

    #[test]
    fn il_fuso_sposta_il_giorno_quando_deve() {
        // Un file salvato alle 23:30 a Roma e' il giorno **dopo** in UTC: se
        // il fuso non contasse, l'elenco direbbe una data sbagliata di un
        // giorno per due ore su ventiquattro.
        // I valori sono presi dal Python, non inventati: la prima volta li
        // avevo scritti a occhio ed erano sbagliati di un'ora e venti. Un
        // valore atteso che si inventa non prova niente, prova solo che si
        // sapeva gia' la risposta.
        assert_eq!(locale(1_788_651_000, 0), "2026-09-05 23:30");
        assert_eq!(locale(1_788_651_000, 2 * 3600), "2026-09-06 01:30");
    }
}
