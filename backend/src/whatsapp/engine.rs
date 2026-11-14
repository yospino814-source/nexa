//! Conversación del bot de pedidos. Es puro: recibe el estado guardado, el
//! mensaje del cliente y los datos de la sede, y devuelve el estado nuevo, las
//! respuestas y, si corresponde, el pedido a registrar. Así se prueba sin
//! base de datos ni WhatsApp.

use chrono::NaiveTime;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::text::{self, Match, Product};

/// Título máximo de un botón de respuesta en WhatsApp.
pub const MAX_BUTTON_TITLE: usize = 20;
/// Botones máximos por mensaje interactivo.
pub const MAX_BUTTONS: usize = 3;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Step {
    /// Esperando un pedido (o armándolo).
    #[default]
    Idle,
    /// Carrito mostrado, esperando "Sí, así está".
    ConfirmCart,
    /// Pedido con licor: falta confirmar mayoría de edad.
    AgeCheck,
    /// Falta la dirección o elegir recoger en tienda.
    Address,
    /// Falta el medio de pago.
    Payment,
    /// Lo atiende una persona; el bot no responde.
    Human,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CartLine {
    pub product_id: Uuid,
    pub name: String,
    pub qty: u32,
    /// Precio unitario con impuesto.
    pub unit_price: Decimal,
}

impl CartLine {
    pub fn total(&self) -> Decimal {
        self.unit_price * Decimal::from(self.qty)
    }
}

/// Estado que se guarda entre mensajes.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct Conversation {
    pub step: Step,
    pub cart: Vec<CartLine>,
    pub age_confirmed: bool,
    pub address: Option<String>,
    pub zone_id: Option<Uuid>,
    pub pickup: bool,
}

#[derive(Debug, Clone)]
pub struct Zone {
    pub id: Uuid,
    pub name: String,
    pub fee: Decimal,
    pub eta_min: i32,
    pub eta_max: i32,
}

#[derive(Debug, Clone)]
pub struct BotConfig {
    pub business_name: String,
    pub opens_at: NaiveTime,
    pub closes_at: NaiveTime,
    pub min_order: Decimal,
    pub prepay_enabled: bool,
    pub usual_enabled: bool,
    pub substitutes_enabled: bool,
    pub age_check: bool,
}

/// Datos de la sede y del cliente para responder un mensaje.
pub struct Context<'a> {
    pub cfg: &'a BotConfig,
    pub catalog: &'a [Product],
    pub zones: &'a [Zone],
    pub customer_name: Option<&'a str>,
    /// Último pedido del cliente ("lo de siempre").
    pub usual: Option<&'a [CartLine]>,
    /// Dirección del último domicilio.
    pub saved_address: Option<(&'a str, Option<Uuid>)>,
    /// Hora local de la sede.
    pub now: NaiveTime,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Input {
    Text(String),
    /// Id del botón que tocó el cliente.
    Button(String),
}

#[derive(Debug, Clone, PartialEq)]
pub enum Reply {
    Text(String),
    Buttons {
        body: String,
        buttons: Vec<(String, String)>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum PaymentMethod {
    /// Link de pago antes de despachar.
    Link,
    Efectivo,
    Datafono,
}

#[derive(Debug, Clone, PartialEq)]
pub struct OrderDraft {
    pub lines: Vec<CartLine>,
    pub subtotal: Decimal,
    pub delivery_fee: Decimal,
    pub total: Decimal,
    pub pickup: bool,
    pub address: Option<String>,
    pub zone_id: Option<Uuid>,
    pub payment: PaymentMethod,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Outcome {
    None,
    PlaceOrder(OrderDraft),
    /// El cliente pidió hablar con una persona.
    Handoff,
}

// ------------------------------------------------------------- formato

/// `$165.000`
pub fn money(x: Decimal) -> String {
    let n = x.round().abs().to_string();
    let n = n.split('.').next().unwrap_or("0");
    let mut out = String::new();
    for (i, c) in n.chars().enumerate() {
        if i > 0 && (n.len() - i).is_multiple_of(3) {
            out.push('.');
        }
        out.push(c);
    }
    format!(
        "{}${}",
        if x.is_sign_negative() && !x.is_zero() {
            "−"
        } else {
            ""
        },
        out
    )
}

fn hour(t: NaiveTime) -> String {
    use chrono::Timelike;
    let (h, m) = (t.hour(), t.minute());
    let (h12, suf) = match h {
        0 => (12, "a. m."),
        1..=11 => (h, "a. m."),
        12 => (12, "m."),
        _ => (h - 12, "p. m."),
    };
    if m == 0 {
        format!("{h12} {suf}")
    } else {
        format!("{h12}:{m:02} {suf}")
    }
}

fn buttons(body: impl Into<String>, list: &[(&str, &str)]) -> Reply {
    Reply::Buttons {
        body: body.into(),
        buttons: list
            .iter()
            .take(MAX_BUTTONS)
            .map(|(id, t)| (id.to_string(), t.to_string()))
            .collect(),
    }
}

fn subtotal(cart: &[CartLine]) -> Decimal {
    cart.iter().map(CartLine::total).sum()
}

fn cart_text(cart: &[CartLine]) -> String {
    let mut s = String::from("Tu pedido:\n");
    for l in cart {
        s.push_str(&format!("• {} ×{} — {}\n", l.name, l.qty, money(l.total())));
    }
    s.push_str(&format!("Productos: {}", money(subtotal(cart))));
    s
}

/// Abierto ahora, también en horarios que pasan la medianoche (10 a. m. a 1 a. m.).
pub fn is_open(cfg: &BotConfig, now: NaiveTime) -> bool {
    if cfg.opens_at == cfg.closes_at {
        return true;
    }
    if cfg.opens_at < cfg.closes_at {
        now >= cfg.opens_at && now < cfg.closes_at
    } else {
        now >= cfg.opens_at || now < cfg.closes_at
    }
}

const AGE_WORDS: &[&str] = &[
    "aguardiente",
    "ron",
    "whisky",
    "cerveza",
    "vino",
    "tequila",
    "vodka",
    "ginebra",
    "gin",
    "licor",
    "cigarrillo",
    "cigarrillos",
    "cigarro",
    "tabaco",
    "champana",
    "brandy",
    "cerveza",
    "aperitivo",
];

/// Licor o cigarrillos: hay que confirmar mayoría de edad.
pub fn is_age_restricted(p: &Product) -> bool {
    let n = text::normalize(&format!(
        "{} {}",
        p.name,
        p.category.clone().unwrap_or_default()
    ));
    n.split_whitespace().any(|w| AGE_WORDS.contains(&w))
}

// ------------------------------------------------------------- motor

struct Turn<'c> {
    conv: Conversation,
    replies: Vec<Reply>,
    outcome: Outcome,
    ctx: &'c Context<'c>,
}

impl<'c> Turn<'c> {
    fn say(&mut self, r: Reply) {
        self.replies.push(r);
    }

    fn text(&mut self, s: impl Into<String>) {
        self.replies.push(Reply::Text(s.into()));
    }

    fn product(&self, id: Uuid) -> Option<&'c Product> {
        self.ctx.catalog.iter().find(|p| p.id == id)
    }

    /// Suma al carrito respetando la existencia. Devuelve una nota si no alcanzó.
    fn add(&mut self, p: &Product, qty: u32) -> Option<String> {
        let in_cart: u32 = self
            .conv
            .cart
            .iter()
            .filter(|l| l.product_id == p.id)
            .map(|l| l.qty)
            .sum();
        let can = match p.available {
            Some(a) => (a.max(0) as u32).saturating_sub(in_cart).min(qty),
            None => qty,
        };
        if can > 0 {
            if let Some(l) = self.conv.cart.iter_mut().find(|l| l.product_id == p.id) {
                l.qty += can;
            } else {
                self.conv.cart.push(CartLine {
                    product_id: p.id,
                    name: p.name.clone(),
                    qty: can,
                    unit_price: p.price,
                });
            }
        }
        if can == qty {
            None
        } else if can == 0 {
            Some(format!("No hay {} en este momento.", p.name))
        } else {
            Some(format!("Solo quedan {can} de {}, te puse esas.", p.name))
        }
    }

    /// Producto parecido con existencia: misma categoría y precio más cercano.
    fn substitute(&self, p: &Product, qty: u32) -> Option<&'c Product> {
        if !self.ctx.cfg.substitutes_enabled {
            return None;
        }
        self.ctx
            .catalog
            .iter()
            .filter(|o| o.id != p.id && o.category.is_some() && o.category == p.category)
            .filter(|o| o.available.is_none_or(|a| a >= qty as i64))
            .min_by_key(|o| (o.price - p.price).abs())
    }

    fn show_cart(&mut self, notes: Vec<String>) {
        let mut body = String::new();
        for n in notes {
            body.push_str(&n);
            body.push('\n');
        }
        body.push_str(&cart_text(&self.conv.cart));
        body.push_str("\n¿Lo confirmo?");
        self.conv.step = Step::ConfirmCart;
        self.say(buttons(
            body,
            &[
                ("CONFIRM", "Sí, así está"),
                ("CHANGE", "Cambiar algo"),
                ("CATALOG", "Ver catálogo"),
            ],
        ));
    }

    fn greet(&mut self) {
        let hi = match self.ctx.customer_name {
            Some(n) => format!("¡Hola, {}!", n.split_whitespace().next().unwrap_or(n)),
            None => "¡Hola!".to_string(),
        };
        let body = format!(
            "{hi} Soy Nexi, de {}. Escribime lo que necesitás, por ejemplo: «2 Águila y 1 hielo».",
            self.ctx.cfg.business_name
        );
        let usual = self.usual().is_some();
        let mut list = Vec::new();
        if usual {
            list.push(("USUAL", "Lo de siempre"));
        }
        list.push(("CATALOG", "Ver catálogo"));
        list.push(("HUMAN", "Hablar con alguien"));
        self.say(buttons(body, &list));
    }

    fn usual(&self) -> Option<&'c [CartLine]> {
        if !self.ctx.cfg.usual_enabled {
            return None;
        }
        self.ctx.usual.filter(|u| !u.is_empty())
    }

    fn catalog(&mut self) {
        let mut s = String::from("Esto es lo que más se pide:\n");
        let mut shown = 0;
        let mut cats: Vec<&str> = Vec::new();
        for p in self
            .ctx
            .catalog
            .iter()
            .filter(|p| p.available.is_none_or(|a| a > 0))
        {
            let c = p.category.as_deref().unwrap_or("Otros");
            if cats.contains(&c) {
                continue;
            }
            cats.push(c);
            s.push_str(&format!("• {} — {}\n", p.name, money(p.price)));
            shown += 1;
            if shown == 8 {
                break;
            }
        }
        s.push_str("Escribí lo que querés y cuántos, por ejemplo: «2 Águila y 1 hielo».");
        self.text(s);
    }

    fn take_text(&mut self, raw: &str) {
        let catalog: &'c [Product] = self.ctx.catalog;
        let items = text::split_items(raw);
        let mut notes = Vec::new();
        let mut asked = false;
        for w in &items {
            match text::find(&w.words, catalog) {
                Match::Found(i) => {
                    let p = &catalog[i];
                    if let Some(note) = self.add(p, w.qty) {
                        let none_added = !self.conv.cart.iter().any(|l| l.product_id == p.id);
                        match (none_added, self.substitute(p, w.qty)) {
                            (true, Some(sub)) if !asked => {
                                asked = true;
                                self.say(buttons(
                                    format!(
                                        "No hay {}. ¿Te sirve {} a {}?",
                                        p.name,
                                        sub.name,
                                        money(sub.price)
                                    ),
                                    &[
                                        (&format!("ADD:{}:{}", sub.id, w.qty), "Sí, ese"),
                                        ("NOOP", "No, gracias"),
                                    ],
                                ));
                            }
                            _ => notes.push(note),
                        }
                    }
                }
                Match::Ambiguous(opts) if !asked => {
                    asked = true;
                    let mut body = format!("¿Cuál {} querés?", w.raw);
                    let ids: Vec<(String, String)> = opts
                        .iter()
                        .enumerate()
                        .map(|(k, i)| {
                            let p = &catalog[*i];
                            body.push_str(&format!("\n{}. {} — {}", k + 1, p.name, money(p.price)));
                            (
                                format!("ADD:{}:{}", p.id, w.qty),
                                format!("Opción {}", k + 1),
                            )
                        })
                        .collect();
                    self.say(Reply::Buttons { body, buttons: ids });
                }
                Match::Ambiguous(_) => notes.push(format!("Decime cuál «{}» querés.", w.raw)),
                Match::NotFound => notes.push(format!("No encontré «{}».", w.raw)),
            }
        }
        if !self.conv.cart.is_empty() {
            self.show_cart(notes);
        } else if !notes.is_empty() {
            notes.push(
                "Escribí el nombre como aparece en la etiqueta o tocá «Ver catálogo».".into(),
            );
            self.say(buttons(
                notes.join("\n"),
                &[("CATALOG", "Ver catálogo"), ("HUMAN", "Hablar con alguien")],
            ));
        } else if !asked {
            self.greet();
        }
    }

    fn after_confirm(&mut self) {
        if self.conv.cart.is_empty() {
            self.conv.step = Step::Idle;
            self.text("Tu pedido está vacío. Escribime qué necesitás.");
            return;
        }
        let needs_age = self.ctx.cfg.age_check
            && !self.conv.age_confirmed
            && self
                .conv
                .cart
                .iter()
                .any(|l| self.product(l.product_id).is_some_and(is_age_restricted));
        if needs_age {
            self.conv.step = Step::AgeCheck;
            self.say(buttons(
                "Como el pedido tiene licor o cigarrillos, confirmame que quien lo recibe es mayor de 18 años. Al entregar le pedimos la cédula.",
                &[("AGE_OK", "Soy mayor de 18")],
            ));
        } else {
            self.ask_address();
        }
    }

    fn ask_address(&mut self) {
        self.conv.step = Step::Address;
        match self.ctx.saved_address {
            Some((addr, _)) => self.say(buttons(
                format!("¿Te lo llevamos a la misma dirección de la otra vez?\n{addr}"),
                &[
                    ("ADDR_SAME", "Ahí mismo"),
                    ("ADDR_OTHER", "Otra dirección"),
                    ("PICKUP", "Paso a recogerlo"),
                ],
            )),
            None => self.say(buttons(
                "¿A qué dirección te lo llevamos? Escribí la calle, el número y el barrio.",
                &[("PICKUP", "Paso a recogerlo")],
            )),
        }
    }

    fn zone(&self) -> Option<&'c Zone> {
        self.conv
            .zone_id
            .and_then(|z| self.ctx.zones.iter().find(|x| x.id == z))
    }

    fn set_address(&mut self, addr: String, zone: Option<Uuid>) {
        let zone = zone.or_else(|| detect_zone(&addr, self.ctx.zones));
        self.conv.address = Some(addr);
        self.conv.pickup = false;
        self.conv.zone_id = zone;
        if zone.is_none() && !self.ctx.zones.is_empty() {
            let list: Vec<String> = self
                .ctx
                .zones
                .iter()
                .map(|z| format!("{} ({})", z.name, money(z.fee)))
                .collect();
            self.say(buttons(
                format!("No reconozco el barrio. Hacemos domicilios a: {}. Escribime la dirección con el barrio.", list.join(", ")),
                &[("PICKUP", "Paso a recogerlo")],
            ));
            return;
        }
        let sub = subtotal(&self.conv.cart);
        if sub < self.ctx.cfg.min_order {
            self.conv.step = Step::ConfirmCart;
            self.say(buttons(
                format!(
                    "El pedido mínimo para domicilio es {}. Te faltan {}: agregá algo más o pasá a recogerlo.",
                    money(self.ctx.cfg.min_order),
                    money(self.ctx.cfg.min_order - sub)
                ),
                &[("PICKUP", "Paso a recogerlo"), ("CATALOG", "Ver catálogo")],
            ));
            return;
        }
        self.ask_payment();
    }

    fn fee(&self) -> Decimal {
        if self.conv.pickup {
            Decimal::ZERO
        } else {
            self.zone().map(|z| z.fee).unwrap_or(Decimal::ZERO)
        }
    }

    fn ask_payment(&mut self) {
        self.conv.step = Step::Payment;
        let sub = subtotal(&self.conv.cart);
        let fee = self.fee();
        let mut body = cart_text(&self.conv.cart);
        if self.conv.pickup {
            body.push_str(&format!(
                "\nTotal: {}\nLo tenemos listo en unos 15 minutos para que pases por él.",
                money(sub)
            ));
        } else {
            let zone = self.zone();
            body.push_str(&format!(
                "\nDomicilio{}: {}\nTotal: {}",
                zone.map(|z| format!(" · {}", z.name)).unwrap_or_default(),
                money(fee),
                money(sub + fee)
            ));
            if let Some(z) = zone {
                body.push_str(&format!(
                    "\nLlega en {} a {} minutos.",
                    z.eta_min, z.eta_max
                ));
            }
        }
        body.push_str("\n¿Cómo vas a pagar?");
        let mut list = Vec::new();
        if self.ctx.cfg.prepay_enabled {
            list.push(("PAY_LINK", "Pagar ya (link)"));
        }
        list.push(("PAY_CASH", "Efectivo al recibir"));
        list.push(("PAY_CARD", "Datáfono al recibir"));
        self.say(buttons(body, &list));
    }

    fn place(&mut self, payment: PaymentMethod) {
        let sub = subtotal(&self.conv.cart);
        let fee = self.fee();
        self.outcome = Outcome::PlaceOrder(OrderDraft {
            lines: std::mem::take(&mut self.conv.cart),
            subtotal: sub,
            delivery_fee: fee,
            total: sub + fee,
            pickup: self.conv.pickup,
            address: if self.conv.pickup {
                None
            } else {
                self.conv.address.clone()
            },
            zone_id: if self.conv.pickup {
                None
            } else {
                self.conv.zone_id
            },
            payment,
        });
        self.conv.step = Step::Idle;
    }
}

/// Zona cuyo nombre aparece en la dirección escrita.
pub fn detect_zone(addr: &str, zones: &[Zone]) -> Option<Uuid> {
    let a = format!(" {} ", text::normalize(addr));
    zones
        .iter()
        .filter(|z| a.contains(&format!(" {} ", text::normalize(&z.name))))
        .max_by_key(|z| z.name.len())
        .map(|z| z.id)
}

/// Responde un mensaje del cliente.
pub fn handle(
    conv: &Conversation,
    input: &Input,
    ctx: &Context<'_>,
) -> (Conversation, Vec<Reply>, Outcome) {
    let mut t = Turn {
        conv: conv.clone(),
        replies: Vec::new(),
        outcome: Outcome::None,
        ctx,
    };
    let typed = match input {
        Input::Text(s) => Some(s.as_str()),
        Input::Button(_) => None,
    };
    let button = match input {
        Input::Button(id) => Some(id.as_str()),
        Input::Text(_) => None,
    };

    if t.conv.step == Step::Human {
        return (t.conv, t.replies, t.outcome);
    }
    if button == Some("HUMAN")
        || typed.is_some_and(|s| {
            text::mentions(
                s,
                &[
                    "asesor",
                    "humano",
                    "una persona",
                    "hablar con alguien",
                    "hablar con una persona",
                ],
            )
        })
    {
        t.conv.step = Step::Human;
        t.text("Listo, ya le aviso a alguien del equipo. Te escribe en unos minutos.");
        t.outcome = Outcome::Handoff;
        return (t.conv, t.replies, t.outcome);
    }
    if typed.is_some_and(|s| text::mentions(s, &["cancelar", "cancela", "cancelalo"])) {
        t.conv = Conversation {
            age_confirmed: t.conv.age_confirmed,
            address: t.conv.address.clone(),
            zone_id: t.conv.zone_id,
            ..Default::default()
        };
        t.text("Listo, cancelé el pedido. Cuando quieras me escribís.");
        return (t.conv, t.replies, t.outcome);
    }
    if !is_open(ctx.cfg, ctx.now) {
        t.text(format!(
            "Ahora estamos cerrados. Atendemos de {} a {}. Dejame tu pedido a esa hora y te lo mandamos.",
            hour(ctx.cfg.opens_at),
            hour(ctx.cfg.closes_at)
        ));
        return (t.conv, t.replies, t.outcome);
    }

    // Botones que sirven en cualquier paso.
    if let Some(b) = button {
        if b == "CATALOG" {
            t.catalog();
            return (t.conv, t.replies, t.outcome);
        }
        if b == "NOOP" {
            if t.conv.cart.is_empty() {
                t.greet()
            } else {
                t.show_cart(Vec::new())
            }
            return (t.conv, t.replies, t.outcome);
        }
        if let Some(rest) = b.strip_prefix("ADD:") {
            let mut it = rest.split(':');
            let id = it.next().and_then(|s| Uuid::parse_str(s).ok());
            let qty = it
                .next()
                .and_then(|s| s.parse::<u32>().ok())
                .unwrap_or(1)
                .clamp(1, 99);
            if let Some(p) = id.and_then(|id| t.product(id)) {
                let note = t.add(p, qty);
                t.show_cart(note.into_iter().collect());
            } else {
                t.text("Ese producto ya no está disponible. Escribime qué más necesitás.");
            }
            return (t.conv, t.replies, t.outcome);
        }
        if b == "USUAL" {
            match t.usual() {
                Some(u) => {
                    let mut notes = Vec::new();
                    for l in u {
                        match t.product(l.product_id) {
                            Some(p) => notes.extend(t.add(p, l.qty)),
                            None => notes.push(format!("{} ya no lo tenemos.", l.name)),
                        }
                    }
                    if t.conv.cart.is_empty() {
                        t.text(notes.join("\n"))
                    } else {
                        t.show_cart(notes)
                    }
                }
                None => t.greet(),
            }
            return (t.conv, t.replies, t.outcome);
        }
        if b == "PICKUP"
            && matches!(
                t.conv.step,
                Step::Address | Step::ConfirmCart | Step::Payment
            )
            && !t.conv.cart.is_empty()
        {
            t.conv.pickup = true;
            t.ask_payment();
            return (t.conv, t.replies, t.outcome);
        }
    }

    match t.conv.step {
        Step::Idle | Step::ConfirmCart | Step::Human => match (button, typed) {
            (Some("CONFIRM"), _) if t.conv.step == Step::ConfirmCart => t.after_confirm(),
            (Some("CHANGE"), _) => {
                t.conv.cart.clear();
                t.conv.step = Step::Idle;
                t.text("Dale. Escribime el pedido como lo querés y lo armo de nuevo.");
            }
            (_, Some(s))
                if text::mentions(
                    s,
                    &[
                        "lo de siempre",
                        "lo mismo de siempre",
                        "lo mismo de la otra vez",
                    ],
                ) =>
            {
                return handle(&t.conv, &Input::Button("USUAL".into()), ctx);
            }
            (_, Some(s))
                if text::mentions(
                    s,
                    &[
                        "catalogo",
                        "menu",
                        "precios",
                        "lista",
                        "que tienen",
                        "que hay",
                    ],
                ) =>
            {
                t.catalog()
            }
            (_, Some(s))
                if t.conv.step == Step::ConfirmCart
                    && text::mentions(s, &["si", "listo", "confirmo", "dale", "de una", "ok"])
                    && text::split_items(s).is_empty() =>
            {
                t.after_confirm()
            }
            (_, Some(s)) if text::is_greeting(s) => {
                if t.conv.cart.is_empty() {
                    t.greet()
                } else {
                    t.show_cart(Vec::new())
                }
            }
            (_, Some(s)) => t.take_text(s),
            _ => {
                if t.conv.cart.is_empty() {
                    t.greet()
                } else {
                    t.show_cart(Vec::new())
                }
            }
        },
        Step::AgeCheck => {
            let ok = button == Some("AGE_OK")
                || typed.is_some_and(|s| {
                    text::mentions(
                        s,
                        &["si", "soy mayor", "mayor de edad", "mayor de 18", "claro"],
                    )
                });
            if ok {
                t.conv.age_confirmed = true;
                t.ask_address();
            } else {
                t.say(buttons(
                    "Para seguir necesito que confirmes que quien recibe es mayor de 18 años.",
                    &[("AGE_OK", "Soy mayor de 18"), ("CHANGE", "Cambiar algo")],
                ));
            }
        }
        Step::Address => match (button, typed) {
            (Some("ADDR_SAME"), _) if ctx.saved_address.is_some() => {
                let (a, z) = ctx.saved_address.unwrap_or_default();
                t.set_address(a.to_string(), z);
            }
            (Some("ADDR_OTHER"), _) => t.text("Escribime la dirección completa con el barrio."),
            (Some("CHANGE"), _) => {
                t.conv.cart.clear();
                t.conv.step = Step::Idle;
                t.text("Dale. Escribime el pedido como lo querés y lo armo de nuevo.");
            }
            (_, Some(s))
                if text::mentions(
                    s,
                    &["recoger", "recojo", "paso por", "voy por", "paso a recoger"],
                ) =>
            {
                t.conv.pickup = true;
                t.ask_payment();
            }
            (_, Some(s)) if s.trim().chars().count() >= 6 => {
                t.set_address(s.trim().to_string(), None)
            }
            _ => t.ask_address(),
        },
        Step::Payment => match (button, typed) {
            (Some("PAY_LINK"), _) if ctx.cfg.prepay_enabled => t.place(PaymentMethod::Link),
            (Some("PAY_CASH"), _) => t.place(PaymentMethod::Efectivo),
            (Some("PAY_CARD"), _) => t.place(PaymentMethod::Datafono),
            (_, Some(s)) if text::mentions(s, &["efectivo", "cash", "billete"]) => {
                t.place(PaymentMethod::Efectivo)
            }
            (_, Some(s)) if text::mentions(s, &["datafono", "tarjeta"]) => {
                t.place(PaymentMethod::Datafono)
            }
            (_, Some(s))
                if ctx.cfg.prepay_enabled
                    && text::mentions(s, &["nequi", "link", "pse", "transferencia"]) =>
            {
                t.place(PaymentMethod::Link)
            }
            _ => t.ask_payment(),
        },
    }
    (t.conv, t.replies, t.outcome)
}

// ------------------------------------------------------------- avisos al cliente

/// Mensaje que confirma un pedido recién registrado.
pub fn confirmation_text(code: &str, d: &OrderDraft) -> String {
    let base = format!("Tu pedido {code} quedó registrado por {}.", money(d.total));
    match (d.payment, d.pickup) {
        (PaymentMethod::Link, _) => format!(
            "{base} En un momento te mando el link de pago; apenas se pague lo empezamos a alistar."
        ),
        (_, true) => format!("{base} Te aviso cuando esté listo para recoger."),
        (PaymentMethod::Efectivo, false) => {
            format!("{base} Pagás en efectivo al recibir. Te aviso cuando salga.")
        }
        (PaymentMethod::Datafono, false) => {
            format!("{base} El domiciliario lleva datáfono. Te aviso cuando salga.")
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum OrderStatus {
    Nuevo,
    Preparando,
    EnCamino,
    Entregado,
    Cancelado,
}

impl OrderStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            OrderStatus::Nuevo => "NUEVO",
            OrderStatus::Preparando => "PREPARANDO",
            OrderStatus::EnCamino => "EN_CAMINO",
            OrderStatus::Entregado => "ENTREGADO",
            OrderStatus::Cancelado => "CANCELADO",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        Some(match s {
            "NUEVO" => OrderStatus::Nuevo,
            "PREPARANDO" => OrderStatus::Preparando,
            "EN_CAMINO" => OrderStatus::EnCamino,
            "ENTREGADO" => OrderStatus::Entregado,
            "CANCELADO" => OrderStatus::Cancelado,
            _ => return None,
        })
    }

    /// Avanza un paso a la vez; se puede cancelar mientras no se haya entregado.
    pub fn can_go_to(self, to: OrderStatus) -> bool {
        use OrderStatus::*;
        matches!(
            (self, to),
            (Nuevo, Preparando)
                | (Preparando, EnCamino)
                | (EnCamino, Entregado)
                | (Nuevo | Preparando | EnCamino, Cancelado)
        )
    }
}

/// Aviso al cliente cuando el pedido cambia de estado.
pub fn status_text(
    code: &str,
    to: OrderStatus,
    pickup: bool,
    courier: Option<&str>,
    business: &str,
) -> String {
    match to {
        OrderStatus::Nuevo => format!("Recibimos tu pedido {code}."),
        OrderStatus::Preparando => format!("Ya estamos alistando tu pedido {code}."),
        OrderStatus::EnCamino if pickup => {
            format!("Tu pedido {code} está listo. Podés pasar a recogerlo.")
        }
        OrderStatus::EnCamino => match courier {
            Some(c) => format!("Tu pedido {code} va en camino con {c}."),
            None => format!("Tu pedido {code} va en camino."),
        },
        OrderStatus::Entregado => {
            format!("Pedido {code} entregado. ¡Gracias por comprar en {business}!")
        }
        OrderStatus::Cancelado => {
            format!("Tu pedido {code} fue cancelado. Si es un error, escribinos y lo revisamos.")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal::Decimal;

    fn d(n: i64) -> Decimal {
        Decimal::from(n)
    }

    struct Fx {
        cfg: BotConfig,
        catalog: Vec<Product>,
        zones: Vec<Zone>,
        usual: Vec<CartLine>,
    }

    fn fx() -> Fx {
        let prod = |name: &str, cat: &str, price: i64, avail: Option<i64>| Product {
            id: Uuid::new_v4(),
            name: name.into(),
            category: Some(cat.into()),
            price: d(price),
            available: avail,
        };
        let catalog = vec![
            prod("Whisky Old Parr 12 años 750 ml", "Whisky", 165000, Some(4)),
            prod(
                "Whisky Buchanan's 12 años 750 ml",
                "Whisky",
                170000,
                Some(6),
            ),
            prod(
                "Aguardiente Antioqueño 750 ml",
                "Aguardiente",
                62000,
                Some(30),
            ),
            prod("Hielo bolsa 2 kg", "Hielo", 4500, Some(40)),
            prod("Coca-Cola 1,5 L", "Gaseosas", 6500, None),
            prod("Cerveza Águila lata 330 ml", "Cerveza", 3200, Some(200)),
        ];
        let usual = vec![
            CartLine {
                product_id: catalog[2].id,
                name: catalog[2].name.clone(),
                qty: 1,
                unit_price: d(62000),
            },
            CartLine {
                product_id: catalog[3].id,
                name: catalog[3].name.clone(),
                qty: 2,
                unit_price: d(4500),
            },
        ];
        Fx {
            cfg: BotConfig {
                business_name: "Licorera La 15".into(),
                opens_at: NaiveTime::from_hms_opt(10, 0, 0).unwrap(),
                closes_at: NaiveTime::from_hms_opt(1, 0, 0).unwrap(),
                min_order: d(30000),
                prepay_enabled: true,
                usual_enabled: true,
                substitutes_enabled: true,
                age_check: true,
            },
            catalog,
            zones: vec![Zone {
                id: Uuid::new_v4(),
                name: "Centro".into(),
                fee: d(4000),
                eta_min: 20,
                eta_max: 30,
            }],
            usual,
        }
    }

    fn ctx<'a>(f: &'a Fx, saved: Option<(&'a str, Option<Uuid>)>) -> Context<'a> {
        Context {
            cfg: &f.cfg,
            catalog: &f.catalog,
            zones: &f.zones,
            customer_name: Some("Ramiro Epieyú"),
            usual: Some(&f.usual),
            saved_address: saved,
            now: NaiveTime::from_hms_opt(20, 12, 0).unwrap(),
        }
    }

    fn txt(s: &str) -> Input {
        Input::Text(s.into())
    }
    fn btn(s: &str) -> Input {
        Input::Button(s.into())
    }

    fn body(r: &[Reply]) -> String {
        r.iter()
            .map(|r| match r {
                Reply::Text(s) => s.clone(),
                Reply::Buttons { body, .. } => body.clone(),
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    #[test]
    fn full_order_with_usual_delivery_and_link() {
        let f = fx();
        let c = ctx(&f, Some(("Calle 15 #8-20, Centro", Some(f.zones[0].id))));
        let s = Conversation::default();

        let (s, r, _) = handle(&s, &txt("Buenas, ¿tienen Old Parr?"), &c);
        assert_eq!(s.step, Step::ConfirmCart);
        assert!(body(&r).contains("Old Parr"));

        let (s, r, _) = handle(&s, &txt("y mandame lo de siempre"), &c);
        assert_eq!(s.cart.len(), 3);
        assert!(body(&r).contains("Productos: $236.000"), "{}", body(&r));

        let (s, r, _) = handle(&s, &btn("CONFIRM"), &c);
        assert_eq!(s.step, Step::AgeCheck);
        assert!(body(&r).contains("mayor de 18"));

        let (s, _, _) = handle(&s, &btn("AGE_OK"), &c);
        assert_eq!(s.step, Step::Address);

        let (s, r, _) = handle(&s, &btn("ADDR_SAME"), &c);
        assert_eq!(s.step, Step::Payment);
        let b = body(&r);
        assert!(
            b.contains("Domicilio · Centro: $4.000")
                && b.contains("Total: $240.000")
                && b.contains("20 a 30"),
            "{b}"
        );

        let (s, _, out) = handle(&s, &btn("PAY_LINK"), &c);
        let Outcome::PlaceOrder(o) = out else {
            panic!("no order")
        };
        assert_eq!(
            (o.total, o.delivery_fee, o.payment, o.pickup),
            (d(240000), d(4000), PaymentMethod::Link, false)
        );
        assert_eq!(s.step, Step::Idle);
        assert!(s.cart.is_empty() && s.age_confirmed);
        assert!(confirmation_text("W-0142", &o).contains("link de pago"));
    }

    #[test]
    fn greeting_offers_usual_and_buttons_fit_whatsapp() {
        let f = fx();
        let c = ctx(&f, None);
        let (_, r, _) = handle(&Conversation::default(), &txt("Hola buenas noches"), &c);
        let Reply::Buttons { body, buttons } = &r[0] else {
            panic!()
        };
        assert!(body.starts_with("¡Hola, Ramiro!"));
        assert_eq!(buttons[0].0, "USUAL");
        for (_, t) in buttons {
            assert!(t.chars().count() <= MAX_BUTTON_TITLE, "{t}");
        }
    }

    #[test]
    fn caps_to_stock_and_offers_substitute() {
        let mut f = fx();
        let c = ctx(&f, None);
        let (s, r, _) = handle(&Conversation::default(), &txt("6 old parr"), &c);
        assert_eq!(s.cart[0].qty, 4);
        assert!(body(&r).contains("Solo quedan 4"));

        f.catalog[0].available = Some(0);
        let c = ctx(&f, None);
        let (s, r, _) = handle(&Conversation::default(), &txt("1 old parr"), &c);
        assert!(s.cart.is_empty());
        let Reply::Buttons { body, buttons } = &r[0] else {
            panic!()
        };
        assert!(body.contains("Buchanan"));
        let (s, _, _) = handle(&s, &btn(&buttons[0].0), &c);
        assert_eq!(s.cart[0].name, "Whisky Buchanan's 12 años 750 ml");
    }

    #[test]
    fn minimum_order_and_pickup() {
        let f = fx();
        let c = ctx(&f, None);
        let (s, _, _) = handle(&Conversation::default(), &txt("2 cocas"), &c);
        let (s, r, _) = handle(&s, &btn("CONFIRM"), &c);
        assert_eq!(s.step, Step::Address, "{}", body(&r)); // sin licor no pide edad
        let (s, r, _) = handle(&s, &txt("Calle 15 #8-20 Centro"), &c);
        assert!(body(&r).contains("pedido mínimo"));
        let (s, _, _) = handle(&s, &btn("PICKUP"), &c);
        assert_eq!(s.step, Step::Payment);
        let (_, _, out) = handle(&s, &txt("efectivo"), &c);
        let Outcome::PlaceOrder(o) = out else {
            panic!()
        };
        assert!(o.pickup && o.delivery_fee.is_zero() && o.total == d(13000));
    }

    #[test]
    fn unknown_zone_is_asked_again() {
        let f = fx();
        let c = ctx(&f, None);
        let s = Conversation {
            step: Step::Address,
            cart: f.usual.clone(),
            age_confirmed: true,
            ..Default::default()
        };
        let (s, r, _) = handle(&s, &txt("Carrera 7 # 40-10, El Prado"), &c);
        assert_eq!(s.step, Step::Address);
        assert!(body(&r).contains("Centro ($4.000)"));
    }

    #[test]
    fn closed_hours_and_handoff() {
        let f = fx();
        let mut c = ctx(&f, None);
        c.now = NaiveTime::from_hms_opt(3, 0, 0).unwrap();
        let (_, r, _) = handle(&Conversation::default(), &txt("2 aguilas"), &c);
        assert!(body(&r).contains("cerrados") && body(&r).contains("10 a. m. a 1 a. m."));

        let c = ctx(&f, None);
        let (s, _, out) = handle(
            &Conversation::default(),
            &txt("quiero hablar con un asesor"),
            &c,
        );
        assert_eq!((s.step, out), (Step::Human, Outcome::Handoff));
        let (_, r, _) = handle(&s, &txt("hola?"), &c);
        assert!(r.is_empty());
    }

    #[test]
    fn money_and_status_transitions() {
        assert_eq!(money(d(245500)), "$245.500");
        assert_eq!(money(d(4000)), "$4.000");
        assert_eq!(money(d(500)), "$500");
        use OrderStatus::*;
        assert!(
            Nuevo.can_go_to(Preparando)
                && EnCamino.can_go_to(Entregado)
                && Preparando.can_go_to(Cancelado)
        );
        assert!(!Nuevo.can_go_to(Entregado) && !Entregado.can_go_to(Cancelado));
        assert!(status_text("W-1", EnCamino, false, Some("Jhon"), "X").contains("con Jhon"));
    }
}
