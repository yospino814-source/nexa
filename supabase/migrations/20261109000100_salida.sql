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


-- S2 Invitaciones
alter table public.invitations
  add constraint invitations_status_check check (status in ('PENDIENTE','ACEPTADA','REVOCADA'));

create or replace function public.my_invitations()
returns table (id uuid, tenant_name text, branch_name text, role_code text, role_name text, created_at timestamptz)
language sql
stable
security definer
set search_path = ''
as $$
  select i.id, t.name, b.name, r.tima_codigo, r.tima_nombre, i.created_at
    from public.invitations i
    join public.tenants t on t.id = i.tenant_id
    left join public.branches b on b.id = i.branch_id
    join public.tbl_tipos_maestra r on r.tima_id = i.role_id
    join auth.users u on u.id = auth.uid()
   where i.status = 'PENDIENTE'
     and u.email_confirmed_at is not null
     and lower(i.email) = lower(u.email)
   order by i.created_at
$$;
revoke all on function public.my_invitations() from public, anon;
grant execute on function public.my_invitations() to authenticated;

create or replace function public.accept_invitation(p_id uuid)
returns table (out_tenant_id uuid, out_branch_id uuid, out_role_code text)
language plpgsql
security definer
set search_path = ''
as $$
#variable_conflict use_column
declare
  v_uid uuid := auth.uid();
  v_email text;
  v_confirmed timestamptz;
  v_inv public.invitations%rowtype;
begin
  if v_uid is null then
    raise exception 'ONBOARDING_NO_SESSION';
  end if;

  select u.email, u.email_confirmed_at into v_email, v_confirmed
    from auth.users u where u.id = v_uid;

  select * into v_inv from public.invitations i where i.id = p_id for update;

  -- El mismo error si no existe o es de otro correo: no se revela que existe.
  if not found or v_confirmed is null or lower(v_inv.email) <> lower(v_email) then
    raise exception 'INVITATION_NOT_FOUND';
  end if;
  if v_inv.status <> 'PENDIENTE' then
    raise exception 'INVITATION_NOT_PENDING';
  end if;

  insert into public.users (id, full_name, email)
  values (v_uid, v_inv.full_name, v_email)
  on conflict (id) do nothing;

  insert into public.memberships (user_id, tenant_id, branch_id, role_id, active)
  values (v_uid, v_inv.tenant_id, v_inv.branch_id, v_inv.role_id, true)
  on conflict (user_id, tenant_id, branch_id)
  do update set role_id = excluded.role_id, active = true;

  update public.invitations set status = 'ACEPTADA', accepted_at = now() where id = p_id;

  return query
    select v_inv.tenant_id, v_inv.branch_id, r.tima_codigo::text
      from public.tbl_tipos_maestra r where r.tima_id = v_inv.role_id;
end;
$$;
revoke all on function public.accept_invitation(uuid) from public, anon;
grant execute on function public.accept_invitation(uuid) to authenticated;
