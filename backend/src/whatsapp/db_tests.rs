//! Prueba de punta a punta contra la BD real: un cliente pide por WhatsApp,
//! el pedido queda en la bandeja de la sede y el equipo lo despacha.
//! Se salta sin `DATABASE_URL`.

use rust_decimal::Decimal;
use uuid::Uuid;

use super::cloud::Inbound;
use super::engine::Input;
use super::*;
use crate::test_support::*;

fn msg(id: &str, input: Input) -> Inbound {
    let body = match &input {
        Input::Text(t) | Input::Button(t) => t.clone(),
    };
    Inbound {
        phone_number_id: "990011".into(),
        from: "573005551234".into(),
        wa_message_id: id.into(),
        contact_name: Some("Ramiro Epieyú".into()),
        input,
        body,
    }
}

#[tokio::test]
async fn whatsapp_order_end_to_end() {
    let (Some(admin), Some(app)) = (admin_pool().await, app_pool().await) else {
        eprintln!("sin DATABASE_URL: se salta la prueba de WhatsApp");
        return;
    };
    let tenant = seed_tenant(&admin).await;
    let branch = seed_branch(&admin, tenant).await;
    let owner = seed_user(&admin).await;
    seed_membership(&admin, tenant, None, owner, "DUENO").await;
    let cat = seed_category(&admin, tenant, branch).await;
    let whisky = seed_product(
        &admin,
        tenant,
        "Whisky Old Parr 12 años 750 ml",
        "UND",
        None,
        false,
    )
    .await;
    seed_branch_product(&admin, tenant, branch, whisky, Some(cat), Some("165000")).await;
    seed_stock(&admin, tenant, branch, whisky, "4", "0").await;
    let hielo = seed_product(
        &admin,
        tenant,
        "Hielo bolsa 2 kg",
        "UND",
        Some("IVA19"),
        false,
    )
    .await;
    seed_branch_product(&admin, tenant, branch, hielo, Some(cat), Some("10000")).await;
    seed_stock(&admin, tenant, branch, hielo, "50", "0").await;
    sqlx::query(
        "insert into whatsapp_bots (tenant_id, branch_id, phone_number_id, business_name, opens_at, closes_at, min_order)
         values ($1, $2, '990011', 'Licorera La 15', '00:00', '00:00', 30000)",
    )
    .bind(tenant)
    .bind(branch)
    .execute(&admin)
    .await
    .unwrap();
    sqlx::query(
        "insert into whatsapp_delivery_zones (tenant_id, branch_id, name, fee, eta_min, eta_max) values ($1, $2, 'Centro', 4000, 20, 30)",
    )
    .bind(tenant)
    .bind(branch)
    .execute(&admin)
    .await
    .unwrap();

    // Pedido: 1 whisky y 2 hielos (el hielo lleva IVA: 10.000 → 11.900).
    let (_, r) = process(
        &app,
        &msg("w1", Input::Text("Buenas, 1 old parr y 2 hielos".into())),
    )
    .await
    .unwrap()
    .unwrap();
    let all = r
        .iter()
        .map(cloud::reply_text)
        .collect::<Vec<_>>()
        .join("\n");
    assert!(all.contains("Productos: $188.800"), "{all}");

    // Un reintento de Meta con el mismo id no se responde dos veces.
    assert!(
        process(
            &app,
            &msg("w1", Input::Text("Buenas, 1 old parr y 2 hielos".into()))
        )
        .await
        .unwrap()
        .is_none()
    );

    process(&app, &msg("w2", Input::Button("CONFIRM".into())))
        .await
        .unwrap();
    process(&app, &msg("w3", Input::Button("AGE_OK".into())))
        .await
        .unwrap();
    process(
        &app,
        &msg("w4", Input::Text("Calle 15 #8-20, Centro".into())),
    )
    .await
    .unwrap();
    let (_, r) = process(&app, &msg("w5", Input::Button("PAY_CASH".into())))
        .await
        .unwrap()
        .unwrap();
    let last = cloud::reply_text(r.last().unwrap());
    assert!(
        last.contains("W-0001") && last.contains("$192.800"),
        "{last}"
    );

    let (id, total, status, payment): (Uuid, Decimal, String, String) = sqlx::query_as(
        "select id, total, status, payment_method from whatsapp_orders where branch_id = $1",
    )
    .bind(branch)
    .fetch_one(&admin)
    .await
    .unwrap();
    assert_eq!(
        (total, status.as_str(), payment.as_str()),
        (Decimal::from(192800), "NUEVO", "EFECTIVO")
    );

    // El equipo lo prepara y lo despacha; no se puede saltar pasos.
    let owner_claims = claims(owner, Some(tenant), None);
    let step = |s: &str| StatusBody {
        status: s.into(),
        courier_name: Some("Jhon".into()),
        sale_id: None,
    };
    assert!(
        change_status(&app, &owner_claims, id, step("ENTREGADO"))
            .await
            .is_err()
    );
    change_status(&app, &owner_claims, id, step("PREPARANDO"))
        .await
        .unwrap();
    let (o, notice) = change_status(&app, &owner_claims, id, step("EN_CAMINO"))
        .await
        .unwrap();
    assert_eq!(o.status, "EN_CAMINO");
    let (_, phone, reply) = notice.expect("aviso al cliente");
    assert_eq!(phone, "573005551234");
    assert!(cloud::reply_text(&reply).contains("va en camino con Jhon"));

    // Otro negocio no ve el pedido.
    let other = seed_tenant(&admin).await;
    let other_branch = seed_branch(&admin, other).await;
    let stranger = seed_user(&admin).await;
    seed_membership(&admin, other, Some(other_branch), stranger, "DUENO").await;
    let err = change_status(
        &app,
        &claims(stranger, Some(other), Some(other_branch)),
        id,
        step("ENTREGADO"),
    )
    .await;
    assert!(matches!(err, Err(ApiError::Domain(DomainError::NotFound))));

    // La segunda vez ofrece "lo de siempre" y la dirección guardada.
    let (_, r) = process(&app, &msg("w6", Input::Text("hola".into())))
        .await
        .unwrap()
        .unwrap();
    let Reply::Buttons { buttons, .. } = &r[0] else {
        panic!("saludo con botones")
    };
    assert_eq!(buttons[0].0, "USUAL");

    for t in [tenant, other] {
        for table in [
            "whatsapp_orders",
            "whatsapp_messages",
            "whatsapp_conversations",
            "whatsapp_delivery_zones",
            "whatsapp_bots",
        ] {
            sqlx::query(&format!("delete from {table} where tenant_id = $1"))
                .bind(t)
                .execute(&admin)
                .await
                .unwrap();
        }
    }
    cleanup(&admin, tenant, &[owner]).await;
    cleanup(&admin, other, &[stranger]).await;
}
