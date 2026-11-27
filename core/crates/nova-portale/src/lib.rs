//! Il portale (D448): un altro dispositivo che parla col NOVA del PC.
//!
//! Deciso con Gio il 27 novembre: un tablet che e' «solo NOVA», una specie
//! di Alexa. Sul tablet c'e' l'interfaccia, il microfono e l'altoparlante; il
//! lavoro lo fa il PC. Fino a qui NOVA parlava solo con se' stessa, su una
//! named pipe: non aveva nessuna porta di rete. Il portale e' quella porta, e
//! siccome NOVA ha le mani sul PC, chi entra di li' lo comanda. Per questo:
//!
//! - **e' spento di serie**: si accende dal pannello (`portale.acceso`);
//! - **il canale e' cifrato** (TLS 1.3) con un certificato fatto in casa, che
//!   nessuna autorita' firma: il dispositivo lo riconosce dall'**impronta**
//!   che ha letto nel codice di abbinamento, e non parla con nessun altro;
//! - **si entra solo abbinati**: il PC mostra un codice che vale una volta e
//!   per pochi minuti; il dispositivo lo presenta e riceve un gettone suo. Del
//!   gettone il PC tiene solo l'impronta, e un dispositivo si dimentica dal
//!   pannello: da quel momento non entra piu', e se era collegato esce.
//!
//! Dopo il saluto il canale porta le stesse righe JSON della pipe: il demone
//! serve il dispositivo come serve la finestra del PC.
//!
//! Qui stanno le due meta' del canale, senza demone: [`identita`] (il
//! certificato), [`abbinamento`] (codici, gettoni, dispositivi), [`invito`]
//! (il codice da inquadrare), [`saluto`] (la prima riga), [`tls`].

pub mod abbinamento;
pub mod identita;
pub mod invito;
pub mod saluto;
pub mod tls;

/// La porta su cui il portale ascolta, se il pannello non ne dice un'altra.
pub const PORTA_DI_SERIE: u16 = 47_810;
