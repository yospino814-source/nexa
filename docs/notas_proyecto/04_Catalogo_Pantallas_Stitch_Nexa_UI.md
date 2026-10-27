# Catálogo Maestro de Pantallas Nexa POS - Stitch UI

- **ID Proyecto Stitch**: `11495705857039807858` (`projects/11495705857039807858`)
- **Design System Vinculado**: `assets/16189402570785253940` (*Nexa POS Design System*)
- **Paleta Oficial**:
  - Primario: Azul Petróleo `#0F6E6E`
  - Fondos: `slate-50` (páginas), `white` (tarjetas `rounded-2xl border border-slate-200`)
  - Navegación Admin: `slate-900` (sidebar colapsado 72px)
  - Semántica: `emerald-500` (éxito/activo), `amber-500` (advertencia/alerta), `rose-500` (agotado/peligro)
- **Moneda**: Pesos Colombianos COP (`$XX.XXX`, sin decimales, separador de miles con punto).
- **Tipografía**: `Inter`.

---

## 1. Módulo Operación: POS Mostrador & Caja
| # | Archivo Fuente | Nombre de Pantalla | Layout | Screen ID en Stitch |
|---|----------------|--------------------|--------|---------------------|
| 1 | `13-pos-mostrador.md` | POS Mostrador Táctil (Split 60/40) | Operación | `bf3a616e01624c509592f905aa9cf841` |
| 2 | `14-cobro-y-comprobante.md` | Modal de Pago Rápido y Factura | Operación | `9af55854cb6140a7a9a0319b027e9e30` |
| 3 | `17-caja.md` | Caja y Arqueo Ciego COP | Operación | `aeb19efd716e4001b05036bf2d905e84` |

---

## 2. Módulo Gastronomía & Restaurante
| # | Archivo Fuente | Nombre de Pantalla | Layout | Screen ID en Stitch |
|---|----------------|--------------------|--------|---------------------|
| 4 | `21-mapa-de-mesas.md` | Mapa de Salón y Estado de Mesas | Operación | `4555f0b43f944065ad89800da1abc327` |
| 5 | `22-comanda.md` | Comanda Táctil para Meseros (Split 60/40) | Operación | `199f069b7b0148e4892b91d930317923` |
| 6 | `23-cocina-kds.md` | Monitor KDS de Cocina | Operación | `6a44a2fcffdf4586882ffe28aafe999e` |
| 7 | `24-cuenta-de-mesa.md` | Cuenta de Mesa y División de Pagos | Operación | `ad36bd1f3c75436cbc06a9d3ff6400dc` |
| 8 | `25-recetas-e-insumos.md` | Recetas y Escandallo de Insumos | Administración | `7c05d5b6e681405b9621cf41eef8a6aa` |

---

## 3. Módulo Inventario, Catálogo & Compras Inteligentes
| # | Archivo Fuente | Nombre de Pantalla | Layout | Screen ID en Stitch |
|---|----------------|--------------------|--------|---------------------|
| 9 | `05-stock-info.md` | Stock Info y Control de Existencias | Administración | `332d36329fdb4410b2d94814428360b9` |
| 10 | `08-compras.md` | Compras a Proveedores y Scan IA | Administración | `b8522a32c7b64b7e9f0403cb95f6396f` |
| 11 | `15-ficha-de-producto.md` | Ficha de Producto y Rentabilidad | Administración | `811e4d0588cf415daf771af6b543c894` |
| 12 | `16-movimientos-de-inventario.md` | Movimientos Kardex y Ajustes de Stock | Administración | `57bb64f6741544609298425cf9098874` |

---

## 4. Módulo Ventas, Reportes & Finanzas
| # | Archivo Fuente | Nombre de Pantalla | Layout | Screen ID en Stitch |
|---|----------------|--------------------|--------|---------------------|
| 13 | `01-panel-principal.md` | Dashboard y Panel Principal | Administración | `538a77bf17ac4fc480c7803ef9717b51` |
| 14 | `02-resumen-de-ventas.md` | Resumen Analítico de Ventas y Canales | Administración | `595431320fe84f79905127790736564e` |
| 15 | `03-ventas-por-producto.md` | Rendimiento y Margen por Producto | Administración | `ea19973d5b4940e0bf3b4e747add898c` |
| 16 | `04-ordenes.md` | Gestión Omnicanal de Órdenes | Administración | `a6bf701756b34d87a7e5df9e5abe9cb6` |
| 17 | `06-gastos.md` | Control de Gastos y Egresos Operativos | Administración | `989fd010d1c14ed49daff13ba4ea23e9` |

---

## 5. Módulo Clientes, Fidelización & Preventa Mayorista (B2B)
| # | Archivo Fuente | Nombre de Pantalla | Layout | Screen ID en Stitch |
|---|----------------|--------------------|--------|---------------------|
| 18 | `07-clientes.md` | Directorio de Clientes y Cartera / Fiado | Administración | `9b0b360e96544d51a2600488ed1f4f52` |
| 19 | `26-rutas.md` | Rutas de Despacho y Logística Mayorista | Administración | `c3f74ea4c1d04500bdd893d6c7f82084` |
| 20 | `27-pedido-mayorista.md` | Pedido Mayorista B2B (Asistente 3 Pasos) | Administración / Tablet | `e1e7c38f50e444df98501083ec26d9d4` |
| 21 | `28-app-del-repartidor.md` | App Móvil del Repartidor en Ruta | Móvil / Operación | `c0495aedd04d4039a94a159116380dcd` |

---

## 6. Módulo E-commerce & Tienda Pública Mobile-First
| # | Archivo Fuente | Nombre de Pantalla | Layout | Screen ID en Stitch |
|---|----------------|--------------------|--------|---------------------|
| 22 | `29-catalogo-publico.md` | Catálogo Web Público de la Tienda | Público Mobile-First | `4b55c89b860445189edbb9ed0c209b41` |
| 23 | `30-carrito-y-checkout.md` | Carrito y Checkout Web / WhatsApp | Público Mobile-First | `ee29d501dd57443b8f1d79391a9d1f25` |
| 24 | `31-seguimiento-del-pedido.md` | Tracking y Seguimiento en Tiempo Real | Público Mobile-First | `b868b984d7f24922a33c58ae17c1a0bb` |

---

## 7. Módulo Autenticación, Configuración & Ajustes Fiscales
| # | Archivo Fuente | Nombre de Pantalla | Layout | Screen ID en Stitch |
|---|----------------|--------------------|--------|---------------------|
| 25 | `09-crear-cuenta.md` | Registro de Organización | Público | `7da79e2ae4b4490e934efccb0d5d2d3f` |
| 26 | `11-iniciar-sesion.md` | Iniciar Sesión y Selección de Sucursal | Público | `6eb72e52392c4ac09c44b12385b8ad39` |
| 27 | `12-configuracion-inicial.md` | Onboarding Inicial (3 Pasos) | Público | `f7fd6515e4ed4f748db8a10bb548e75e` |
| 28 | `18-negocio-y-sucursales.md` | Configuración de Negocio y Sucursales | Administración | `9a8ae899bbea4776a5db61c2288e0a91` |
| 29 | `19-usuarios-y-roles.md` | Usuarios, Roles y Permisos de Acceso | Administración | `28e860c6b5b34fa9806115d2a91329db` |
| 30 | `20-pagos-impuestos-y-facturacion.md` | Pagos, Impuestos (IVA/INC) y DIAN | Administración | `605aac5bd3354999b12a5863c92f12ed` |
