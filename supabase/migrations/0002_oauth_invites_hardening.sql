-- Version 2: invite rules that also work for OAuth sign-ups (Google, Microsoft, Apple), password sign-in disabled,
-- and the security advisor's findings on 0001 fixed.
--
-- Invite rules for a NEW account (signing in to an existing account never runs them; Supabase links an OAuth
-- identity to the existing user when the provider reports the same verified email):
--   1. the email is on private.allowed_emails (the owner, seeded by `cargo xtask supabase-owner`), or
--   2. user_metadata.invite_code is a usable invite (email OTP sign-up passes it in `options.data`), and when that
--      invite is bound to an email, it is this user's email, or
--   3. a usable invite is bound to this user's email (OAuth cannot carry metadata, so the sign-in screen binds the
--      typed code to the typed email with reserve_invite() before redirecting) and the provider says the email is
--      verified. Apple private-relay addresses never match a bound invite: those users sign up with email first and
--      link Apple from settings (manual linking).

-- ------------------------------------------------------------------------------------------------- hygiene first

-- Functions are executable by PUBLIC by default; nothing in `private` may be, unless granted one by one below.
alter default privileges in schema private revoke execute on functions from public;
alter default privileges in schema public revoke execute on functions from public;
revoke execute on all functions in schema private from public, anon, authenticated;

-- RLS on and forced on every table (the `postgres` role keeps BYPASSRLS, so migrations and the definer functions
-- below still work; nothing else bypasses it).
alter table public.profiles force row level security;
alter table public.invites force row level security;
alter table public.documents force row level security;
alter table public.progress force row level security;
alter table public.bookmarks force row level security;
alter table public.voices force row level security;
alter table public.budget_shares force row level security;

-- ------------------------------------------------------------------------------------- invites bound to an email

alter table public.invites add column email text check (email = lower(btrim(email)) and email like '%_@_%');
create index invites_by_email on public.invites (email) where used_by is null;

-- Emails that may always create an account (the owner). Not reachable through PostgREST.
create table private.allowed_emails (
  email       text primary key check (email = lower(btrim(email))),
  created_at  timestamptz not null default now()
);
alter table private.allowed_emails enable row level security;
alter table private.allowed_emails force row level security;
revoke all on table private.allowed_emails from public, anon, authenticated;

-- The invite (or 'allowed') a new account may use, or null. Pure lookup; spending happens in the trigger.
create or replace function private.signup_allowance(email text, code text, email_verified boolean) returns text
language sql stable security definer set search_path = '' as $$
  select coalesce(
    (select 'allowed' from private.allowed_emails a where a.email = lower(btrim(signup_allowance.email))),
    (select i.code from public.invites i
      where i.code = private.normalise_invite(signup_allowance.code)
        and i.used_by is null and i.expires_at > now()
        and (i.email is null or i.email = lower(btrim(signup_allowance.email)))),
    (select i.code from public.invites i
      where signup_allowance.email_verified
        and i.email = lower(btrim(signup_allowance.email))
        and i.used_by is null and i.expires_at > now()
      order by i.created_at limit 1)
  )
$$;
revoke all on function private.signup_allowance(text, text, boolean) from public, anon, authenticated;

-- Whether the provider vouches for the email. Email sign-ups are verified by the code itself, so they count as
-- verified here; OAuth providers report `email_verified` in the identity data copied to user_metadata.
create or replace function private.email_is_verified(provider text, meta jsonb) returns boolean
language sql immutable set search_path = '' as $$
  select coalesce(provider, 'email') = 'email' or coalesce((meta ->> 'email_verified')::boolean, false)
$$;
revoke all on function private.email_is_verified(text, jsonb) from public, anon, authenticated;

create or replace function private.hook_before_user_created(event jsonb) returns jsonb
language plpgsql security definer set search_path = '' as $$
declare
  u jsonb := event -> 'user';
  allowance text := private.signup_allowance(
    u ->> 'email',
    u -> 'user_metadata' ->> 'invite_code',
    private.email_is_verified(u -> 'app_metadata' ->> 'provider', u -> 'user_metadata'));
begin
  if allowance is null then
    return jsonb_build_object('error', jsonb_build_object('http_code', 403, 'message',
      'Streams Reader is invite-only: enter your invite code with the email address it was sent to'));
  end if;
  return '{}'::jsonb;
end;
$$;
revoke all on function private.hook_before_user_created(jsonb) from public, anon, authenticated;
grant execute on function private.hook_before_user_created(jsonb) to supabase_auth_admin;

create or replace function private.handle_new_user() returns trigger
language plpgsql security definer set search_path = '' as $$
declare
  wanted_locale text := new.raw_user_meta_data ->> 'locale';
  allowance text := private.signup_allowance(
    new.email,
    new.raw_user_meta_data ->> 'invite_code',
    private.email_is_verified(new.raw_app_meta_data ->> 'provider', new.raw_user_meta_data));
begin
  if allowance is null or (allowance <> 'allowed' and not private.redeem_invite(allowance, new.id)) then
    raise exception 'Streams Reader is invite-only: the invite code is not valid, already used or expired'
      using errcode = 'P0001';
  end if;
  if wanted_locale is null or wanted_locale !~ '^[a-z]{2,3}(-[A-Za-z0-9]{2,8})*$' then
    wanted_locale := 'en';
  end if;
  insert into public.profiles (id, display_name, locale)
  values (new.id,
          nullif(left(btrim(coalesce(new.raw_user_meta_data ->> 'display_name',
                                     new.raw_user_meta_data ->> 'full_name',
                                     new.raw_user_meta_data ->> 'name', '')), 80), ''),
          wanted_locale);
  insert into public.budget_shares (user_id) values (new.id);
  return new;
end;
$$;
revoke all on function private.handle_new_user() from public, anon, authenticated;

-- --------------------------------------------------------------------------------- password sign-in turned off

-- Supabase has no switch that keeps email codes but refuses passwords, so no account may hold one: every password
-- written by sign-up, the Admin API or updateUser() is blanked before it is stored, and password sign-in always
-- fails. People sign in with email codes, magic links or OAuth.
create or replace function private.no_passwords() returns trigger
language plpgsql security definer set search_path = '' as $$
begin
  new.encrypted_password := '';
  return new;
end;
$$;
revoke all on function private.no_passwords() from public, anon, authenticated;
create trigger no_passwords before insert or update of encrypted_password on auth.users
  for each row when (new.encrypted_password is not null and new.encrypted_password <> '')
  execute function private.no_passwords();

-- ------------------------------------------------------------------------ public RPC: invoker over private definer

-- The exposed functions are SECURITY INVOKER; the privileged lookups they need live in `private`, which PostgREST
-- does not serve. anon and authenticated may call exactly these private functions and nothing else there.
grant usage on schema private to anon, authenticated;

drop function public.invite_status(text);
create or replace function private.invite_status(code text) returns text
language sql stable security definer set search_path = '' as $$
  select coalesce((
    select case when i.used_by is not null then 'used'
                when i.expires_at <= now() then 'expired'
                else 'valid' end
      from public.invites i where i.code = private.normalise_invite(invite_status.code)), 'unknown')
$$;
revoke all on function private.invite_status(text) from public;
grant execute on function private.invite_status(text) to anon, authenticated;
grant execute on function private.normalise_invite(text) to anon, authenticated;

-- Whether a typed invite code can be used: 'valid', 'used', 'expired' or 'unknown'.
create function public.invite_status(code text) returns text
language sql stable security invoker set search_path = '' as $$
  select private.invite_status(invite_status.code)
$$;
revoke all on function public.invite_status(text) from public;
grant execute on function public.invite_status(text) to anon, authenticated;

-- Bind a usable, unbound invite to the email the person typed, before an OAuth redirect (OAuth sign-ups cannot carry
-- the code). Answers 'reserved' (also when already bound to this email), 'taken' (bound to another email) or the
-- invite_status answer for unusable codes. Binding never spends the invite; the sign-up does.
create or replace function private.reserve_invite(code text, email text) returns text
language plpgsql security definer set search_path = '' as $$
declare
  wanted text := lower(btrim(coalesce(reserve_invite.email, '')));
  status text := private.invite_status(reserve_invite.code);
  bound text;
begin
  if wanted !~ '^[^@\s]+@[^@\s]+\.[^@\s]+$' then
    return 'bad_email';
  end if;
  if status <> 'valid' then
    return status;
  end if;
  update public.invites i set email = wanted
   where i.code = private.normalise_invite(reserve_invite.code) and i.email is null and i.used_by is null;
  select i.email into bound from public.invites i where i.code = private.normalise_invite(reserve_invite.code);
  return case when bound = wanted then 'reserved' else 'taken' end;
end;
$$;
revoke all on function private.reserve_invite(text, text) from public;
grant execute on function private.reserve_invite(text, text) to anon, authenticated;

create function public.reserve_invite(code text, email text) returns text
language sql volatile security invoker set search_path = '' as $$
  select private.reserve_invite(reserve_invite.code, reserve_invite.email)
$$;
revoke all on function public.reserve_invite(text, text) from public;
grant execute on function public.reserve_invite(text, text) to anon, authenticated;

-- create_invite needs no definer rights: the invites policies already let admins (and only admins) insert.
drop function public.create_invite(integer, text);
create function public.create_invite(days integer default 14, note text default null, email text default null)
returns text
language plpgsql volatile security invoker set search_path = '' as $$
declare
  made text;
begin
  if days is null or days < 1 or days > 90 then
    raise exception 'days must be between 1 and 90' using errcode = '22023';
  end if;
  insert into public.invites (created_by, note, email, expires_at)
  values (auth.uid(), left(create_invite.note, 200), nullif(lower(btrim(create_invite.email)), ''),
          now() + make_interval(days => create_invite.days))
  returning code into made;
  return made;
end;
$$;
revoke all on function public.create_invite(integer, text, text) from public, anon;
grant execute on function public.create_invite(integer, text, text) to authenticated;

-- gpu_share sums other users' shares, which RLS hides from the caller: the sum is a private definer function.
drop function public.gpu_share(integer);
create or replace function private.gpu_share_of(user_id uuid, daily_cap_s integer) returns integer
language sql stable security definer set search_path = '' as $$
  select case
    when p.is_admin then greatest(0, gpu_share_of.daily_cap_s - coalesce((
      select sum(coalesce(b.daily_gpu_s, 60))::integer
        from public.profiles q left join public.budget_shares b on b.user_id = q.id
       where not q.is_admin), 0))
    else least(gpu_share_of.daily_cap_s, coalesce((
      select b.daily_gpu_s from public.budget_shares b where b.user_id = p.id), 60))
  end
  from public.profiles p where p.id = gpu_share_of.user_id
$$;
revoke all on function private.gpu_share_of(uuid, integer) from public;
grant execute on function private.gpu_share_of(uuid, integer) to authenticated;

-- The caller's daily GPU share in seconds given the deployment's cap (the Worker calls it with the user's token).
create function public.gpu_share(daily_cap_s integer) returns integer
language sql stable security invoker set search_path = '' as $$
  select private.gpu_share_of(auth.uid(), gpu_share.daily_cap_s)
$$;
revoke all on function public.gpu_share(integer) from public, anon;
grant execute on function public.gpu_share(integer) to authenticated;
