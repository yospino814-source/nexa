//! Punta a punta contra la BD real: una venta y su devolución salen como
//! documento POS y nota crédito, primero en modo SIMULADO y luego contra un
//! MATIAS de mentira (servidor local) que revisa lo que le llega.
//! Se salta sin `DATABASE_URL`.

use std::sync::{Arc, Mutex};

use axum::extract::{Query as AxQuery, State as AxState};
use axum::http::HeaderMap;
use axum::routing::{get, post};
use rust_decimal::Decimal;
use serde_json::{Value, json};
use uuid::Uuid;

use super::*;
use crate::sales::{Checkout, PaymentMethod, SaleItem, SalePayment, SaleRequest};
use crate::test_support::*;

#[derive(Default)]
struct Mock {
    bodies: Vec<(String, Value)>,
    auth: Vec<String>,
    client_uuid: Vec<Option<String>>,
    reject_next: bool,
}

type Shared = Arc<Mutex<Mock>>;

async fn mock_send(
    path: &'static str,
    st: Shared,
    headers: HeaderMap,
    q: HashMap<String, String>,
    body: Value,
) -> axum::Json<Value> {
    let mut m = st.lock().unwrap();
    m.auth.push(
        headers
            .get("authorization")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .to_string(),
    );
    m.client_uuid.push(q.get("client_uuid").cloned());
    m.bodies.push((path.to_string(), body.clone()));
    if m.reject_next {
        m.reject_next = false;
        return axum::Json(json!({
            "success": true,
            "response": { "IsValid": "false", "StatusCode": "99", "StatusDescription": "Documento con errores en campos obligatorios.",
                          "ErrorMessage": { "string": ["FAJ43b: nombre del cliente"] } }
        }));
    }
    let key = format!(
        "CUDE-MOCK-{}",
        body["document_number"].as_str().unwrap_or("0")
    );
    axum::Json(json!({
        "uuid": "mock-uuid",
        "message": "Solicitud procesada por la DIAN.",
        "XmlDocumentKey": key,
        "response": { "IsValid": "true", "StatusCode": "00", "XmlDocumentKey": key },
        "qr": { "qrDian": format!("https://catalogo-vpfe.dian.gov.co/document/searchqr?documentkey={key}") },
        "pdf": { "url": "https://mock/pdf" },
        "warnings": [],
        "success": true
    }))
}

use std::collections::HashMap;

async fn start_mock() -> (String, Shared) {
    let st: Shared = Arc::new(Mutex::new(Mock::default()));
    let app = axum::Router::new()
        .route(
            "/api/ubl2.1/invoice",
            post(
                |AxState(s): AxState<Shared>,
                 h: HeaderMap,
                 AxQuery(q): AxQuery<HashMap<String, String>>,
                 axum::Json(b): axum::Json<Value>| {
                    mock_send("/invoice", s, h, q, b)
                },
            ),
        )
        .route(
            "/api/ubl2.1/notes/credit",
            post(
                |AxState(s): AxState<Shared>,
                 h: HeaderMap,
                 AxQuery(q): AxQuery<HashMap<String, String>>,
                 axum::Json(b): axum::Json<Value>| {
                    mock_send("/notes/credit", s, h, q, b)
                },
            ),
        )
        .route(
            "/api/ubl2.1/status",
            get(|| async { axum::Json(json!({ "document": { "is_valid": 0 } })) }),
        )
        .with_state(st.clone());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    (format!("http://{addr}/api/ubl2.1"), st)
}

fn dec(n: i64) -> Decimal {
    Decimal::from(n)
}

fn sale(branch: Uuid, product: Uuid, qty: i64, total: i64) -> SaleRequest {
    SaleRequest {
        id: Uuid::new_v4(),
        branch_id: branch,
        items: vec![SaleItem {
            product_id: Some(product),
            custom_name: None,
            qty: dec(qty),
            unit_price_override: None,
            discount_pct: None,
            tier_id: None,
            price_override_reason: None,
            custom_tax: None,
            option_ids: vec![],
        }],
        payments: vec![SalePayment {
            method: PaymentMethod::Efectivo,
            amount: dec(total),
            reference: None,
        }],
        quoted_total: dec(total),
        ..Default::default()
    }
}

async fn status_of(
    admin: &PgPool,
    order: Uuid,
    doc_type: &str,
) -> (String, Option<String>, Option<String>, Option<String>) {
    sqlx::query_as(
        "select t.tima_codigo, f.cude, f.qr_url, f.last_error from fiscal_documents f
           join tbl_tipos_maestra t on t.tima_id = f.status_id where f.order_id = $1 and f.doc_type = $2",
    )
    .bind(order)
    .bind(doc_type)
    .fetch_one(admin)
    .await
    .unwrap()
}

#[tokio::test]
async fn pos_and_credit_note_end_to_end() {
    let (Some(admin), Some(app)) = (admin_pool().await, app_pool().await) else {
        eprintln!("sin DATABASE_URL: se salta la prueba de facturación");
        return;
    };
    let tenant = seed_tenant(&admin).await;
    let branch = seed_branch(&admin, tenant).await;
    let owner = seed_user(&admin).await;
    seed_membership(&admin, tenant, None, owner, "DUENO").await;
    let c = claims(owner, Some(tenant), Some(branch));
    let product = seed_product(
        &admin,
        tenant,
        "Aguardiente Antioqueño 750 ml",
        "UND",
        Some("IVA19"),
        false,
    )
    .await;
    set_base_price(&admin, product, "50000").await;
    seed_branch_product(&admin, tenant, branch, product, None, None).await;
    seed_stock(&admin, tenant, branch, product, "20", "0").await;
    let register: Uuid = sqlx::query_scalar(
        "select id from cash_registers where branch_id = $1 and code = 'CAJA-1'",
    )
    .bind(branch)
    .fetch_one(&admin)
    .await
    .unwrap();
    crate::cash::open(&app, &c, branch, register, dec(0))
        .await
        .unwrap();
    let none = Providers::default();

    // Sin facturación activada, la venta queda PENDIENTE y el proceso no la toca.
    let s1 = match crate::sales::checkout(&app, &c, sale(branch, product, 2, 119000))
        .await
        .unwrap()
    {
        Checkout::Created(s) => s,
        Checkout::Replayed(_) => panic!("venta nueva"),
    };
    let order1 = s1.order.id;
    process_due(&app, &none, 50).await.unwrap();
    assert_eq!(status_of(&admin, order1, "POS").await.0, "PENDIENTE");

    // Modo SIMULADO: acepta con un CUDE de 96 caracteres.
    let mut cfg = settings(&app, &c, branch).await.unwrap();
    cfg.enabled = true;
    save_settings(&app, &c, &cfg).await.unwrap();
    process_due(&app, &none, 50).await.unwrap();
    let (st, cude, _, _) = status_of(&admin, order1, "POS").await;
    assert_eq!(st, "ACEPTADO");
    assert_eq!(cude.unwrap().len(), 96);

    // Devolución de 1 unidad: nota crédito que referencia el POS aceptado.
    let item: Uuid = sqlx::query_scalar("select id from order_items where order_id = $1")
        .bind(order1)
        .fetch_one(&admin)
        .await
        .unwrap();
    crate::refunds::create(
        &app,
        &c,
        crate::refunds::RefundRequest {
            id: Uuid::new_v4(),
            order_id: order1,
            method: crate::refunds::RefundMethod::Efectivo,
            reason: "Botella rota".into(),
            reversal_reference: None,
            items: vec![crate::refunds::RefundItemInput {
                order_item_id: item,
                qty: dec(1),
                disposition: crate::refunds::Disposition::Reintegrar,
            }],
        },
    )
    .await
    .unwrap();
    process_due(&app, &none, 50).await.unwrap();
    assert_eq!(
        status_of(&admin, order1, "NOTA_CREDITO").await.0,
        "ACEPTADO"
    );

    // MATIAS sin resolución: no se deja activar.
    cfg = settings(&app, &c, branch).await.unwrap();
    cfg.provider = "MATIAS".into();
    assert!(save_settings(&app, &c, &cfg).await.is_err());

    // Con resolución y sub-cuenta de Casa de Software.
    let (url, mock) = start_mock().await;
    let matias = Providers {
        matias: Some(matias::Client::new(&url, "token-prueba")),
    };
    cfg.provider_client_id = Some("cliente-123".into());
    cfg.pos_address = Some("Calle 15 #8-20, Riohacha".into());
    let mut pos = cfg.pos.clone().unwrap();
    pos.resolution_number = Some("18764071164948".into());
    pos.range_to = Some(5000);
    cfg.pos = Some(pos);
    let mut nc = cfg.credit_notes.clone().unwrap();
    nc.resolution_number = Some("18764071164949".into());
    cfg.credit_notes = Some(nc);
    save_settings(&app, &c, &cfg).await.unwrap();

    let s2 = match crate::sales::checkout(&app, &c, sale(branch, product, 1, 59500))
        .await
        .unwrap()
    {
        Checkout::Created(s) => s,
        Checkout::Replayed(_) => panic!("venta nueva"),
    };
    process_due(&app, &matias, 50).await.unwrap();
    let (st, cude, qr, _) = status_of(&admin, s2.order.id, "POS").await;
    assert_eq!(st, "ACEPTADO");
    assert_eq!(cude.as_deref(), Some("CUDE-MOCK-2"));
    assert!(qr.unwrap().contains("CUDE-MOCK-2"));
    {
        let m = mock.lock().unwrap();
        let (path, body) = m.bodies.last().unwrap();
        assert_eq!(path, "/invoice");
        assert_eq!(body["type_document_id"], 20);
        assert_eq!(body["resolution_number"], "18764071164948");
        assert_eq!(body["point_of_sale"]["address"], "Calle 15 #8-20, Riohacha");
        assert_eq!(body["legal_monetary_totals"]["payable_amount"], "59500.00");
        assert_eq!(
            body["lines"][0]["description"],
            "Aguardiente Antioqueño 750 ml"
        );
        assert_eq!(m.auth.last().unwrap(), "Bearer token-prueba");
        assert_eq!(
            m.client_uuid.last().unwrap().as_deref(),
            Some("cliente-123")
        );
    }

    // El tiquete: documento de la venta con el QR ya dibujado.
    let doc = document_of_order(&app, &c, s2.order.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        (doc.status.as_str(), doc.provider.as_deref()),
        ("ACEPTADO", Some("MATIAS"))
    );
    assert!(doc.qr_svg.unwrap().contains("<svg"));
    let sim = document_of_order(&app, &c, order1).await.unwrap().unwrap();
    assert!(
        sim.qr_svg.is_none(),
        "el modo simulado no trae QR de la DIAN"
    );

    // Rechazo de la DIAN: queda RECHAZADO con el motivo y se puede reintentar.
    mock.lock().unwrap().reject_next = true;
    let s3 = match crate::sales::checkout(&app, &c, sale(branch, product, 1, 59500))
        .await
        .unwrap()
    {
        Checkout::Created(s) => s,
        Checkout::Replayed(_) => panic!("venta nueva"),
    };
    process_due(&app, &matias, 50).await.unwrap();
    let (st, _, _, err) = status_of(&admin, s3.order.id, "POS").await;
    assert_eq!(st, "RECHAZADO");
    assert!(err.unwrap().contains("FAJ43b"));
    let id: Uuid = sqlx::query_scalar("select id from fiscal_documents where order_id = $1")
        .bind(s3.order.id)
        .fetch_one(&admin)
        .await
        .unwrap();
    retry(&app, &c, id).await.unwrap();
    process_due(&app, &matias, 50).await.unwrap();
    assert_eq!(status_of(&admin, s3.order.id, "POS").await.0, "ACEPTADO");

    // Otro negocio no ve ni toca la configuración.
    let other = seed_tenant(&admin).await;
    let other_branch = seed_branch(&admin, other).await;
    let stranger = seed_user(&admin).await;
    seed_membership(&admin, other, None, stranger, "DUENO").await;
    assert!(
        settings(&app, &claims(stranger, Some(other), None), branch)
            .await
            .is_err()
    );

    let _ = other_branch;
    for t in [tenant, other] {
        sqlx::query("delete from fiscal_settings where tenant_id = $1")
            .bind(t)
            .execute(&admin)
            .await
            .unwrap();
    }
    cleanup(&admin, tenant, &[owner]).await;
    cleanup(&admin, other, &[stranger]).await;
}
