# M18 Correcciones del modo demo y datos de licorería

| Dato | Valor |
|---|---|
| Fase | Salida |
| Repositorio | `nexa/` (solo la carpeta `frontend/`) |
| Rama | La rama actual de `nexa/` (`git branch --show-current`) |
| Commits | Uno por tarea: `fix(salida): M18.N <resumen>` |
| Migración | Ninguna |
| Backend | No se toca |
| Reporte | Sección "M18" al final de `antigravity/REPORTE-FASE-S.md` |
| Origen | Defectos D1 a D5 de `antigravity/INFORME-SALIDA.md` §2 |


## Estado actual (2026-11-20): corregir antes de seguir

Una primera pasada de M18 ya está en `frontend/` **sin commitear**, y tiene tres defectos. Se corrigen antes de cualquier otro paso:

1. **Texto dañado (mojibake).** Los archivos quedaron con la codificación rota ("tenés" salió "tenÃ©s", "Medellín" salió "MedellÃ­n") y con saltos CRLF en vez de LF. Guardá primero `git diff > ../respaldos/m18-con-mojibake.patch`. Después, por cada archivo tocado, `git checkout HEAD -- <archivo>` y reaplicá solo los cambios de M18 en UTF-8 sin BOM y LF, con la herramienta de edición o con Node (`fs.readFileSync(p,"utf8")` / `fs.writeFileSync(p,s,"utf8")`). Prohibido `Get-Content`, `Set-Content` y `Out-File`. Comprobá: `grep -rn "Ã\|Â" lib components app` sin resultados.
2. **`export const instant = false;`** se quitó de `frontend/app/(operacion)/layout.tsx`. Se restaura: lo añadió el usuario el 18 de noviembre (commit `70f095d`) en los tres layouts privados.
3. **`frontend/app/(admin)/modulos/page.tsx`** recibió un `notFound()` en producción que M18 no pide. Se revierte con `git checkout HEAD -- "app/(admin)/modulos/page.tsx"`. Si te parece buena idea, anotala en "Preguntas abiertas".

## Prompt para Antigravity

```text
Vas a hacer el módulo M18 de Nexa POS. Vos (modelo 3.8) coordinás; el trabajo lo hacen subagentes.

0. Abrí la terminal en la raíz de nexa/ y corré `git rev-parse --show-toplevel` y
   `git branch --show-current`. Si el primero no termina en /nexa, detenete.
1. Leé completos antigravity/00-LEEME.md, antigravity/INFORME-SALIDA.md (§4) y
   antigravity/modulos/M18-correcciones-demo.md.
1b. Leé la sección "Estado actual" de este módulo y corregí primero esos 3 defectos.
2. Abrí 4 subagentes, uno por tarea (M18.1, M18.2, M18.3 y M18.4), y lanzalos TODOS
   AL MISMO TIEMPO. No esperés a que uno termine para lanzar el siguiente.
   A cada uno dale: su tarea tal como está escrita, su lista de "Archivos que le
   pertenecen" y la regla 0 del LEEME. Ningún subagente edita un archivo que no está
   en su lista; si lo necesita, se detiene y te avisa.
3. Cuando terminen los 4, uní el trabajo y hacé un commit por tarea con el mensaje
   que indica cada una.
4. Corré VOS la verificación final de este módulo (sección "Verificación del
   coordinador"), después del último cambio. Si algo falla, devolvé la corrección al
   subagente dueño del archivo y volvé a verificar.
5. Agregá la sección "M18" al final de antigravity/REPORTE-FASE-S.md como dice
   "Reporte".
6. Subí: git push origin <la rama actual>
7. Detenete. Escribime "M18 listo para revisión".

No toques `backend/`, no apliques migraciones y no edites .env.local.
```

## Cómo arrancar el modo demo

```sh
cd frontend && node scripts/dev-demo.mjs
```

Arranca en el puerto **3200** con las variables de Supabase vacías, así que no toca la base de producción. No uses `npm run dev`: ese lee `.env.local` y apunta a producción.

---

## M18.1 "Crear cuenta" en el login del modo demo

**Archivos que le pertenecen:** `components/auth/login-screen.tsx`.

**Commit:** `fix(salida): M18.1 crear cuenta en el login demo`

Hoy, con `!AUTH_ENABLED` (líneas 135 a 150), la pantalla muestra solo "Entrar al demo". En ese bloque, debajo del `Link` "Entrar al demo", agregá exactamente el mismo párrafo que ya existe al final del formulario real (líneas 250 a 255):

```tsx
<p className="text-center text-sm text-slate-500">
  ¿No tenés cuenta?{" "}
  <Link href="/registro" className="font-semibold text-brand hover:underline">
    Crear cuenta
  </Link>
</p>
```

No cambies nada del formulario real.

**Prueba del subagente:** en el modo demo, `/login` muestra "Entrar al demo" y "Crear cuenta"; "Crear cuenta" lleva a `/registro`.

## M18.2 La sucursal nueva aparece en el demo

**Archivos que le pertenecen:** `lib/api/index.ts` (solo la función `session.branches`) y `lib/api/mock-platform.ts` (solo si hace falta exportar algo).

**Commit:** `fix(salida): M18.2 sucursales del demo desde la lista mock`

**Causa:** en mocks, `session.branches()` (`lib/api/index.ts`, alrededor de la línea 1410) devuelve una lista fija de una sucursal. `mockCreateBranch` agrega la sucursal nueva a `mockBranchesList`, pero nadie la lee. En el modo real esto ya funciona: no lo cambies.

**Cambio:** en la rama `if (USE_MOCKS)` de `session.branches()`, devolvé:

```ts
return mockBranchesList.map((b) => ({ id: b.id, name: b.name, code: b.code, role: "DUENO" as const }));
```

Importá `mockBranchesList` desde `./mock-platform` si no está importado. La primera sucursal de `mockBranchesList` debe seguir teniendo el id `MOCK_BRANCH_ID`: los demás mocks dependen de él.

**Prueba del subagente:** demo → `/configuracion/negocio` → "Sucursales" → "Nueva sucursal" → "Sede Norte" → "Guardar". El aviso "Sucursal creada exitosamente" aparece, y "Sede Norte" aparece en la lista y en el selector de la barra superior sin recargar.

## M18.3 Fuera los roles de restaurante y domicilios

**Archivos que le pertenecen:** `components/configuracion/users-screen.tsx` y `lib/api/mock-platform.ts`, salvo la función que toca M18.2. Si M18.2 necesita editar `mock-platform.ts`, coordinalo: M18.3 edita primero y M18.2 después.

**Commit:** `fix(salida): M18.3 roles de salida solamente`

Los roles `MESERO`, `COCINA` y `REPARTIDOR` son de las fases 5 y 6, pospuestas. No se muestran en ningún lado:

1. **Filtro "Todos los roles"** (`users-screen.tsx`, alrededor de la línea 323): en lugar de recorrer `Object.entries(ROLE_LABELS)`, recorré `["DUENO", ...ASSIGNABLE_ROLES]` y mostrá `ROLE_LABELS[code]`. `ASSIGNABLE_ROLES` ya está importado.
2. **Pestaña "Roles y Permisos":** mostrá solo esos mismos 5 roles. Si la lista sale de `mockPermissionsList()`, cambiá en `mock-platform.ts` el arreglo `roles` (alrededor de la línea 377) a `["DUENO", "ADMIN_CAJA", "SUPERVISOR", "CAJERO", "BODEGA"]`.
3. **Usuarios de ejemplo:** en `mock-platform.ts` borrá los usuarios con rol `MESERO`, `COCINA` y `REPARTIDOR`. Dejá los demás.
4. **No borres** los roles del tipo `Role`, de `ROLE_LABELS` ni de la matriz de permisos: el backend todavía los conoce y la pantalla "Tu rol todavía no tiene pantallas" (S4.5) depende de ellos.

**Prueba del subagente:** `grep -n "Mesero\|Cocina\|Repartidor"` sobre el texto de `/configuracion/usuarios` en el demo no devuelve nada, en ninguna de las dos pestañas.

## M18.4 Datos del demo de una licorería de Riohacha

**Archivos que le pertenecen:** los archivos de datos mock **excepto** `mock-platform.ts` (que es de M18.2 y M18.3): `lib/api/mock.ts`, `lib/api/mock-inventory.ts`, `lib/api/mock-procurement.ts` y los mocks de panel y reportes, si están en otro archivo.

**Commit:** `fix(salida): M18.4 demo de licorería`

El demo se le va a mostrar a licorerías y tiendas de Riohacha. Hoy muestra un supermercado de Medellín. Cambiá **solo textos de ejemplo**, no estructura, ids ni cantidades de registros:

1. **Negocio:** "Licorera La 15". Ciudad "Riohacha". El cambio del nombre del negocio, razón social y NIT está en `mock-platform.ts`: **no lo hagas vos**; anotá en tu respuesta al coordinador los valores, y el coordinador se los pasa a M18.3 o los aplica al unir:
   - `commercial_name`: "Licorera La 15";
   - `legal_name`: "Licorera La 15 S.A.S.";
   - sucursales: "Sede Centro" (código `SC`) y "Sede Calle 15" (código `S15`), ciudad "Riohacha". La segunda reemplaza a "Asadero El Vecino" y su tipo activo pasa a "Tienda";
   - correos `@licorerala15.co` en lugar de `@elvecino.co`.
2. **Productos y categorías:** reemplazá los nombres por estos, conservando precios y existencias del mismo orden de magnitud:
   - Categorías: "Aguardiente", "Ron", "Whisky", "Cerveza", "Vinos", "Gaseosas y agua", "Snacks", "Hielo".
   - Productos: "Aguardiente Antioqueño 750 ml", "Aguardiente Antioqueño sin azúcar 375 ml", "Ron Medellín 3 años 750 ml", "Ron Viejo de Caldas 750 ml", "Whisky Old Parr 12 años 750 ml", "Whisky Buchanan's 12 años 750 ml", "Cerveza Águila lata 330 ml", "Club Colombia Dorada 330 ml", "Six pack Águila", "Cerveza Poker lata x6", "Vino Gato Negro 750 ml", "Coca-Cola 1,5 L", "Agua Cristal 600 ml", "Papas Margarita 105 g", "Maní La Especial 180 g", "Hielo bolsa 2 kg".
   - Si un mock tiene más productos que esta lista, repetí presentaciones distintas de los mismos ("... 1 L", "... 1,75 L"). No inventes marcas fuera de las de esta lista.
3. **Clientes de ejemplo:** "Ramiro Epieyú", "Yolanda Pushaina", "Carlos Iguarán", "Martha Gómez", "Luis Fernando Brito". Sin datos reales de nadie: cédulas inventadas que empiecen por `1000000`.
4. **Proveedores:** "Distribuidora La Guajira", "Bavaria Riohacha", "Licores del Caribe".

**Prueba del subagente:** en el demo, `/pos`, `/inventario`, `/panel` y `/clientes` no muestran "Vecino", "Medellín", "Huevos", "Detergente", "Pan tajado" ni "Asadero".

---

## Verificación del coordinador

Esto lo corre el modelo 3.8, no los subagentes, **después del último cambio**:

```sh
cd frontend
npx tsc --noEmit
npm run lint
npx next build
grep -rn "elvecino\|Asadero\|El Vecino" lib components app
```

`tsc`, `lint` y `next build` sin errores; el `grep` sin resultados.

Luego, con `node scripts/dev-demo.mjs`, repasá los 8 puntos de la lista de M17 (en `M17-faseS-cierre.md`) y además los 4 de este módulo. Anotá cada uno como "pasa" o "falla", con lo que viste.

## Reporte

Sección "M18" al final de `antigravity/REPORTE-FASE-S.md`:
1. Una tabla con cada subagente: tarea, archivos que tocó, commit y las pruebas que corrió.
2. Los choques que tuviste que resolver al unir, y cómo.
3. La salida completa de tu verificación final.
4. El resultado de los 12 puntos (8 de M17 y 4 de M18).
5. Preguntas abiertas, con archivo y línea.

## Fuera de alcance

- La tarjeta "Propinas" y "Accesos rápidos" del panel: esperan la decisión A del usuario.
- El rediseño visual con el logo.
- Cualquier archivo de `backend/`.
