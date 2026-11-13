# Proyecto Nexa POS - Centro de Conocimiento

Este directorio funciona como un vault de Obsidian para mantener el contexto del desarrollo del backend del POS Omnicanal.

## Estado Actual
- **Objetivo**: Desarrollar un sistema POS unificado para Tiendas de Víveres, Restaurantes y Bodegas con sistema de envíos.
- **Backend**: Rust + Supabase (Oracle DB concept para el proyecto universitario, pero implementado en Supabase/PostgreSQL).
- **Diseño DB**: Se ha propuesto un esquema unificado (`modelo_omnicanal.sql`) que abarca Retail, Hospitalidad y Logística.

## Siguientes Pasos
1. Validar el esquema SQL unificado con las migraciones actuales.
2. Configurar CodeGraph para mantener el contexto del código Rust.
3. Iniciar la implementación de los endpoints core (Catálogo e Inventario).

## Enlaces Rápidos
- [[01_arquitectura_db]]
- [[02_flujos_negocio]]
- [[03_Benchmarking_y_Modulo_Compras_Inteligentes]]
- [[04_Catalogo_Pantallas_Stitch_Nexa_UI]]
