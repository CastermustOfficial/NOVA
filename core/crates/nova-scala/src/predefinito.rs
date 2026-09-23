//! La scala di fabbrica: `routing_predefinito()` di `nova/routing.py`.
//!
//! **Generato da `attrezzi/_estrai_routing.py`. Non si scrive a mano.**
//! Python la mette sotto a quello che l'utente ha scritto in
//! `brains.routing`, al primo livello; [`crate::routing_effettivo`] fa lo
//! stesso.

/// Il JSON di `routing_predefinito()`.
pub const ROUTING_PREDEFINITO: &str = r#"{
 "abilitato": true,
 "orchestratore": "locale",
 "scala": [
  "locale",
  "standard",
  "difficile",
  "alternativo"
 ],
 "tiers": {
  "locale": {
   "brain": "locale",
   "descrizione": "Il modello sul PC. Gratis, privato, orchestra e fa i compiti semplici.",
   "locale": true,
   "a_pagamento": false
  },
  "standard": {
   "brain": "claude",
   "model": "claude-sonnet-5",
   "descrizione": "Il cavallo da lavoro: codice, analisi, compiti articolati."
  },
  "difficile": {
   "brain": "claude",
   "model": "claude-opus-5",
   "descrizione": "Quando il compito lo merita davvero. Pesa sulla quota. In alternativa: claude-fable-5."
  },
  "alternativo": {
   "brain": "gemini",
   "model": "gemini-2.5-pro",
   "descrizione": "Seconda opinione, o quando serve un altro punto di vista."
  }
 },
 "escalation_automatica": true,
 "fallimenti_prima_di_salire": 2,
 "passi_prima_di_salire": 4,
 "salite_massime": 2,
 "solo_locale": false,
 "tetto_usd_sessione": 5.0,
 "ripiego_su_limite": true,
 "costo_stimato_delega": 0.1,
 "categorie_che_salgono": {
  "review_multifile": {
   "attiva": true,
   "gradino_minimo": "difficile",
   "descrizione": "review di codice su piu' file",
   "min_file": 2,
   "parole": [
    "review",
    "code review",
    "rivedi",
    "revision*",
    "audit",
    "difett*",
    "bug",
    "vulnerabilit*",
    "analizza il codice",
    "cosa non va",
    "controlla il codice"
   ]
  },
  "perdita_dati": {
   "attiva": true,
   "gradino_minimo": "difficile",
   "descrizione": "rischio di perdita o corruzione di dati",
   "min_file": 0,
   "parole": [
    "perdita di dati",
    "perdere dati",
    "sovrascriv*",
    "corruzione",
    "corrompe",
    "race condition",
    "concorrenza",
    "migrazione dei dati",
    "cancellazione",
    "irreversibil*"
   ]
  },
  "architettura": {
   "attiva": true,
   "gradino_minimo": "difficile",
   "descrizione": "decisione di architettura",
   "min_file": 0,
   "parole": [
    "architettura",
    "architettural*",
    "progetta*",
    "come strutturare",
    "refactor*",
    "trade-off",
    "quale approccio",
    "design"
   ]
  }
 }
}"#;
