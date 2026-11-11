//! Serialización CSV compatible con Excel en español (BOM UTF-8, `;` y `,` decimal).

use axum::http::{HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use chrono::NaiveDate;
use rust_decimal::Decimal;

use crate::money::money;

/// Marca de orden de bytes UTF-8 para que Excel reconozca tildes y ñ sin asistente.
pub const UTF8_BOM: &str = "\u{FEFF}";
const DELIM: char = ';';
const CRLF: &str = "\r\n";

/// Formatea un monto monetario o porcentaje redondeado a 2 decimales con coma decimal.
pub fn format_decimal_2(value: Decimal) -> String {
    format!("{:.2}", money(value)).replace('.', ",")
}

/// Formatea una cantidad eliminando ceros finales y usando coma decimal si hay fracción.
pub fn format_qty(value: Decimal) -> String {
    value.normalize().to_string().replace('.', ",")
}

/// Escapa una celda según RFC 4180 cuando contiene `;`, `"`, `\r` o `\n`.
pub fn escape_cell(raw: &str) -> String {
    if raw.contains([DELIM, '"', '\r', '\n']) {
        format!("\"{}\"", raw.replace('"', "\"\""))
    } else {
        raw.to_string()
    }
}

/// Construye un documento CSV con UTF-8 BOM inicial, delimitador `;` y fin de línea CRLF.
pub fn build_csv(headers: &[&str], rows: &[Vec<String>]) -> String {
    let mut out = String::from(UTF8_BOM);
    out.push_str(
        &headers
            .iter()
            .map(|h| escape_cell(h))
            .collect::<Vec<_>>()
            .join(";"),
    );
    out.push_str(CRLF);
    for row in rows {
        out.push_str(
            &row.iter()
                .map(|c| escape_cell(c))
                .collect::<Vec<_>>()
                .join(";"),
        );
        out.push_str(CRLF);
    }
    out
}

/// Arma la respuesta HTTP `text/csv; charset=utf-8` con `Content-Disposition`.
pub fn csv_response(report_slug: &str, from: NaiveDate, to: NaiveDate, body: String) -> Response {
    let safe_slug: String = report_slug
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_')
        .collect();
    let filename = format!("{safe_slug}_{from}_{to}.csv");
    let disposition = format!("attachment; filename=\"{filename}\"");

    let mut resp = (StatusCode::OK, body).into_response();
    let headers = resp.headers_mut();
    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("text/csv; charset=utf-8"),
    );
    if let Ok(val) = HeaderValue::from_str(&disposition) {
        headers.insert(header::CONTENT_DISPOSITION, val);
    }
    headers.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    resp
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::to_bytes;
    use std::str::FromStr;

    fn d(s: &str) -> Decimal {
        Decimal::from_str(s).expect("decimal literal")
    }

    #[test]
    fn formats_numbers_with_comma_decimal() {
        assert_eq!(format_decimal_2(d("0")), "0,00");
        assert_eq!(format_decimal_2(d("12500.5")), "12500,50");
        assert_eq!(format_decimal_2(d("-1500.255")), "-1500,26");
        assert_eq!(format_qty(d("10.0000")), "10");
        assert_eq!(format_qty(d("1.5000")), "1,5");
    }

    #[test]
    fn builds_utf8_bom_semicolon_csv_with_rfc4180_escaping() {
        let csv = build_csv(
            &["producto", "categoría", "valor"],
            &[vec![
                "Café; \"Especial\"".into(),
                "Lácteos y cafés".into(),
                format_decimal_2(d("42500.75")),
            ]],
        );

        assert!(csv.starts_with(UTF8_BOM));
        assert!(csv.as_bytes().starts_with(&[0xEF, 0xBB, 0xBF]));
        let without_bom = csv.trim_start_matches(UTF8_BOM);
        let lines: Vec<&str> = without_bom.split(CRLF).collect();
        assert_eq!(lines[0], "producto;categoría;valor");
        assert_eq!(
            lines[1],
            "\"Café; \"\"Especial\"\"\";Lácteos y cafés;42500,75"
        );
        assert_eq!(lines[2], "");
    }

    #[tokio::test]
    async fn csv_response_sets_headers_and_filename() {
        let from = NaiveDate::from_ymd_opt(2026, 11, 1).unwrap();
        let to = NaiveDate::from_ymd_opt(2026, 11, 10).unwrap();
        let body = build_csv(&["col"], &[vec!["12,50".into()]]);
        let resp = csv_response("sales-by-day", from, to, body);

        assert_eq!(resp.status(), StatusCode::OK);
        assert_eq!(
            resp.headers().get(header::CONTENT_TYPE).unwrap(),
            "text/csv; charset=utf-8"
        );
        assert_eq!(
            resp.headers().get(header::CONTENT_DISPOSITION).unwrap(),
            "attachment; filename=\"sales-by-day_2026-11-01_2026-11-10.csv\""
        );
        let bytes = to_bytes(resp.into_body(), 1024 * 64).await.unwrap();
        assert!(bytes.starts_with(&[0xEF, 0xBB, 0xBF]));
    }
}
