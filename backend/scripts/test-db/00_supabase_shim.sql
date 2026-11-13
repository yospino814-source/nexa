-- Shim minimo de Supabase para tests locales.
create role anon nologin;
create role authenticated nologin;
create role service_role nologin bypassrls;
create role supabase_auth_admin nologin;
create schema auth;
grant usage on schema auth to authenticated, anon, supabase_auth_admin;
create table auth.users (id uuid primary key, email text, email_confirmed_at timestamptz);
create or replace function auth.uid() returns uuid language sql stable as $$
  select coalesce(
    nullif(current_setting('request.jwt.claim.sub', true), ''),
    (nullif(current_setting('request.jwt.claims', true), '')::jsonb ->> 'sub')
  )::uuid
$$;
grant execute on function auth.uid() to authenticated, anon;
grant usage on schema public to supabase_auth_admin;
