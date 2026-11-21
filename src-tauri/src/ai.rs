//! Recherche intelligente : Gemini (gratuit avec une clé Google AI Studio) regarde chaque motif
//! une seule fois et note ce qu'il représente. Ensuite, chercher « ange » trouve les anges,
//! même si le fichier s'appelle « D1234.pes ». La recherche elle-même reste sur l'ordinateur.

use base64::Engine;
use serde::Serialize;
use serde_json::{json, Value};
use std::time::Duration;

/// Modèles essayés dans l'ordre (les « lite » ont le plus de quota gratuit).
pub const MODELS: [&str; 4] = ["gemini-flash-lite-latest", "gemini-2.5-flash-lite", "gemini-flash-latest", "gemini-2.5-flash"];
/// Nombre de motifs envoyés ensemble.
pub const BATCH: usize = 8;

#[derive(Debug)]
pub enum AiError {
    /// Clé refusée par Google.
    BadKey(String),
    /// Trop de demandes : attendre avant de recommencer.
    RateLimited,
    /// Ce modèle n'existe pas (ou plus) : essayer le suivant.
    ModelNotFound,
    Other(String),
}

pub struct Described {
    pub id: String,
    pub description: String,
    pub keywords: Vec<String>,
}

#[derive(Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct AiStatus {
    pub has_key: bool,
    pub running: bool,
    pub analysed: usize,
    pub total: usize,
    pub error: Option<String>,
}

const PROMPT: &str = "Tu aides une grand-mère à retrouver ses motifs de broderie machine. \
Voici des images de motifs, numérotées dans l'ordre. Pour chacune, décris ce que le motif représente. \
Réponds uniquement en JSON : un tableau d'objets {\"n\": numéro de l'image, \"description\": une courte phrase en français \
(12 mots au plus), \"motscles\": de 6 à 15 mots-clés en français, en minuscules, au singulier}. \
Dans les mots-clés, mets : ce qui est représenté (ex. ange, lapin, rose, voiture), des synonymes simples \
(ex. ange, angelot, chérubin), la catégorie (animal, fleur, personnage, lettre, chiffre, texte, cadre, bordure…), \
les fêtes ou occasions si c'est évident (noël, pâques, naissance, mariage, anniversaire, halloween…), \
et les couleurs principales. Si c'est une lettre, un chiffre ou un mot, écris-le. N'invente rien.";

fn post(model: &str, key: &str, body: &Value) -> Result<Value, AiError> {
    let url = format!("https://generativelanguage.googleapis.com/v1beta/models/{model}:generateContent");
    let agent = ureq::AgentBuilder::new().timeout(Duration::from_secs(120)).try_proxy_from_env(true).build();
    match agent.post(&url).set("x-goog-api-key", key).send_json(body) {
        Ok(r) => r.into_json::<Value>().map_err(|e| AiError::Other(e.to_string())),
        Err(ureq::Error::Status(code, r)) => {
            let text = r.into_string().unwrap_or_default();
            let msg = serde_json::from_str::<Value>(&text)
                .ok()
                .and_then(|v| v["error"]["message"].as_str().map(str::to_string))
                .unwrap_or(text);
            Err(match code {
                429 => AiError::RateLimited,
                404 => AiError::ModelNotFound,
                400 if msg.to_lowercase().contains("api key") => AiError::BadKey(msg),
                401 | 403 => AiError::BadKey(msg),
                _ => AiError::Other(format!("erreur {code} : {msg}")),
            })
        }
        Err(e) => Err(AiError::Other(format!("pas de connexion à internet ? ({e})"))),
    }
}

fn answer_text(v: &Value) -> String {
    v["candidates"][0]["content"]["parts"]
        .as_array()
        .map(|parts| parts.iter().filter_map(|p| p["text"].as_str()).collect::<Vec<_>>().join(""))
        .unwrap_or_default()
}

/// Vérifie la clé avec une toute petite demande ; renvoie le modèle qui répond.
pub fn check_key(key: &str) -> Result<String, AiError> {
    let body = json!({ "contents": [{ "parts": [{ "text": "Réponds seulement : OK" }] }] });
    let mut last = AiError::ModelNotFound;
    for m in MODELS {
        match post(m, key, &body) {
            Ok(_) => return Ok(m.to_string()),
            Err(AiError::ModelNotFound) => continue,
            Err(e) => last = e,
        }
        break;
    }
    Err(last)
}

/// Demande à Gemini de décrire plusieurs motifs (images PNG) d'un coup.
pub fn describe(key: &str, model: &str, images: &[(String, Vec<u8>)]) -> Result<Vec<Described>, AiError> {
    let b64 = base64::engine::general_purpose::STANDARD;
    let mut parts = vec![json!({ "text": PROMPT })];
    for (i, (_, png)) in images.iter().enumerate() {
        parts.push(json!({ "text": format!("Image {} :", i + 1) }));
        parts.push(json!({ "inline_data": { "mime_type": "image/png", "data": b64.encode(png) } }));
    }
    let body = json!({
        "contents": [{ "role": "user", "parts": parts }],
        "generationConfig": { "response_mime_type": "application/json", "temperature": 0.2 }
    });
    let v = post(model, key, &body)?;
    let text = answer_text(&v);
    let start = text.find('[').unwrap_or(0);
    let end = text.rfind(']').map(|e| e + 1).unwrap_or(text.len());
    let arr: Vec<Value> = serde_json::from_str(&text[start..end]).map_err(|_| AiError::Other("réponse de l'IA illisible".into()))?;
    let mut out = Vec::new();
    for (k, item) in arr.iter().enumerate() {
        let n = item["n"].as_u64().map(|n| n as usize).filter(|n| (1..=images.len()).contains(n)).unwrap_or(k + 1);
        let Some((id, _)) = images.get(n - 1) else { continue };
        let keywords = item["motscles"]
            .as_array()
            .or_else(|| item["mots_cles"].as_array())
            .map(|a| a.iter().filter_map(|x| x.as_str().map(str::to_string)).collect())
            .unwrap_or_default();
        out.push(Described {
            id: id.clone(),
            description: item["description"].as_str().unwrap_or_default().to_string(),
            keywords,
        });
    }
    Ok(out)
}
