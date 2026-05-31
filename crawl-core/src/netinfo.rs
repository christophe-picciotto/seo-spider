//! Informations reseau de la machine : IP publique sortante + reverse DNS.
//!
//! On interroge le service public ipinfo.io/json qui renvoie en une requete
//! l'IP vue de l'exterieur ET son `hostname` (le reverse DNS / PTR), evitant
//! d'avoir a embarquer un resolveur DNS dans l'application.

use serde::{Deserialize, Serialize};

/// Resultat expose au frontend (serialise tel quel : `ip`, `reverse_dns`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkInfo {
    /// IP publique sortante (telle que vue par les sites crawles).
    pub ip: String,
    /// Reverse DNS (PTR) de cette IP, si disponible.
    pub reverse_dns: Option<String>,
}

/// Recupere l'IP publique sortante et son reverse DNS via ipinfo.io.
pub async fn fetch() -> anyhow::Result<NetworkInfo> {
    /// Sous-ensemble de la reponse JSON d'ipinfo.io qui nous interesse.
    #[derive(Deserialize)]
    struct IpInfoResponse {
        ip: String,
        /// Reverse DNS de l'IP (absent si aucun PTR).
        hostname: Option<String>,
    }

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(8))
        .build()?;

    let resp = client
        .get("https://ipinfo.io/json")
        .header("Accept", "application/json")
        .send()
        .await?
        .error_for_status()?;

    let body = resp.text().await?;
    let info: IpInfoResponse = serde_json::from_str(&body)?;

    Ok(NetworkInfo {
        ip: info.ip,
        reverse_dns: info.hostname,
    })
}
