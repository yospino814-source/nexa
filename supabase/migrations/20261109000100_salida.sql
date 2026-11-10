-- S1 Alta de negocio
create or replace function public.create_business(
  p_business_name text,
  p_branch_name text,
  p_full_name text
)
returns table (tenant_id uuid, branch_id uuid)
language plpgsql
security definer
set search_path = ''
as $$
#variable_conflict use_column
declare
  v_uid uuid := auth.uid();
  v_email text;
  v_tenant uuid;
  v_branch uuid;
begin
  if v_uid is null then
    raise exception 'ONBOARDING_NO_SESSION';
  end if;

  -- Dos llamadas simultáneas del mismo usuario no crean dos negocios.
  perform pg_advisory_xact_lock(hashtext('create_business:' || v_uid::text));

  if exists (select 1 from public.memberships m where m.user_id = v_uid and m.active) then
    raise exception 'ONBOARDING_ALREADY_MEMBER';
  end if;

  select u.email into v_email from auth.users u where u.id = v_uid;

  insert into public.users (id, full_name, email)
  values (v_uid, p_full_name, v_email)
  on conflict (id) do update set full_name = excluded.full_name;

  insert into public.tenants (name) values (p_business_name)
  returning id into v_tenant;

  insert into public.branches (tenant_id, name) values (v_tenant, p_branch_name)
  returning id into v_branch;

  insert into public.memberships (user_id, tenant_id, branch_id, role_id, active)
  values (v_uid, v_tenant, null, public.tipo_id('ROL', 'DUENO'), true);

  return query select v_tenant, v_branch;
end;
$$;
revoke all on function public.create_business(text, text, text) from public, anon;
grant execute on function public.create_business(text, text, text) to authenticated;

