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
    let nome = ServerName::try_from(NOME_NEL_CERTIFICATO).map_err(|e| anyhow!("{e}"))?;
    tokio::time::timeout(ATTESA, connettore(impronta)?.connect(nome, tcp))
        .await
        .map_err(|_| anyhow!("{indirizzo}:{porta} non finisce la stretta di mano"))?
        .with_context(|| format!("{indirizzo}:{porta} non e' il PC dell'invito"))
}
