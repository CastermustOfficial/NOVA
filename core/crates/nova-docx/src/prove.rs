use super::*;

const CORPO: &str = r#"<w:body>
<w:p><w:pPr><w:pStyle w:val="Heading1"/></w:pPr><w:r><w:t>Titolo</w:t></w:r></w:p>
<w:p/>
<w:p><w:r><w:rPr><w:b/></w:rPr><w:t xml:space="preserve">Un </w:t></w:r><w:r><w:t>corpo diviso</w:t></w:r></w:p>
<w:tbl><w:tr><w:tc><w:p><w:r><w:t>Voce</w:t></w:r></w:p></w:tc><w:tc><w:p><w:r><w:t>Importo</w:t></w:r></w:p></w:tc></w:tr>
<w:tr><w:tc><w:p><w:r><w:t>Consulenza</w:t></w:r></w:p></w:tc><w:tc><w:p><w:r><w:t>1000</w:t></w:r></w:p></w:tc></w:tr></w:tbl>
<w:p><w:r><w:t>Dopo la tabella</w:t></w:r></w:p>
</w:body>"#;

#[test]
fn un_tag_non_e_un_altro_che_comincia_uguale() {
    // `<w:pPr>` sta dentro ogni paragrafo: confonderlo con `<w:p>` vuol
    // dire trovare il doppio dei paragrafi, e meta' non sono paragrafi.
    assert!(comincia_qui("<w:p>", 0, "w:p"));
    assert!(comincia_qui("<w:p/>", 0, "w:p"));
    assert!(comincia_qui("<w:p w:rsid=\"x\">", 0, "w:p"));
    assert!(!comincia_qui("<w:pPr>", 0, "w:p"));
    assert!(!comincia_qui("<w:pStyle/>", 0, "w:p"));
    assert!(!comincia_qui("</w:p>", 0, "w:p"));
}

#[test]
fn i_paragrafi_del_corpo_non_sono_quelli_delle_celle() {
    // `python-docx` fa la stessa distinzione, e i blocchi `p0`, `p1`
    // dell'harness contano su quella: se cambiasse, `p3` indicherebbe un
    // altro paragrafo e la modifica finirebbe altrove.
    let p = paragrafi(CORPO);
    assert_eq!(p.len(), 4, "titolo, vuoto, corpo, dopo la tabella");
    assert_eq!(testo_di(p[0].dentro(CORPO)), "Titolo");
    assert_eq!(
        testo_di(p[1].dentro(CORPO)),
        "",
        "il paragrafo vuoto c'e' lo stesso"
    );
    assert_eq!(testo_di(p[2].dentro(CORPO)), "Un corpo diviso");
    assert_eq!(testo_di(p[3].dentro(CORPO)), "Dopo la tabella");
}

#[test]
fn e_le_tabelle_si_trovano_riga_per_riga() {
    let t = tabelle(CORPO);
    assert_eq!(t.len(), 1);
    let dentro = t[0].dentro(CORPO);
    let righe = elementi(dentro, "w:tr");
    assert_eq!(righe.len(), 2);
    let celle = elementi(righe[1].dentro(dentro), "w:tc");
    assert_eq!(celle.len(), 2);
    assert_eq!(
        testo_di(celle[0].dentro(righe[1].dentro(dentro))),
        "Consulenza"
    );
    assert_eq!(testo_di(celle[1].dentro(righe[1].dentro(dentro))), "1000");
}

#[test]
fn lo_stile_di_un_paragrafo_si_legge() {
    let p = paragrafi(CORPO);
    assert_eq!(stile_di(p[0].dentro(CORPO)), "Heading1");
    assert_eq!(stile_di(p[2].dentro(CORPO)), "", "senza stile dichiarato");
}

#[test]
fn il_testo_nuovo_va_tutto_nella_prima_porzione() {
    // Le porzioni esistono perche' hanno formattazioni diverse: distribuire
    // un testo nuovo fra porzioni vecchie vorrebbe dire indovinare quale
    // pezzo va in grassetto. Nella prima e' l'unica scelta che non inventa.
    let p = paragrafi(CORPO);
    let vecchio = p[2].dentro(CORPO);
    let nuovo = riscrivi_testo(vecchio, "Tutto rifatto");
    assert_eq!(testo_di(&nuovo), "Tutto rifatto");
    // Il grassetto della prima porzione e' ancora li'.
    assert!(nuovo.contains("<w:b/>"), "{nuovo}");
    // E le porzioni sono ancora due: non si e' ricostruito niente.
    assert_eq!(elementi(&nuovo, "w:r").len(), 2, "{nuovo}");
    assert_eq!(elementi(&nuovo, "w:t").len(), 2, "{nuovo}");
}

#[test]
fn e_gli_spazi_in_testa_non_si_perdono() {
    // Senza `xml:space="preserve"` Word li toglie, e una riga di codice
    // indentata perde il rientro.
    let p = paragrafi(CORPO);
    let nuovo = riscrivi_testo(p[0].dentro(CORPO), "    rientrato");
    assert!(nuovo.contains("xml:space=\"preserve\""), "{nuovo}");
    assert_eq!(testo_di(&nuovo), "    rientrato");
}

#[test]
fn un_paragrafo_senza_porzioni_resta_com_e() {
    // Un `<w:p/>` non ha dove mettere il testo. Inventargli una porzione
    // vorrebbe dire creare struttura, cioe' la cosa che questo file non fa.
    let p = paragrafi(CORPO);
    let vuoto = p[1].dentro(CORPO);
    assert_eq!(riscrivi_testo(vuoto, "qualcosa"), vuoto);
}

#[test]
fn i_caratteri_che_lxml_non_sopporta_si_proteggono() {
    let p = paragrafi(CORPO);
    let nuovo = riscrivi_testo(p[0].dentro(CORPO), "a < b & c > d \"virgolette\"");
    assert!(nuovo.contains("a &lt; b &amp; c &gt; d"), "{nuovo}");
    // E tornano indietro com'erano.
    assert_eq!(testo_di(&nuovo), "a < b & c > d \"virgolette\"");
}

#[test]
fn e_la_e_commerciale_si_scioglie_per_ultima() {
    // `&amp;lt;` e' il modo di scrivere la stringa «&lt;»: sciogliendo
    // `&amp;` per primo diventerebbe un `<`.
    assert_eq!(da_xml("&amp;lt;"), "&lt;");
    assert_eq!(da_xml("&lt;tag&gt;"), "<tag>");
    assert_eq!(in_xml("&lt;"), "&amp;lt;");
    assert_eq!(da_xml(&in_xml("a & b < c")), "a & b < c");
}

#[test]
fn sostituire_e_togliere_lasciano_il_resto_intatto() {
    let p = paragrafi(CORPO);
    let nuovo = riscrivi_testo(p[0].dentro(CORPO), "ALTRO");
    let tutto = sostituisci(CORPO, p[0], &nuovo);
    assert!(tutto.contains("ALTRO"), "{tutto}");
    assert!(tutto.contains("Dopo la tabella"), "il resto c'e' ancora");
    assert_eq!(paragrafi(&tutto).len(), 4);
    let senza = togli(CORPO, p[0]);
    assert_eq!(paragrafi(&senza).len(), 3);
    assert!(!senza.contains("Titolo"));
}

#[test]
fn un_elemento_dentro_un_altro_con_lo_stesso_nome_si_conta_una_volta() {
    // Le tabelle si annidano. Contando le aperture e le chiusure, quella
    // dentro non diventa una tabella del corpo.
    let x = "<w:tbl>fuori<w:tbl>dentro</w:tbl>ancora fuori</w:tbl><w:tbl>sola</w:tbl>";
    let t = elementi(x, "w:tbl");
    assert_eq!(t.len(), 2);
    // La prima e' quella **esterna**, tutta intera: se il conteggio fosse
    // sbagliato si troverebbe quella dentro, e sarebbe un elemento che nel
    // corpo non esiste.
    assert_eq!(
        t[0].dentro(x),
        "<w:tbl>fuori<w:tbl>dentro</w:tbl>ancora fuori</w:tbl>"
    );
    assert_eq!(t[1].dentro(x), "<w:tbl>sola</w:tbl>");
}

#[test]
fn una_parte_che_non_ce_non_si_scrive_e_il_file_resta_com_e() {
    // Dire «fatto» a una parte che non si e' toccata vuol dire che chi
    // chiama crede di aver modificato il documento, e non se ne accorge
    // finche' non lo riapre.
    let d = std::env::temp_dir().join(format!("nova-docx-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    let f = d.join("finto.docx");
    {
        let uscita = std::fs::File::create(&f).unwrap();
        let mut z = zip::ZipWriter::new(uscita);
        let o = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated);
        z.start_file("word/document.xml", o).unwrap();
        z.write_all(b"<w:body><w:p><w:r><w:t>ciao</w:t></w:r></w:p></w:body>")
            .unwrap();
        z.start_file("docProps/app.xml", o).unwrap();
        z.write_all(b"<Properties/>").unwrap();
        z.finish().unwrap();
    }
    let prima = std::fs::metadata(&f).unwrap().len();
    let e = riscrivi_parte(&f, "word/inventata.xml", "<x/>").unwrap_err();
    assert!(e.contains("inventata"), "{e}");
    assert_eq!(
        std::fs::metadata(&f).unwrap().len(),
        prima,
        "il file non si tocca"
    );
    assert!(
        !d.join("finto.docx.parte").exists(),
        "e non resta un .parte"
    );
    // E su una parte che c'e' funziona, e le altre restano.
    let quante = riscrivi_parte(
        &f,
        DOCUMENTO,
        "<w:body><w:p><w:r><w:t>altro</w:t></w:r></w:p></w:body>",
    )
    .unwrap();
    assert_eq!(quante, 2, "tutte e due le parti sono state rimesse");
    assert_eq!(testo_di(&leggi_parte(&f, DOCUMENTO).unwrap()), "altro");
    assert_eq!(
        leggi_parte(&f, "docProps/app.xml").unwrap(),
        "<Properties/>"
    );
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn un_xml_troncato_non_fa_esplodere_niente() {
    // Un documento rotto si legge male, non fa cadere NOVA.
    assert!(elementi("<w:p>senza chiusura", "w:p").is_empty());
    assert!(elementi("", "w:p").is_empty());
    assert_eq!(testo_di("<w:t>"), "");
    assert_eq!(stile_di("<w:pStyle w:val="), "");
}
