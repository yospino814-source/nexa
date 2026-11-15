//! MATIAS API (proveedor tecnológico): arma el JSON del documento equivalente
//! POS electrónico y de la nota crédito POS, lo envía y lee la respuesta.
//!
//! Catálogos usados (tablas paramétricas DIAN de MATIAS):
//! - `type_document_id`: 20 = P.O.S Electrónico, 94 = Nota crédito P.O.S.
//! - `operation_type_id`: 1 = Estándar, 11 = Nota de ajuste al documento equivalente POS.
//! - `tax_id`: 1 = IVA, 4 = INC, 18 = ICL (Impuesto al Consumo de Licores).
//! - `quantity_units_id`: 70 = unidad (código 94).
//! - `means_payment_id`: 10 efectivo, 49 tarjeta débito, 47 transferencia, 1 no definido.
//! - `identity_document_id`: 1 = cédula, 3 = NIT, 2 = cédula de extranjería, 9 = pasaporte.
//! - Motivo de la nota (`discrepancy_response.response_id`): 15 = devolución
//!   parcial, 16 = anulación del documento equivalente electrónico.

use std::collections::BTreeMap;
use std::time::Duration;

use rust_decimal::{Decimal, RoundingStrategy};
use serde_json::{Value, json};

use super::{CreditNote, Line, Outcome, Payment, PosDoc};

pub const SANDBOX_URL: &str = "https://sandbox-api.matias-api.com/api/ubl2.1";

/// Quién hizo el software (va en cada documento POS).
pub const SOFTWARE_OWNER: &str = "Nexa POS";
pub const SOFTWARE_COMPANY: &str = "Nexa POS";
pub const SOFTWARE_NAME: &str = "Nexa POS";

fn money(x: Decimal) -> String {
    format!(
        "{:.2}",
        x.round_dp_with_strategy(2, RoundingStrategy::MidpointAwayFromZero)
    )
}

fn qty(x: Decimal) -> String {
    x.normalize().to_string()
}

/// Impuesto de la línea según su tarifa: 8 % es INC, el resto IVA (0 % incluido).
pub fn tax_of(rate: Decimal) -> (&'static str, Decimal) {
    let percent = (rate * Decimal::from(100)).normalize();
    if percent == Decimal::from(8) {
        ("4", percent)
    } else {
        ("1", percent)
    }
}

fn means_of(method: &str) -> (u8, u8) {
    // (payment_method_id: 1 contado / 2 crédito, means_payment_id)
    match method {
        "EFECTIVO" => (1, 10),
        "DATAFONO" => (1, 49),
        "QR" => (1, 47),
        "CREDITO" => (2, 1),
        _ => (1, 1),
    }
}

fn identity_of(doc_type: Option<&str>) -> Option<&'static str> {
    Some(match doc_type? {
        "CC" => "1",
        "CE" => "2",
        "NIT" => "3",
        "PP" => "9",
        "TI" => "7",
        "PPT" => "13",
        "PEP" => "14",
        "NIUP" => "12",
        _ => return None,
    })
}

/// Precio unitario neto (después de descuentos) para que precio × cantidad
/// dé el valor de la línea.
fn unit_net(l: &Line) -> String {
    if l.qty.is_zero() {
        return money(l.line_total);
    }
    let p = l.line_total / l.qty;
    let two = p.round_dp(2);
    if two * l.qty == l.line_total {
        money(two)
    } else {
        p.round_dp(6).normalize().to_string()
    }
}

fn line_json(l: &Line) -> Value {
    let (tax_id, percent) = if l.tax_id.is_empty() {
        tax_of(l.tax_rate)
    } else if l.tax_id == "18" {
        ("18", Decimal::ZERO)
    } else {
        (
            l.tax_id.as_str(),
            (l.tax_rate * Decimal::from(100)).normalize(),
        )
    };
    json!({
        "invoiced_quantity": qty(l.qty),
        "quantity_units_id": "70",
        "line_extension_amount": money(l.line_total),
        "free_of_charge_indicator": false,
        "description": l.description,
        "code": l.code,
        "type_item_identifications_id": "4",
        "reference_price_id": "1",
        "price_amount": unit_net(l),
        "base_quantity": qty(l.qty),
        "tax_totals": [{
            "tax_id": tax_id,
            "tax_amount": money(l.tax_amount),
            "taxable_amount": money(l.line_total),
            "percent": percent.to_string()
        }]
    })
}

/// Totales del documento: suma por impuesto y tarifa.
fn totals(lines: &[Line]) -> (Value, Value) {
    let mut by_tax: BTreeMap<(&str, String), (Decimal, Decimal)> = BTreeMap::new();
    let (mut base, mut tax) = (Decimal::ZERO, Decimal::ZERO);
    for l in lines {
        base += l.line_total;
        tax += l.tax_amount;
        let (id, pct) = if l.tax_id.is_empty() {
            tax_of(l.tax_rate)
        } else if l.tax_id == "18" {
            ("18", Decimal::ZERO)
        } else {
            (
                l.tax_id.as_str(),
                (l.tax_rate * Decimal::from(100)).normalize(),
            )
        };
        let e = by_tax.entry((id, pct.to_string())).or_default();
        e.0 += l.tax_amount;
        e.1 += l.line_total;
    }
    let tax_totals: Vec<Value> = by_tax
        .into_iter()
        .map(|((id, pct), (t, b))| json!({ "tax_id": id, "tax_amount": money(t), "taxable_amount": money(b), "percent": pct }))
        .collect();
    let monetary = json!({
        "line_extension_amount": money(base),
        "tax_exclusive_amount": money(base),
        "tax_inclusive_amount": money(base + tax),
        "payable_amount": money(base + tax)
    });
    (Value::Array(tax_totals), monetary)
}

fn payments_json(payments: &[Payment], total: Decimal) -> Value {
    let list: Vec<Value> = if payments.is_empty() {
        vec![json!({ "payment_method_id": 1, "means_payment_id": 10, "value_paid": money(total) })]
    } else {
        payments
            .iter()
            .map(|p| {
                let (m, means) = means_of(&p.method);
                json!({ "payment_method_id": m, "means_payment_id": means, "value_paid": money(p.amount) })
            })
            .collect()
    };
    Value::Array(list)
}

fn customer_json(c: &super::Customer) -> Value {
    let mut v = json!({ "company_name": c.name, "dni": c.doc_number, "points": 0 });
    if let Some(id) = identity_of(c.doc_type.as_deref()) {
        v["identity_document_id"] = json!(id);
    }
    if let Some(e) = c.email.as_deref().filter(|e| !e.trim().is_empty()) {
        v["email"] = json!(e);
    }
    v
}

/// JSON del documento equivalente POS electrónico (`POST /invoice`).
pub fn pos_json(d: &PosDoc) -> Value {
    let (tax_totals, monetary) = totals(&d.lines);
    let total: Decimal = d.lines.iter().map(|l| l.line_total + l.tax_amount).sum();
    let mut v = json!({
        "uuid": d.id.to_string(),
        "resolution_number": d.resolution_number,
        "prefix": d.prefix,
        "document_number": d.number.to_string(),
        "graphic_representation": 1,
        "send_email": if d.customer.as_ref().and_then(|c| c.email.as_deref()).is_some_and(|e| !e.is_empty()) { 1 } else { 0 },
        "operation_type_id": 1,
        "type_document_id": 20,
        "payments": payments_json(&d.payments, total),
        "document_signature": { "cashier": d.cashier, "seller": d.cashier },
        "point_of_sale": {
            "cashier_name": d.cashier,
            "terminal_number": d.terminal,
            "cashier_type": "Caja",
            "sales_code": d.prefix,
            "address": d.address,
            "sub_total": money(d.lines.iter().map(|l| l.line_total).sum())
        },
        "software_manufacturer": {
            "owner_name": SOFTWARE_OWNER,
            "company_name": SOFTWARE_COMPANY,
            "software_name": SOFTWARE_NAME
        },
        "lines": d.lines.iter().map(line_json).collect::<Vec<_>>(),
        "legal_monetary_totals": monetary,
        "tax_totals": tax_totals
    });
    if let Some(c) = &d.customer {
        v["customer"] = customer_json(c);
    }
    if let Some(n) = d.notes.as_deref().filter(|n| !n.is_empty()) {
        v["notes"] = json!(n);
    }
    v
}

/// JSON de la nota crédito POS (`POST /notes/credit`).
pub fn credit_note_json(n: &CreditNote) -> Value {
    let (tax_totals, monetary) = totals(&n.lines);
    let total: Decimal = n.lines.iter().map(|l| l.line_total + l.tax_amount).sum();
    let reference = format!("{}{}", n.ref_prefix, n.ref_number);
    let mut v = json!({
        "uuid": n.id.to_string(),
        "resolution_number": n.resolution_number,
        "prefix": n.prefix,
        "document_number": n.number.to_string(),
        "notes": n.reason,
        "graphic_representation": 1,
        "send_email": 0,
        "operation_type_id": 11,
        "type_document_id": 94,
        "discrepancy_response": { "reference_id": reference, "response_id": if n.full { "16" } else { "15" } },
        "billing_reference": { "number": reference, "date": n.ref_date.to_string(), "uuid": n.ref_cude },
        "payments": payments_json(&n.payments, total),
        "lines": n.lines.iter().map(line_json).collect::<Vec<_>>(),
        "legal_monetary_totals": monetary,
        "tax_totals": tax_totals
    });
    if let Some(c) = &n.customer {
        v["customer"] = customer_json(c);
    }
    v
}

/// Respuesta de MATIAS → resultado para Nexa.
pub fn classify(status: u16, body: &Value) -> Outcome {
    let text = |v: &Value| v.as_str().map(str::to_string);
    let resp = &body["response"];
    match status {
        200 | 201 if resp.is_object() || body["XmlDocumentKey"].is_string() => {
            let valid = resp["IsValid"].as_str() == Some("true") || body["is_valid"] == json!(1);
            let code = resp["StatusCode"].as_str().unwrap_or("");
            let cude = text(&body["XmlDocumentKey"])
                .or_else(|| text(&resp["XmlDocumentKey"]))
                .filter(|s| !s.is_empty());
            if valid && (code == "00" || code.is_empty()) {
                Outcome::Accepted {
                    cude: cude.unwrap_or_default(),
                    provider_uuid: text(&body["uuid"]),
                    qr_url: text(&body["qr"]["qrDian"]).or_else(|| text(&body["qr"]["url"])),
                    pdf_url: text(&body["pdf"]["url"]),
                }
            } else if code == "98" {
                Outcome::InProcess { cude }
            } else {
                let reasons: Vec<String> = resp["ErrorMessage"]["string"]
                    .as_array()
                    .map(|a| {
                        a.iter()
                            .filter_map(|x| x.as_str().map(str::to_string))
                            .collect()
                    })
                    .or_else(|| text(&resp["ErrorMessage"]["string"]).map(|s| vec![s]))
                    .unwrap_or_default();
                let desc = text(&resp["StatusDescription"])
                    .unwrap_or_else(|| "Rechazado por la DIAN".into());
                Outcome::Rejected {
                    reason: if reasons.is_empty() {
                        desc
                    } else {
                        format!("{desc}: {}", reasons.join(" | "))
                    },
                }
            }
        }
        // Validación del JSON: es un error nuestro, reintentar no lo arregla.
        422 => Outcome::Rejected {
            reason: format!("JSON inválido para el proveedor: {}", body["errors"]),
        },
        400 if body["message"]
            .as_str()
            .is_some_and(|m| m.to_lowercase().contains("uuid")) =>
        {
            Outcome::CheckStatus {
                error: body["message"]
                    .as_str()
                    .unwrap_or("documento ya registrado")
                    .into(),
            }
        }
        409 | 500 | 503 if body["code"] == "DOCUMENT_POSSIBLY_SENT" || status == 409 => {
            Outcome::CheckStatus {
                error: body["message"]
                    .as_str()
                    .unwrap_or("posiblemente enviado")
                    .into(),
            }
        }
        _ => Outcome::Retry {
            error: format!(
                "HTTP {status}: {}",
                body["message"].as_str().unwrap_or("sin detalle")
            ),
        },
    }
}

#[derive(Clone)]
pub struct Client {
    base: String,
    token: String,
    http: reqwest::Client,
}

impl Client {
    pub fn new(base: &str, token: &str) -> Self {
        Self {
            base: base.trim_end_matches('/').to_string(),
            token: token.to_string(),
            http: reqwest::Client::builder()
                .timeout(Duration::from_secs(40))
                .build()
                .unwrap_or_default(),
        }
    }

    /// Desde `MATIAS_API_URL` (por defecto el sandbox) y `MATIAS_API_TOKEN`.
    pub fn from_env() -> Option<Self> {
        let token = std::env::var("MATIAS_API_TOKEN")
            .ok()
            .filter(|t| !t.is_empty())?;
        let base = std::env::var("MATIAS_API_URL")
            .ok()
            .filter(|u| !u.is_empty())
            .unwrap_or_else(|| SANDBOX_URL.into());
        Some(Self::new(&base, &token))
    }

    fn url(&self, path: &str, client: Option<&str>) -> String {
        match client {
            Some(c) if !c.is_empty() => format!("{}{}?client_uuid={}", self.base, path, c),
            _ => format!("{}{}", self.base, path),
        }
    }

    async fn call(&self, req: reqwest::RequestBuilder) -> Outcome {
        let res = req
            .bearer_auth(&self.token)
            .header("Accept", "application/json")
            .send()
            .await;
        match res {
            Ok(r) => {
                let status = r.status().as_u16();
                let body: Value = r.json().await.unwrap_or(Value::Null);
                classify(status, &body)
            }
            Err(e) => Outcome::Retry {
                error: format!("sin respuesta del proveedor: {e}"),
            },
        }
    }

    pub async fn send_pos(&self, body: &Value, client: Option<&str>) -> Outcome {
        self.call(self.http.post(self.url("/invoice", client)).json(body))
            .await
    }

    pub async fn send_credit_note(&self, body: &Value, client: Option<&str>) -> Outcome {
        self.call(self.http.post(self.url("/notes/credit", client)).json(body))
            .await
    }

    /// ¿El token sirve? Consulta los tokens de la cuenta (no gasta documentos).
    pub async fn ping(&self, client: Option<&str>) -> (bool, String) {
        let res = self
            .http
            .get(self.url("/tokens", client))
            .bearer_auth(&self.token)
            .header("Accept", "application/json")
            .send()
            .await;
        match res {
            Ok(r) if r.status().is_success() => (
                true,
                format!(
                    "Conectado con MATIAS ({}).",
                    if self.base.contains("sandbox") {
                        "sandbox"
                    } else {
                        "producción"
                    }
                ),
            ),
            Ok(r) if matches!(r.status().as_u16(), 401 | 403) => (
                false,
                "MATIAS rechazó el token: revisá MATIAS_API_TOKEN.".into(),
            ),
            Ok(r) => (
                false,
                format!("MATIAS respondió HTTP {}.", r.status().as_u16()),
            ),
            Err(e) => (false, format!("No se pudo llegar a MATIAS: {e}")),
        }
    }

    /// Estado de un documento ya enviado, por su número.
    pub async fn status(
        &self,
        prefix: &str,
        number: i64,
        resolution: &str,
        client: Option<&str>,
    ) -> Outcome {
        let mut url = self.url("/status", client);
        url.push(if url.contains('?') { '&' } else { '?' });
        url.push_str(&format!(
            "prefix={prefix}&number={number}&resolution={resolution}"
        ));
        let res = self
            .http
            .get(url)
            .bearer_auth(&self.token)
            .header("Accept", "application/json")
            .send()
            .await;
        match res {
            Ok(r) if r.status().as_u16() == 404 => Outcome::Retry {
                error: "el proveedor no tiene el documento: se reenvía".into(),
            },
            Ok(r) => {
                let body: Value = r.json().await.unwrap_or(Value::Null);
                let doc = &body["document"];
                if doc["is_valid"] == json!(1) || doc["is_valid"] == json!(true) {
                    Outcome::Accepted {
                        cude: doc["document_key"].as_str().unwrap_or_default().to_string(),
                        provider_uuid: doc["uuid"].as_str().map(str::to_string),
                        qr_url: doc["qr"]["qrDian"].as_str().map(str::to_string),
                        pdf_url: None,
                    }
                } else {
                    Outcome::InProcess {
                        cude: doc["document_key"].as_str().map(str::to_string),
                    }
                }
            }
            Err(e) => Outcome::Retry {
                error: format!("sin respuesta del proveedor: {e}"),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fiscal::{Customer, Line, Payment};
    use chrono::NaiveDate;
    use uuid::Uuid;

    fn d(s: &str) -> Decimal {
        s.parse().unwrap()
    }

    fn line(desc: &str, q: &str, total: &str, rate: &str) -> Line {
        let total = d(total);
        let rate = d(rate);
        let (tax_id, _) = tax_of(rate);
        Line {
            code: "P1".into(),
            description: desc.into(),
            qty: d(q),
            line_total: total,
            tax_rate: rate,
            tax_amount: (total * rate).round_dp(2),
            tax_id: tax_id.to_string(),
        }
    }

    fn liquor_line(desc: &str, q: &str, total: &str, degrees: u32, volume: u32) -> Line {
        let qty = d(q);
        let total = d(total);
        let icl_unit = crate::catalog::calculate_icl(Decimal::from(degrees), Decimal::from(volume));
        let tax_amount = icl_unit * qty;
        Line {
            code: "LIC1".into(),
            description: desc.into(),
            qty,
            line_total: total,
            tax_rate: Decimal::ZERO,
            tax_amount,
            tax_id: "18".into(),
        }
    }

    fn doc() -> PosDoc {
        PosDoc {
            id: Uuid::nil(),
            resolution_number: "18764071164948".into(),
            prefix: "FPOS".into(),
            number: 229,
            cashier: "Carlos Pérez".into(),
            terminal: "CAJA-1".into(),
            address: "Calle 15 #8-20".into(),
            notes: None,
            customer: None,
            lines: vec![
                line("Aguardiente 750", "2", "100000.00", "0.19"),
                line("Hielo", "3", "12000.00", "0.05"),
                line("Cigarrillos", "1", "10000.00", "0.08"),
            ],
            payments: vec![
                Payment {
                    method: "EFECTIVO".into(),
                    amount: d("100000"),
                },
                Payment {
                    method: "DATAFONO".into(),
                    amount: d("33400"),
                },
            ],
        }
    }

    #[test]
    fn pos_json_totals_add_up() {
        let v = pos_json(&doc());
        assert_eq!(v["type_document_id"], 20);
        assert_eq!(v["document_number"], "229");
        assert!(v.get("customer").is_none(), "consumidor final: sin cliente");
        let m = &v["legal_monetary_totals"];
        assert_eq!(m["line_extension_amount"], "122000.00");
        // 19.000 + 600 + 800
        assert_eq!(m["tax_inclusive_amount"], "142400.00");
        let taxes = v["tax_totals"].as_array().unwrap();
        assert_eq!(taxes.len(), 3);
        let inc = taxes.iter().find(|t| t["tax_id"] == "4").unwrap();
        assert_eq!(
            (
                inc["percent"].as_str().unwrap(),
                inc["tax_amount"].as_str().unwrap()
            ),
            ("8", "800.00")
        );
        let l = &v["lines"][0];
        assert_eq!(
            (
                l["price_amount"].as_str().unwrap(),
                l["invoiced_quantity"].as_str().unwrap()
            ),
            ("50000.00", "2")
        );
        assert_eq!(v["payments"][1]["means_payment_id"], 49);
    }

    #[test]
    fn pos_json_with_icl_tax() {
        let mut d = doc();
        d.lines.push(liquor_line(
            "Aguardiente Antioqueño 750",
            "2",
            "70000.00",
            29,
            750,
        ));
        let v = pos_json(&d);
        let taxes = v["tax_totals"].as_array().unwrap();
        let icl = taxes
            .iter()
            .find(|t| t["tax_id"] == "18")
            .expect("ICL tax total present");
        // 29 * (750/750) * 320 = 9280 * 2 = 18560
        assert_eq!(icl["tax_amount"], "18560.00");
        assert_eq!(icl["taxable_amount"], "70000.00");
        assert_eq!(icl["percent"], "0");

        let m = &v["legal_monetary_totals"];
        // base anterior 122000 + 70000 = 192000
        assert_eq!(m["line_extension_amount"], "192000.00");
        // total anterior 142400 + 70000 + 18560 = 230960
        assert_eq!(m["tax_inclusive_amount"], "230960.00");
    }

    #[test]
    fn identified_customer_and_odd_prices() {
        let mut doc = doc();
        doc.customer = Some(Customer {
            name: "Tienda Doña Rosa".into(),
            doc_type: Some("NIT".into()),
            doc_number: "900456789".into(),
            email: Some("rosa@x.co".into()),
        });
        doc.lines = vec![line("Promo", "3", "10000.00", "0.19")];
        let v = pos_json(&doc);
        assert_eq!(v["customer"]["identity_document_id"], "3");
        assert_eq!(v["send_email"], 1);
        // 10.000 / 3 no cierra en 2 decimales: se manda con más precisión.
        assert_eq!(v["lines"][0]["price_amount"], "3333.333333");
    }

    #[test]
    fn credit_note_references_pos() {
        let n = CreditNote {
            id: Uuid::nil(),
            resolution_number: "18764071164949".into(),
            prefix: "NCP".into(),
            number: 7,
            reason: "Devolución de 1 aguardiente".into(),
            ref_prefix: "FPOS".into(),
            ref_number: 229,
            ref_date: NaiveDate::from_ymd_opt(2026, 11, 14).unwrap(),
            ref_cude: "abc123".into(),
            full: false,
            customer: None,
            lines: vec![line("Aguardiente 750", "1", "50000.00", "0.19")],
            payments: vec![],
        };
        let v = credit_note_json(&n);
        assert_eq!(
            (
                v["type_document_id"].as_i64(),
                v["operation_type_id"].as_i64()
            ),
            (Some(94), Some(11))
        );
        assert_eq!(v["discrepancy_response"]["response_id"], "15");
        assert_eq!(v["billing_reference"]["number"], "FPOS229");
        assert_eq!(v["billing_reference"]["date"], "2026-11-14");
        assert_eq!(v["legal_monetary_totals"]["payable_amount"], "59500.00");
    }

    #[test]
    fn classifies_provider_answers() {
        let ok = json!({"uuid": "u1", "XmlDocumentKey": "CUDE1", "response": {"IsValid": "true", "StatusCode": "00"}, "qr": {"qrDian": "https://dian/qr"}, "pdf": {"url": "https://pdf"}});
        assert_eq!(
            classify(200, &ok),
            Outcome::Accepted {
                cude: "CUDE1".into(),
                provider_uuid: Some("u1".into()),
                qr_url: Some("https://dian/qr".into()),
                pdf_url: Some("https://pdf".into())
            }
        );
        let rej = json!({"response": {"IsValid": "false", "StatusCode": "99", "StatusDescription": "Documento con errores", "ErrorMessage": {"string": ["FAS01A"]}}});
        assert!(
            matches!(classify(200, &rej), Outcome::Rejected { reason } if reason.contains("FAS01A"))
        );
        assert!(matches!(
            classify(
                200,
                &json!({"response": {"IsValid": "false", "StatusCode": "98"}})
            ),
            Outcome::InProcess { .. }
        ));
        assert!(matches!(
            classify(422, &json!({"errors": {"customer.dni": ["x"]}})),
            Outcome::Rejected { .. }
        ));
        assert!(matches!(
            classify(500, &json!({"code": "DOCUMENT_POSSIBLY_SENT"})),
            Outcome::CheckStatus { .. }
        ));
        assert!(matches!(
            classify(503, &json!({"code": "DIAN_STATUS_UNAVAILABLE"})),
            Outcome::Retry { .. }
        ));
        assert!(matches!(
            classify(401, &json!({"message": "Unauthenticated."})),
            Outcome::Retry { .. }
        ));
    }
}
