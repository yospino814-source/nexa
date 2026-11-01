-- Correcciones de RLS de la fase 0.
--
-- `memberships` y `users` solo permitían ver la fila propia (user_id = auth.uid()).
-- Con eso, dentro del backend nadie veía a los demás miembros del negocio:
-- cambiar el rol o desactivar a otro usuario respondía "no encontrado", el
-- selector de meseros quedaba vacío y los nombres de cajeros salían en blanco.
--
-- Estas políticas se suman a las existentes (son permisivas: basta con una).
-- Dependen de app.current_tenant_id, que solo fija el backend; por PostgREST
-- (sin ese contexto) siguen sin dar acceso a filas ajenas. Los permisos de
-- negocio (users:manage, etc.) los valida el backend.

create policy memberships_tenant_read on public.memberships
  for select using (tenant_id = public.current_tenant_id());

create policy memberships_tenant_update on public.memberships
  for update using (tenant_id = public.current_tenant_id())
  with check (tenant_id = public.current_tenant_id());

create policy users_tenant_read on public.users
  for select using (
    exists (
      select 1 from public.memberships m
       where m.user_id = users.id
         and m.tenant_id = public.current_tenant_id()
    )
  );

-- login_attempts: la tabla no tiene políticas ni permisos para authenticated
-- (nadie la lee por PostgREST). El backend registra y cuenta los intentos con
-- esta función, que corre con los permisos de su dueño. Devuelve los fallos
-- del correo en los últimos 15 minutos.
create or replace function public.record_login_attempt(
  p_email text,
  p_ip text,
  p_user_agent text,
  p_success boolean,
  p_reason text
) returns integer
language plpgsql
security definer
set search_path = ''
as $$
declare
  v_email text := lower(trim(p_email));
  v_failed integer;
begin
  if v_email is null or v_email = '' then
    raise exception 'email is required' using errcode = '22023';
  end if;
  if p_success is not null then
    insert into public.login_attempts (email, ip, user_agent, success, reason)
    values (v_email, left(p_ip, 64), left(p_user_agent, 300), p_success, left(p_reason, 200));
  end if;
  select count(*) into v_failed
    from public.login_attempts
   where email = v_email and not success and at >= now() - interval '15 minutes';
  return v_failed;
end;
$$;
revoke execute on function public.record_login_attempt(text, text, text, boolean, text) from public, anon;
grant execute on function public.record_login_attempt(text, text, text, boolean, text) to authenticated;
