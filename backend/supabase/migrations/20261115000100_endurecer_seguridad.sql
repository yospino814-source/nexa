-- Endurecimiento tras la auditoría de seguridad (noviembre 2026).

-- 1. Funciones que corren sin contexto de negocio: fuera del esquema público.
-- Supabase publica `public` en su API REST (PostgREST); un esquema propio no
-- se publica, así que nadie las puede llamar con la clave pública.
create schema if not exists private;
revoke all on schema private from public, anon;
grant usage on schema private to authenticated;

-- 1a. Intentos de login: se cuentan por correo **y** IP. Antes se contaban
-- solo por correo y cualquiera podía bloquear la entrada de otra persona
-- mandando 5 fallos con su correo.
drop function if exists public.record_login_attempt(text, text, text, boolean, text);
create or replace function private.record_login_attempt(
  p_email text, p_ip text, p_user_agent text, p_success boolean, p_reason text
)
returns integer
language plpgsql
security definer
set search_path = ''
as $$
declare
  v_email text := lower(trim(p_email));
  v_ip text := left(nullif(trim(p_ip), ''), 64);
  v_failed integer;
begin
  if v_email is null or v_email = '' then
    raise exception 'email is required' using errcode = '22023';
  end if;
  if p_success is not null then
    insert into public.login_attempts (email, ip, user_agent, success, reason)
    values (v_email, v_ip, left(p_user_agent, 300), p_success, left(p_reason, 200));
  end if;
  select count(*) into v_failed
    from public.login_attempts
   where email = v_email
     and ip is not distinct from v_ip
     and not success
     and at >= now() - interval '15 minutes';
  return v_failed;
end;
$$;
revoke all on function private.record_login_attempt(text, text, text, boolean, text) from public, anon;
grant execute on function private.record_login_attempt(text, text, text, boolean, text) to authenticated;

-- 1b. Ruta del webhook de WhatsApp (phone_number_id → negocio y sede).
drop function if exists public.whatsapp_bot_route(text);
create or replace function private.whatsapp_bot_route(p_phone_number_id text)
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
revoke all on function private.whatsapp_bot_route(text) from public, anon;
grant execute on function private.whatsapp_bot_route(text) to authenticated;

-- 2. Negocio activo: solo uno donde el usuario tiene membresía activa (y una
-- sucursal que le corresponde). Antes cualquiera podía poner cualquier
-- negocio en su contexto y el JWT lo copiaba.
create or replace function public.has_active_membership(p_tenant uuid, p_branch uuid)
returns boolean
language sql
stable
security definer
set search_path = ''
as $$
  select exists (
    select 1 from public.memberships m
     where m.user_id = auth.uid()
       and m.active
       and m.tenant_id = p_tenant
       and (p_branch is null or m.branch_id is null or m.branch_id = p_branch)
  )
$$;
revoke all on function public.has_active_membership(uuid, uuid) from public, anon;
grant execute on function public.has_active_membership(uuid, uuid) to authenticated;

drop policy if exists user_active_context_self on public.user_active_context;
create policy user_active_context_self_read on public.user_active_context for select to authenticated
  using (user_id = auth.uid());
create policy user_active_context_self_write on public.user_active_context for insert to authenticated
  with check (user_id = auth.uid() and public.has_active_membership(tenant_id, branch_id));
create policy user_active_context_self_update on public.user_active_context for update to authenticated
  using (user_id = auth.uid())
  with check (user_id = auth.uid() and public.has_active_membership(tenant_id, branch_id));
create policy user_active_context_self_delete on public.user_active_context for delete to authenticated
  using (user_id = auth.uid());

-- El hook del JWT tampoco confía en la fila: revisa la membresía.
create or replace function public.custom_access_token_hook(event jsonb)
returns jsonb
language plpgsql
stable
security definer
set search_path = ''
as $$
declare
  claims jsonb;
  ctx record;
  v_user uuid := (event ->> 'user_id')::uuid;
begin
  claims := event -> 'claims';
  select uc.tenant_id, uc.branch_id into ctx
    from public.user_active_context uc
   where uc.user_id = v_user
     and exists (
       select 1 from public.memberships m
        where m.user_id = v_user and m.active and m.tenant_id = uc.tenant_id
          and (uc.branch_id is null or m.branch_id is null or m.branch_id = uc.branch_id)
     );
  if ctx.tenant_id is not null then
    claims := jsonb_set(claims, '{app_metadata,tenant_id}', to_jsonb(ctx.tenant_id::text));
    if ctx.branch_id is not null then
      claims := jsonb_set(claims, '{app_metadata,branch_id}', to_jsonb(ctx.branch_id::text));
    end if;
  end if;
  return jsonb_set(event, '{claims}', claims);
end;
$$;
revoke execute on function public.custom_access_token_hook(jsonb) from authenticated, anon, public;
grant execute on function public.custom_access_token_hook(jsonb) to supabase_auth_admin;

-- 3. Perfil propio: se puede leer y cambiar el nombre, nada más. El correo
-- lo administra Supabase Auth; altas y bajas, las funciones del sistema.
drop policy if exists users_self on public.users;
create policy users_self_read on public.users for select to authenticated using (id = auth.uid());
create policy users_self_update on public.users for update to authenticated
  using (id = auth.uid()) with check (id = auth.uid());
revoke insert, update, delete on public.users from authenticated;
grant update (full_name) on public.users to authenticated;
