//! WhatsApp Cloud API de Meta: firma del webhook, lectura de mensajes
//! entrantes y envío de respuestas.

use std::sync::OnceLock;

use hmac::{Hmac, Mac};
use serde_json::{Value, json};
use sha2::Sha256;

use super::engine::{Input, MAX_BUTTON_TITLE, MAX_BUTTONS, Reply};

/// Credenciales de la app de Meta. Sin `WHATSAPP_APP_SECRET` el webhook
/// responde 503: nunca se aceptan mensajes sin verificar la firma.
#[derive(Debug, Clone)]
pub struct WaEnv {
    pub verify_token: String,
    pub app_secret: String,
    /// Token de acceso del sistema para enviar mensajes. Sin él, el bot
    /// procesa y guarda, pero no responde (útil en pruebas).
    pub access_token: Option<String>,
    pub graph_version: String,
}

pub fn env() -> Option<&'static WaEnv> {
    static ENV: OnceLock<Option<WaEnv>> = OnceLock::new();
    ENV.get_or_init(|| {
        let verify_token = std::env::var("WHATSAPP_VERIFY_TOKEN")
            .ok()
            .filter(|s| !s.is_empty())?;
        let app_secret = std::env::var("WHATSAPP_APP_SECRET")
            .ok()
            .filter(|s| !s.is_empty())?;
        Some(WaEnv {
            verify_token,
            app_secret,
            access_token: std::env::var("WHATSAPP_ACCESS_TOKEN")
                .ok()
                .filter(|s| !s.is_empty()),
            graph_version: std::env::var("WHATSAPP_GRAPH_VERSION")
                .ok()
                .filter(|s| !s.is_empty())
                .unwrap_or_else(|| "v21.0".into()),
        })
    })
    .as_ref()
}

fn hex_decode(s: &str) -> Option<Vec<u8>> {
    if !s.len().is_multiple_of(2) {
        return None;
    }
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(s.get(i..i + 2)?, 16).ok())
        .collect()
}

/// Comprueba `X-Hub-Signature-256: sha256=<hex>` (HMAC-SHA256 del cuerpo con
/// el secreto de la app). La comparación es en tiempo constante.
pub fn verify_signature(app_secret: &str, body: &[u8], header: Option<&str>) -> bool {
    let Some(sig) = header
        .and_then(|h| h.strip_prefix("sha256="))
        .and_then(hex_decode)
    else {
        return false;
    };
    let Ok(mut mac) = Hmac::<Sha256>::new_from_slice(app_secret.as_bytes()) else {
        return false;
    };
    mac.update(body);
    mac.verify_slice(&sig).is_ok()
}

/// Un mensaje entrante ya interpretado.
#[derive(Debug, Clone, PartialEq)]
pub struct Inbound {
    pub phone_number_id: String,
    pub from: String,
    pub wa_message_id: String,
    pub contact_name: Option<String>,
    pub input: Input,
    /// Texto para el historial (lo escrito o el título del botón).
    pub body: String,
}

/// Lee los mensajes de un webhook. Ignora estados de entrega y lo que no se
/// pueda leer.
pub fn parse_webhook(v: &Value) -> Vec<Inbound> {
    let mut out = Vec::new();
    let empty = Vec::new();
    for entry in v["entry"].as_array().unwrap_or(&empty) {
        for change in entry["changes"].as_array().unwrap_or(&empty) {
            let value = &change["value"];
            let Some(pnid) = value["metadata"]["phone_number_id"].as_str() else {
                continue;
            };
            let name = value["contacts"][0]["profile"]["name"]
                .as_str()
                .map(str::to_string);
            for m in value["messages"].as_array().unwrap_or(&empty) {
                let (Some(from), Some(id)) = (m["from"].as_str(), m["id"].as_str()) else {
                    continue;
                };
                let (input, body) = match m["type"].as_str() {
                    Some("text") => {
                        let t = m["text"]["body"].as_str().unwrap_or("").to_string();
                        (Input::Text(t.clone()), t)
                    }
                    Some("interactive") => {
                        let r = if m["interactive"]["type"] == "list_reply" {
                            &m["interactive"]["list_reply"]
                        } else {
                            &m["interactive"]["button_reply"]
                        };
                        (
                            Input::Button(r["id"].as_str().unwrap_or("").into()),
                            r["title"].as_str().unwrap_or("").into(),
                        )
                    }
                    Some("button") => {
                        let p = m["button"]["payload"].as_str().unwrap_or("");
                        (
                            Input::Button(p.into()),
                            m["button"]["text"].as_str().unwrap_or(p).into(),
                        )
                    }
                    Some("location") => {
                        let l = &m["location"];
                        let t = match (l["address"].as_str(), l["name"].as_str()) {
                            (Some(a), _) => a.to_string(),
                            (None, Some(n)) => n.to_string(),
                            _ => format!("Ubicación {}, {}", l["latitude"], l["longitude"]),
                        };
                        (Input::Text(t.clone()), t)
                    }
                    _ => (
                        Input::Text(String::new()),
                        format!("[{}]", m["type"].as_str().unwrap_or("mensaje")),
                    ),
                };
                out.push(Inbound {
                    phone_number_id: pnid.into(),
                    from: from.into(),
                    wa_message_id: id.into(),
                    contact_name: name.clone(),
                    input,
                    body,
                });
            }
        }
    }
    out
}

fn clip(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        s.chars().take(max - 1).collect::<String>() + "…"
    }
}

/// Cuerpo JSON de `POST /{phone_number_id}/messages`.
pub fn message_payload(to: &str, reply: &Reply) -> Value {
    match reply {
        Reply::Text(t) => json!({
            "messaging_product": "whatsapp",
            "to": to,
            "type": "text",
            "text": { "body": clip(t, 4096), "preview_url": false }
        }),
        Reply::Buttons { body, buttons } => json!({
            "messaging_product": "whatsapp",
            "to": to,
            "type": "interactive",
            "interactive": {
                "type": "button",
                "body": { "text": clip(body, 1024) },
                "action": { "buttons": buttons.iter().take(MAX_BUTTONS).map(|(id, title)| json!({
                    "type": "reply",
                    "reply": { "id": clip(id, 256), "title": clip(title, MAX_BUTTON_TITLE) }
                })).collect::<Vec<_>>() }
            }
        }),
    }
}

/// Texto plano de una respuesta, para el historial.
pub fn reply_text(r: &Reply) -> String {
    match r {
        Reply::Text(t) => t.clone(),
        Reply::Buttons { body, buttons } => {
            let titles: Vec<&str> = buttons.iter().map(|(_, t)| t.as_str()).collect();
            format!("{body}\n[{}]", titles.join(" · "))
        }
    }
}

fn client() -> &'static reqwest::Client {
    static C: OnceLock<reqwest::Client> = OnceLock::new();
    C.get_or_init(|| {
        reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(8))
            .build()
            .unwrap_or_default()
    })
}

/// Envía las respuestas en orden. Si no hay token configurado no hace nada.
pub async fn send_all(env: &WaEnv, phone_number_id: &str, to: &str, replies: &[Reply]) {
    let Some(token) = env.access_token.as_deref() else {
        tracing::warn!("WHATSAPP_ACCESS_TOKEN no configurado: no se envían respuestas");
        return;
    };
    let url = format!(
        "https://graph.facebook.com/{}/{}/messages",
        env.graph_version, phone_number_id
    );
    for r in replies {
        let res = client()
            .post(&url)
            .bearer_auth(token)
            .json(&message_payload(to, r))
            .send()
            .await;
        match res {
            Ok(resp) if resp.status().is_success() => {}
            Ok(resp) => {
                let status = resp.status();
                let body = resp.text().await.unwrap_or_default();
                tracing::error!(%status, body, "WhatsApp rechazó el mensaje");
                break;
            }
            Err(e) => {
                tracing::error!(error = %e, "no se pudo enviar el mensaje de WhatsApp");
                break;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn signature_is_checked() {
        let body = br#"{"object":"whatsapp_business_account"}"#;
        let mut mac = Hmac::<Sha256>::new_from_slice(b"secreto").unwrap();
        mac.update(body);
        let hex: String = mac
            .finalize()
            .into_bytes()
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect();
        assert!(verify_signature(
            "secreto",
            body,
            Some(&format!("sha256={hex}"))
        ));
        assert!(!verify_signature(
            "otro",
            body,
            Some(&format!("sha256={hex}"))
        ));
        assert!(!verify_signature(
            "secreto",
            b"{}",
            Some(&format!("sha256={hex}"))
        ));
        assert!(!verify_signature("secreto", body, None));
        assert!(!verify_signature("secreto", body, Some("sha256=zz")));
    }

    #[test]
    fn parses_text_button_and_ignores_statuses() {
        let v = json!({"object": "whatsapp_business_account", "entry": [{"changes": [{"value": {
            "metadata": {"phone_number_id": "1234"},
            "contacts": [{"profile": {"name": "Ramiro"}, "wa_id": "573005551234"}],
            "messages": [
                {"from": "573005551234", "id": "wamid.1", "type": "text", "text": {"body": "2 aguilas"}},
                {"from": "573005551234", "id": "wamid.2", "type": "interactive",
                 "interactive": {"type": "button_reply", "button_reply": {"id": "CONFIRM", "title": "Sí, así está"}}},
                {"from": "573005551234", "id": "wamid.3", "type": "audio", "audio": {"id": "x"}}
            ],
            "statuses": [{"id": "wamid.0", "status": "read"}]
        }}]}]});
        let m = parse_webhook(&v);
        assert_eq!(m.len(), 3);
        assert_eq!(m[0].input, Input::Text("2 aguilas".into()));
        assert_eq!(m[0].contact_name.as_deref(), Some("Ramiro"));
        assert_eq!(m[1].input, Input::Button("CONFIRM".into()));
        assert_eq!(m[2].input, Input::Text(String::new()));
        assert!(parse_webhook(&json!({"entry": [{"changes": [{"value": {"metadata": {"phone_number_id": "1"}, "statuses": []}}]}]})).is_empty());
    }

    #[test]
    fn payload_respects_whatsapp_limits() {
        let r = Reply::Buttons {
            body: "x".repeat(2000),
            buttons: vec![
                ("A".into(), "Un título demasiado largo para botón".into()),
                ("B".into(), "b".into()),
                ("C".into(), "c".into()),
                ("D".into(), "d".into()),
            ],
        };
        let p = message_payload("57300", &r);
        let b = p["interactive"]["action"]["buttons"].as_array().unwrap();
        assert_eq!(b.len(), 3);
        assert_eq!(b[0]["reply"]["title"].as_str().unwrap().chars().count(), 20);
        assert_eq!(
            p["interactive"]["body"]["text"]
                .as_str()
                .unwrap()
                .chars()
                .count(),
            1024
        );
    }
}
