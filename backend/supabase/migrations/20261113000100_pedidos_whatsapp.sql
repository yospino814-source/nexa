-- Pedidos por WhatsApp: bot por sede, zonas de domicilio, conversaciones,
-- mensajes y pedidos. El bot atiende con el catálogo y las existencias de la
-- sede; el equipo prepara, despacha y cobra desde la bandeja de pedidos.

-- Un número de WhatsApp Business por sede.
create table public.whatsapp_bots (
  id uuid primary key default gen_random_uuid(),
  tenant_id uuid not null references public.tenants(id) on delete cascade,
  branch_id uuid not null,
  -- Id del número en la Cloud API de Meta (metadata.phone_number_id).
  phone_number_id text not null unique,
  display_phone text,
  business_name text not null check (length(trim(business_name)) between 1 and 80),
  active boolean not null default true,
  opens_at time not null default '10:00',
  closes_at time not null default '23:00',
  min_order numeric(12,2) not null default 0 check (min_order >= 0),
  prepay_enabled boolean not null default true,
  usual_enabled boolean not null default true,
  substitutes_enabled boolean not null default true,
  -- Licor y cigarrillos: confirmar mayoría de edad. Se puede apagar solo si
  -- la sede no vende productos restringidos.
  age_check boolean not null default true,
  next_order_number integer not null default 1 check (next_order_number > 0),
  created_at timestamptz not null default now(),
  updated_at timestamptz not null default now(),
  unique (branch_id),
  unique (tenant_id, id),
  foreign key (tenant_id, branch_id) references public.branches (tenant_id, id) on delete cascade
);

create table public.whatsapp_delivery_zones (
  id uuid primary key default gen_random_uuid(),
  tenant_id uuid not null references public.tenants(id) on delete cascade,
  branch_id uuid not null,
  name text not null check (length(trim(name)) between 1 and 60),
  fee numeric(12,2) not null default 0 check (fee >= 0),
  eta_min integer not null default 20 check (eta_min >= 0),
  eta_max integer not null default 40 check (eta_max >= eta_min),
  active boolean not null default true,
  created_at timestamptz not null default now(),
  unique (branch_id, name),
  foreign key (tenant_id, branch_id) references public.branches (tenant_id, id) on delete cascade
);

create table public.whatsapp_conversations (
  id uuid primary key default gen_random_uuid(),
  tenant_id uuid not null references public.tenants(id) on delete cascade,
  branch_id uuid not null,
  -- Número del cliente como lo manda WhatsApp (solo dígitos, con indicativo).
  wa_phone text not null check (wa_phone ~ '^[0-9]{8,15}$'),
  contact_name text,
  customer_id uuid references public.customers(id) on delete set null,
  -- Estado del bot (paso, carrito, dirección). Lo escribe solo el backend.
  state jsonb not null default '{}'::jsonb,
  needs_human boolean not null default false,
  created_at timestamptz not null default now(),
  updated_at timestamptz not null default now(),
  unique (branch_id, wa_phone),
  foreign key (tenant_id, branch_id) references public.branches (tenant_id, id) on delete cascade
);

create table public.whatsapp_messages (
  id uuid primary key default gen_random_uuid(),
  tenant_id uuid not null references public.tenants(id) on delete cascade,
  conversation_id uuid not null references public.whatsapp_conversations(id) on delete cascade,
  direction text not null check (direction in ('IN', 'OUT')),
  -- Id de Meta. Único para no procesar dos veces un reintento del webhook.
  wa_message_id text unique,
  body text not null default '',
  created_at timestamptz not null default now()
);
create index whatsapp_messages_conv_idx on public.whatsapp_messages (conversation_id, created_at);

create table public.whatsapp_orders (
  id uuid primary key default gen_random_uuid(),
  tenant_id uuid not null references public.tenants(id) on delete cascade,
  branch_id uuid not null,
  code text not null,
  conversation_id uuid not null references public.whatsapp_conversations(id) on delete restrict,
  customer_id uuid references public.customers(id) on delete set null,
  wa_phone text not null,
  contact_name text,
  -- [{product_id, name, qty, unit_price}] con precio con impuesto al momento del pedido.
  items jsonb not null check (jsonb_typeof(items) = 'array' and jsonb_array_length(items) > 0),
  subtotal numeric(12,2) not null check (subtotal >= 0),
  delivery_fee numeric(12,2) not null default 0 check (delivery_fee >= 0),
  total numeric(12,2) not null check (total = subtotal + delivery_fee),
  pickup boolean not null default false,
  address text,
  zone_id uuid references public.whatsapp_delivery_zones(id) on delete set null,
  age_confirmed boolean not null default false,
  payment_method text not null check (payment_method in ('LINK', 'EFECTIVO', 'DATAFONO')),
  payment_status text not null default 'PENDIENTE' check (payment_status in ('PENDIENTE', 'PAGADO')),
  payment_reference text,
  status text not null default 'NUEVO' check (status in ('NUEVO', 'PREPARANDO', 'EN_CAMINO', 'ENTREGADO', 'CANCELADO')),
  courier_name text,
  -- Venta del POS con la que se cobró el pedido.
  sale_id uuid,
  created_at timestamptz not null default now(),
  updated_at timestamptz not null default now(),
  unique (branch_id, code),
  check (pickup or address is not null),
  foreign key (tenant_id, branch_id) references public.branches (tenant_id, id) on delete cascade
);
create index whatsapp_orders_board_idx on public.whatsapp_orders (branch_id, status, created_at desc);
create index whatsapp_orders_phone_idx on public.whatsapp_orders (branch_id, wa_phone, created_at desc);

-- RLS: igual que el resto, por tenant y sucursal del contexto.
alter table public.whatsapp_bots enable row level security;
alter table public.whatsapp_delivery_zones enable row level security;
alter table public.whatsapp_conversations enable row level security;
alter table public.whatsapp_messages enable row level security;
alter table public.whatsapp_orders enable row level security;

create policy whatsapp_bots_isolation on public.whatsapp_bots for all to authenticated
  using (public.in_scope(tenant_id, branch_id)) with check (public.in_scope(tenant_id, branch_id));
create policy whatsapp_zones_isolation on public.whatsapp_delivery_zones for all to authenticated
  using (public.in_scope(tenant_id, branch_id)) with check (public.in_scope(tenant_id, branch_id));
create policy whatsapp_conversations_isolation on public.whatsapp_conversations for all to authenticated
  using (public.in_scope(tenant_id, branch_id)) with check (public.in_scope(tenant_id, branch_id));
create policy whatsapp_messages_isolation on public.whatsapp_messages for all to authenticated
  using (tenant_id = public.current_tenant_id()) with check (tenant_id = public.current_tenant_id());
create policy whatsapp_orders_isolation on public.whatsapp_orders for all to authenticated
  using (public.in_scope(tenant_id, branch_id)) with check (public.in_scope(tenant_id, branch_id));

grant select, insert, update, delete on public.whatsapp_bots, public.whatsapp_delivery_zones,
  public.whatsapp_conversations, public.whatsapp_messages, public.whatsapp_orders to authenticated;

-- El webhook de Meta llega sin usuario: solo trae el phone_number_id. Esta
-- función resuelve a qué negocio y sede pertenece ese número para que el
-- backend fije el contexto RLS. No expone nada más.
create or replace function public.whatsapp_bot_route(p_phone_number_id text)
returns table (tenant_id uuid, branch_id uuid)
language sql
stable
security definer
set search_path = ''
as $$
  select b.tenant_id, b.branch_id
    from public.whatsapp_bots b
   where b.phone_number_id = p_phone_number_id and b.active
$$;
revoke all on function public.whatsapp_bot_route(text) from public, anon;
grant execute on function public.whatsapp_bot_route(text) to authenticated;
