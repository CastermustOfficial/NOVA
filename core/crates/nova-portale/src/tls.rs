//! Il canale cifrato: TLS 1.3, col certificato del PC riconosciuto
//! dall'impronta.
//!
//! Nessuna autorita' firma il certificato del PC, e il dispositivo non ne
//! chiede: accetta **solo** il certificato che ha l'impronta letta
//! nell'invito ([`connettore`]). Chi si mette in mezzo con un altro
//! certificato non passa, e senza la chiave del PC non puo' usare il suo. La
//! firma della stretta di mano si controlla comunque: l'impronta dice quale
//! certificato, la firma dice che chi risponde ne ha la chiave.

use crate::abbinamento::uguali;
use crate::identita::{impronta_di, Identita, NOME_NEL_CERTIFICATO};
use anyhow::{anyhow, Context, Result};
use std::sync::Arc;
use std::time::Duration;
use tokio::net::TcpStream;
use tokio_rustls::rustls::client::danger::{
    HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier,
};
use tokio_rustls::rustls::crypto::{
    verify_tls12_signature, verify_tls13_signature, CryptoProvider,
};
use tokio_rustls::rustls::pki_types::{
    CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer, ServerName, UnixTime,
};
use tokio_rustls::rustls::{self, DigitallySignedStruct, SignatureScheme};
use tokio_rustls::{TlsAcceptor, TlsConnector};

/// Quanto si aspetta che il PC risponda a un indirizzo, prima di provare il
/// prossimo.
pub const ATTESA: Duration = Duration::from_secs(5);

/// Dopo quanto silenzio si chiede all'altro capo se c'e' ancora...
pub const SILENZIO_PRIMA_DI_CHIEDERE: Duration = Duration::from_secs(20);

/// ...e ogni quanto lo si richiede, finche' non risponde o lo si da' per
/// perso.
pub const RIDOMANDA_OGNI: Duration = Duration::from_secs(5);

/// Fa in modo che un capo che sparisce senza salutare si noti (D452).
///
/// Un canale su cui nessuno scrive non sa che l'altro capo non c'e' piu':
/// un tablet a cui si toglie la corrente, o che esce dal Wi-Fi, non manda
/// nessun saluto. Il 28 novembre il tablet e' stato ripristinato mentre era
/// collegato, e un'ora dopo il pannello del PC lo dava ancora «collegato».
/// Con questo il sistema, dopo [`SILENZIO_PRIMA_DI_CHIEDERE`], chiede all'altro capo se c'e'
/// ancora, ogni [`RIDOMANDA_OGNI`]: se non risponde il canale si chiude con un errore,
/// e chi lo serviva smette. Ci vuole circa un minuto.
pub fn tieni_d_occhio(tcp: &TcpStream) -> std::io::Result<()> {
    let come = socket2::TcpKeepalive::new()
        .with_time(SILENZIO_PRIMA_DI_CHIEDERE)
        .with_interval(RIDOMANDA_OGNI);
    socket2::SockRef::from(tcp).set_tcp_keepalive(&come)
}

/// Il canale dalla parte del dispositivo.
pub type Canale = tokio_rustls::client::TlsStream<TcpStream>;

fn fornitore() -> Arc<CryptoProvider> {
    Arc::new(rustls::crypto::ring::default_provider())
}

/// La meta' del PC: accetta le connessioni col suo certificato.
pub fn accettatore(identita: &Identita) -> Result<TlsAcceptor> {
    let config = rustls::ServerConfig::builder_with_provider(fornitore())
        .with_protocol_versions(&[&rustls::version::TLS13])
        .context("TLS 1.3 non disponibile")?
        .with_no_client_auth()
        .with_single_cert(
            vec![CertificateDer::from(identita.certificato.clone())],
            PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(identita.chiave.clone())),
        )
        .context("il certificato del portale non e' buono")?;
    Ok(TlsAcceptor::from(Arc::new(config)))
}

/// Accetta un certificato solo: quello con questa impronta.
#[derive(Debug)]
struct SoloQuesto {
    impronta: String,
    fornitore: Arc<CryptoProvider>,
}

impl ServerCertVerifier for SoloQuesto {
    fn verify_server_cert(
        &self,
        certificato: &CertificateDer<'_>,
        _intermedi: &[CertificateDer<'_>],
        _nome: &ServerName<'_>,
        _ocsp: &[u8],
        _ora: UnixTime,
    ) -> Result<ServerCertVerified, rustls::Error> {
        if uguali(&impronta_di(certificato.as_ref()), &self.impronta) {
            Ok(ServerCertVerified::assertion())
        } else {
            Err(rustls::Error::General(
                "chi risponde non e' il PC dell'invito: l'impronta e' un'altra".into(),
            ))
        }
    }

    fn verify_tls12_signature(
        &self,
        messaggio: &[u8],
        certificato: &CertificateDer<'_>,
        firma: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        verify_tls12_signature(
            messaggio,
            certificato,
            firma,
            &self.fornitore.signature_verification_algorithms,
        )
    }

    fn verify_tls13_signature(
        &self,
        messaggio: &[u8],
        certificato: &CertificateDer<'_>,
        firma: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        verify_tls13_signature(
            messaggio,
            certificato,
            firma,
            &self.fornitore.signature_verification_algorithms,
        )
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        self.fornitore
            .signature_verification_algorithms
            .supported_schemes()
    }
}

/// La meta' del dispositivo: parla solo col PC che ha questa impronta.
pub fn connettore(impronta: &str) -> Result<TlsConnector> {
    let fornitore = fornitore();
    let config = rustls::ClientConfig::builder_with_provider(fornitore.clone())
        .with_protocol_versions(&[&rustls::version::TLS13])
        .context("TLS 1.3 non disponibile")?
        .dangerous()
        .with_custom_certificate_verifier(Arc::new(SoloQuesto {
            impronta: impronta.to_string(),
            fornitore,
        }))
        .with_no_client_auth();
    Ok(TlsConnector::from(Arc::new(config)))
}

/// Si collega al PC a questo indirizzo, e lo riconosce dall'impronta.
pub async fn collega(indirizzo: &str, porta: u16, impronta: &str) -> Result<Canale> {
    let tcp = tokio::time::timeout(ATTESA, TcpStream::connect((indirizzo, porta)))
        .await
        .map_err(|_| anyhow!("{indirizzo}:{porta} non risponde"))?
        .with_context(|| format!("{indirizzo}:{porta} non si apre"))?;
    // Righe corte, e la voce a pezzi piccoli: non si aspetta di riempire un
    // pacchetto.
    let _ = tcp.set_nodelay(true);
    // Se il PC sparisce senza salutare, il dispositivo se ne accorge.
    let _ = tieni_d_occhio(&tcp);
    let nome = ServerName::try_from(NOME_NEL_CERTIFICATO).map_err(|e| anyhow!("{e}"))?;
    tokio::time::timeout(ATTESA, connettore(impronta)?.connect(nome, tcp))
        .await
        .map_err(|_| anyhow!("{indirizzo}:{porta} non finisce la stretta di mano"))?
        .with_context(|| format!("{indirizzo}:{porta} non e' il PC dell'invito"))
}

#[cfg(test)]
mod prove {
    use super::*;
    use tokio::net::TcpListener;

    /// D452: tutti e due i capi tengono d'occhio il canale.
    #[tokio::test]
    async fn un_capo_che_sparisce_senza_salutare_si_nota() {
        let ascolto = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let porta = ascolto.local_addr().unwrap().port();
        let identita = Identita::nuova().unwrap();
        let accettatore = accettatore(&identita).unwrap();
        let pc = tokio::spawn(async move {
            let (tcp, _) = ascolto.accept().await.unwrap();
            // Appena accettato il sistema non lo tiene d'occhio da solo.
            let prima = socket2::SockRef::from(&tcp).keepalive().unwrap();
            tieni_d_occhio(&tcp).unwrap();
            let dopo = socket2::SockRef::from(&tcp).keepalive().unwrap();
            let _canale = accettatore.accept(tcp).await.unwrap();
            (prima, dopo)
        });
        // Dalla parte del dispositivo lo fa `collega`.
        let canale = collega("127.0.0.1", porta, &identita.impronta())
            .await
            .unwrap();
        assert!(socket2::SockRef::from(canale.get_ref().0)
            .keepalive()
            .unwrap());
        assert_eq!(pc.await.unwrap(), (false, true));
        // Meno di un minuto e mezzo anche dove le domande sono dieci.
        assert!(SILENZIO_PRIMA_DI_CHIEDERE + RIDOMANDA_OGNI * 10 <= Duration::from_secs(90));
    }
}
