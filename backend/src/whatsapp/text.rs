//! Lectura de pedidos escritos en español de Colombia: normaliza el texto,
//! separa los ítems, entiende cantidades y busca el producto en el catálogo.

use uuid::Uuid;

/// Producto vendible tal como lo ve el bot (precio ya con impuesto).
#[derive(Debug, Clone)]
pub struct Product {
    pub id: Uuid,
    pub name: String,
    pub category: Option<String>,
    pub price: rust_decimal::Decimal,
    /// Unidades disponibles en la sede; `None` si no se controla existencia.
    pub available: Option<i64>,
}

/// Minúsculas, sin tildes ni signos, con espacios simples.
pub fn normalize(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars().flat_map(char::to_lowercase) {
        let c = match ch {
            'á' | 'à' | 'ä' | 'â' => 'a',
            'é' | 'è' | 'ë' | 'ê' => 'e',
            'í' | 'ì' | 'ï' | 'î' => 'i',
            'ó' | 'ò' | 'ö' | 'ô' => 'o',
            'ú' | 'ù' | 'ü' | 'û' => 'u',
            'ñ' => 'n',
            c if c.is_alphanumeric() => c,
            _ => ' ',
        };
        out.push(c);
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Palabras que no aportan al nombre del producto.
const FILLER: &[&str] = &[
    "quiero", "quisiera", "dame", "deme", "regalame", "regaleme", "mandame", "mandeme", "manda",
    "traeme", "traigame", "necesito", "tienen", "tiene", "tienes", "tenes", "hay", "porfa",
    "favor", "por", "porfavor", "buenas", "buenos", "buena", "hola", "dias", "tardes", "noches",
    "me", "de", "del", "la", "el", "los", "las", "para", "ya", "ok", "tambien", "pues", "y", "a",
    "al", "vecino", "vecina", "señor", "senor", "sumerce", "parce", "mas", "con", "que", "si",
    "no", "otra", "otro",
];

fn number_word(t: &str) -> Option<u32> {
    Some(match t {
        "un" | "una" | "uno" => 1,
        "dos" | "par" => 2,
        "tres" => 3,
        "cuatro" => 4,
        "cinco" => 5,
        "seis" => 6,
        "siete" => 7,
        "ocho" => 8,
        "nueve" => 9,
        "diez" => 10,
        "once" => 11,
        "doce" | "docena" => 12,
        _ => return None,
    })
}

/// Cantidad escrita como `2`, `x2`, `2x` o con palabras (hasta 99).
fn quantity(t: &str) -> Option<u32> {
    let digits = t.trim_start_matches('x').trim_end_matches('x');
    if !digits.is_empty() && digits.len() <= 2 && digits.chars().all(|c| c.is_ascii_digit()) {
        return digits.parse().ok().filter(|n| *n > 0);
    }
    number_word(t)
}

/// Plural sencillo: `hielos` → `hielo`, `aguilas` → `aguila`.
fn singular(t: &str) -> String {
    if t.len() > 4 && t.ends_with("es") && !t.ends_with("ses") {
        let base = &t[..t.len() - 2];
        // "limones" → "limon", pero "cervezas" no termina en "es".
        if base.ends_with(['n', 'r', 'l', 'd']) {
            return base.to_string();
        }
    }
    if t.len() > 3 && t.ends_with('s') {
        return t[..t.len() - 1].to_string();
    }
    t.to_string()
}

/// Un ítem pedido: cantidad y palabras con las que se busca el producto.
#[derive(Debug, Clone, PartialEq)]
pub struct Wanted {
    pub qty: u32,
    pub words: Vec<String>,
    /// El texto del ítem como lo escribió el cliente (para repreguntar).
    pub raw: String,
}

/// Separa un mensaje en ítems: "1 old parr y 2 hielos, una coca".
pub fn split_items(text: &str) -> Vec<Wanted> {
    let mut out = Vec::new();
    for chunk in text.split([',', ';', '\n', '+']) {
        let norm = normalize(chunk);
        // " y " separa ítems salvo dentro de un nombre; lo tratamos como separador.
        for part in norm
            .split(" y ")
            .flat_map(|p| p.split(" mas "))
            .flat_map(|p| p.split(" con "))
        {
            let tokens: Vec<&str> = part.split_whitespace().collect();
            let mut qty: Option<u32> = None;
            let mut words: Vec<String> = Vec::new();
            let mut start = 0usize;
            for (i, t) in tokens.iter().enumerate() {
                let prev = if i > 0 { Some(tokens[i - 1]) } else { None };
                if *t == "media" && tokens.get(i + 1) == Some(&"docena") {
                    if !words.is_empty() {
                        out.push(Wanted {
                            qty: qty.unwrap_or(1),
                            words: std::mem::take(&mut words),
                            raw: tokens[start..i].join(" "),
                        });
                        start = i;
                    }
                    qty = Some(6);
                    continue;
                }
                if *t == "docena" && prev == Some("media") {
                    continue;
                }
                // Una cantidad en palabras a mitad de frase empieza otro ítem:
                // "2 hielos una coca". Los números en cifras no, porque son
                // parte del nombre ("Old Parr 12 años").
                if !words.is_empty() && number_word(t).is_some() {
                    out.push(Wanted {
                        qty: qty.unwrap_or(1),
                        words: std::mem::take(&mut words),
                        raw: tokens[start..i].join(" "),
                    });
                    start = i;
                    qty = number_word(t);
                    continue;
                }
                if *t == "par" && words.is_empty() && qty == Some(1) {
                    qty = Some(2); // "un par de"
                    continue;
                }
                if qty.is_none()
                    && words.is_empty()
                    && let Some(n) = quantity(t)
                {
                    qty = Some(n);
                    continue;
                }
                if FILLER.contains(t) {
                    continue;
                }
                words.push(singular(t));
            }
            if !words.is_empty() {
                out.push(Wanted {
                    qty: qty.unwrap_or(1),
                    words,
                    raw: tokens[start..].join(" "),
                });
            }
        }
    }
    out
}

/// Resultado de buscar un ítem en el catálogo.
#[derive(Debug, Clone, PartialEq)]
pub enum Match {
    Found(usize),
    /// Varias opciones igual de buenas (índices en el catálogo, máx. 3).
    Ambiguous(Vec<usize>),
    NotFound,
}

fn product_tokens(name: &str) -> Vec<String> {
    normalize(name).split_whitespace().map(singular).collect()
}

/// Busca las palabras en el catálogo. Un producto suma un punto por cada
/// palabra del cliente que aparece en su nombre (o es el comienzo de una
/// palabra del nombre, con al menos 3 letras).
pub fn find(words: &[String], catalog: &[Product]) -> Match {
    let mut best: Vec<(usize, usize, usize)> = Vec::new(); // (idx, score, extra)
    let mut best_score = 0;
    for (i, p) in catalog.iter().enumerate() {
        let mut toks = product_tokens(&p.name);
        if let Some(c) = &p.category {
            toks.extend(product_tokens(c));
        }
        let score = words
            .iter()
            .filter(|w| {
                toks.iter()
                    .any(|t| t == *w || (w.len() >= 3 && t.starts_with(w.as_str())))
            })
            .count();
        if score == 0 {
            continue;
        }
        let extra = product_tokens(&p.name).len().saturating_sub(score);
        match score.cmp(&best_score) {
            std::cmp::Ordering::Greater => {
                best_score = score;
                best = vec![(i, score, extra)];
            }
            std::cmp::Ordering::Equal => best.push((i, score, extra)),
            std::cmp::Ordering::Less => {}
        }
    }
    // Hay que reconocer la mayoría de las palabras del cliente.
    let needed = words.len().div_ceil(2).max(1);
    if best_score < needed || best.is_empty() {
        return Match::NotFound;
    }
    best.sort_by_key(|(i, _, extra)| (*extra, catalog[*i].name.len()));
    let min_extra = best[0].2;
    let tied: Vec<usize> = best
        .iter()
        .filter(|b| b.2 == min_extra)
        .map(|b| b.0)
        .collect();
    if tied.len() == 1 {
        Match::Found(tied[0])
    } else {
        Match::Ambiguous(tied.into_iter().take(3).collect())
    }
}

/// ¿El texto contiene alguna de estas frases (ya normalizadas)?
pub fn mentions(text: &str, phrases: &[&str]) -> bool {
    let n = format!(" {} ", normalize(text));
    phrases.iter().any(|p| n.contains(&format!(" {p} ")))
}

/// Mensaje que es solo un saludo o relleno, sin pedido.
pub fn is_greeting(text: &str) -> bool {
    let n = normalize(text);
    !n.is_empty()
        && n.split_whitespace().all(|t| {
            FILLER.contains(&t)
                || matches!(
                    t,
                    "buen" | "saludos" | "hey" | "holi" | "ola" | "como" | "estan" | "esta" | "va"
                )
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal::Decimal;

    fn p(name: &str, cat: &str) -> Product {
        Product {
            id: Uuid::new_v4(),
            name: name.into(),
            category: Some(cat.into()),
            price: Decimal::from(1000),
            available: Some(10),
        }
    }

    fn catalog() -> Vec<Product> {
        vec![
            p("Whisky Old Parr 12 años 750 ml", "Whisky"),
            p("Aguardiente Antioqueño 750 ml", "Aguardiente"),
            p("Aguardiente Antioqueño sin azúcar 375 ml", "Aguardiente"),
            p("Hielo bolsa 2 kg", "Hielo"),
            p("Cerveza Águila lata 330 ml", "Cerveza"),
            p("Club Colombia Dorada 330 ml", "Cerveza"),
            p("Coca-Cola 1,5 L", "Gaseosas"),
        ]
    }

    #[test]
    fn normalizes_accents_and_signs() {
        assert_eq!(
            normalize("¿Tienen Águila, AGUARDIENTE?"),
            "tienen aguila aguardiente"
        );
    }

    #[test]
    fn splits_items_with_quantities() {
        let w = split_items("Buenas, ¿tienen 1 old parr y 2 hielos? una coca");
        assert_eq!(w.len(), 3);
        assert_eq!(
            (w[0].qty, w[0].words.clone()),
            (1, vec!["old".to_string(), "parr".into()])
        );
        assert_eq!(
            (w[1].qty, w[1].words.clone()),
            (2, vec!["hielo".to_string()])
        );
        assert_eq!(
            (w[2].qty, w[2].words.clone()),
            (1, vec!["coca".to_string()])
        );
    }

    #[test]
    fn understands_number_words_and_x() {
        let w = split_items("dos aguilas, x3 hielo, media docena de club colombia");
        assert_eq!(w.iter().map(|w| w.qty).collect::<Vec<_>>(), vec![2, 3, 6]);
    }

    #[test]
    fn finds_products() {
        let c = catalog();
        let get = |s: &str| find(&split_items(s)[0].words, &c);
        assert_eq!(get("old parr"), Match::Found(0));
        assert_eq!(get("2 hielos"), Match::Found(3));
        assert_eq!(get("una coca"), Match::Found(6));
        assert_eq!(get("aguilas"), Match::Found(4));
        assert_eq!(get("club colombia"), Match::Found(5));
        // "aguardiente" a secas: gana el nombre más corto (el de 750).
        assert_eq!(get("aguardiente"), Match::Found(1));
        assert_eq!(get("tequila"), Match::NotFound);
    }

    #[test]
    fn detects_greeting_and_phrases() {
        assert!(is_greeting("Hola buenas noches"));
        assert!(!is_greeting("hola, 2 aguilas"));
        assert!(mentions("Mandame lo de siempre porfa", &["lo de siempre"]));
        assert!(!mentions("siempre", &["lo de siempre"]));
    }
}
