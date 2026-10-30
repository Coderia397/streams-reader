-- nolabs Supabase schema, version 1: accounts (invite-only), each user's library, progress, bookmarks, voices and
-- GPU budget share. Forward-only: later changes go in 0002_*.sql and never edit this file. Applied by
-- `cargo xtask supabase-migrate` through the Management API, recorded in private.nolabs_migrations.
--
-- Security model (notes/14, D10): the apps talk to PostgREST with the user's access token; row-level security keeps
-- every row owner-only. The Cloudflare Worker uses the same token (never a service key), so it can do no more than
-- the user could. Internal functions live in the `private` schema, which PostgREST does not expose.

create schema if not exists private;
revoke all on schema private from public, anon, authenticated;
grant usage on schema private to supabase_auth_admin;

-- Keeps updated_at honest on every UPDATE.
create or replace function private.touch_updated_at() returns trigger
language plpgsql set search_path = '' as $$
begin
  new.updated_at := now();
  return new;
end;
$$;

-- 16 chars of lowercase Crockford base32: the ids the apps and the Worker generate (nolabs_core::ids).
create domain public.nolabs_id as text check (value ~ '^[0-9a-hjkmnp-tv-z]{16}$');

-- ------------------------------------------------------------------------------------------------------- profiles

-- One row per account, created by the sign-up trigger below. Settings live here (no separate settings table).
create table public.profiles (
  id             uuid primary key references auth.users (id) on delete cascade,
  display_name   text check (char_length(display_name) <= 80),
  locale         text not null default 'en' check (locale ~ '^[a-z]{2,3}(-[A-Za-z0-9]{2,8})*$'),
  default_voice  text check (char_length(default_voice) <= 100),
  default_speed  real not null default 1.0 check (default_speed between 0.5 and 3.0),
  theme          text not null default 'system' check (theme in ('system', 'light', 'dark')),
  -- Admins create invites and get the rest of the daily GPU budget. Only SQL run by the owner can set it.
  is_admin       boolean not null default false,
  created_at     timestamptz not null default now(),
  updated_at     timestamptz not null default now()
);
create trigger profiles_touch before update on public.profiles
  for each row execute function private.touch_updated_at();

-- -------------------------------------------------------------------------------------------------------- invites

-- Sign-up needs an unused, unexpired code in the user metadata (`invite_code`). Codes are 12 hex chars (48 random
-- bits from gen_random_uuid's random part) and expire, so guessing one is not practical.
create table public.invites (
  code        text primary key default substr(replace(gen_random_uuid()::text, '-', ''), 1, 12)
              check (code ~ '^[0-9a-z]{8,32}$'),
  created_by  uuid references auth.users (id) on delete set null,
  used_by     uuid unique references auth.users (id) on delete set null,
  used_at     timestamptz,
  note        text check (char_length(note) <= 200),
  expires_at  timestamptz not null default now() + interval '14 days',
  created_at  timestamptz not null default now()
);

-- The canonical form of a typed code: lowercase, no spaces or dashes.
create or replace function private.normalise_invite(code text) returns text
language sql immutable set search_path = '' as $$
  select lower(regexp_replace(coalesce(code, ''), '[\s-]', '', 'g'))
$$;

-- ------------------------------------------------------------------------------------------------------ documents

-- Metadata of an imported document. The body lives in R2 at docs/<user_id>/<id>/body.json, uploaded through the
-- Worker, which sets `ready` once R2 has it.
create table public.documents (
  id           public.nolabs_id primary key,
  user_id      uuid not null default auth.uid() references auth.users (id) on delete cascade,
  title        text not null check (char_length(btrim(title)) between 1 and 500),
  author       text check (char_length(author) <= 500),
  source       text not null check (source in ('paste', 'url', 'epub', 'text', 'markdown', 'html', 'pdf')),
  source_url   text check (char_length(source_url) <= 4096),
  language     text not null default 'en' check (char_length(language) between 1 and 35),
  char_count   bigint not null default 0 check (char_count >= 0),
  word_count   bigint not null default 0 check (word_count >= 0),
  block_count  integer not null default 0 check (block_count >= 0),
  -- Exact byte length of the body the app will PUT to the Worker (at most 32 MiB, as MAX_BODY_BYTES).
  body_bytes   bigint not null check (body_bytes between 1 and 33554432),
  ready        boolean not null default false,
  created_at   timestamptz not null default now(),
  updated_at   timestamptz not null default now(),
  -- Target of the composite foreign keys below, so progress and bookmarks can only point at the owner's documents.
  unique (id, user_id)
);
create index documents_by_user on public.documents (user_id, updated_at desc);
create trigger documents_touch before update on public.documents
  for each row execute function private.touch_updated_at();

-- ------------------------------------------------------------------------------------------------------- progress

-- Reading position per document, synced between devices through Realtime. `updated_at` is the client's clock when the
-- position was taken; an older write never replaces a newer one (see progress_keep_newest).
create table public.progress (
  user_id      uuid not null default auth.uid() references auth.users (id) on delete cascade,
  document_id  public.nolabs_id not null,
  char_offset  bigint not null check (char_offset >= 0),
  updated_at   timestamptz not null default now(),
  device       text check (char_length(device) <= 100),
  primary key (user_id, document_id),
  foreign key (document_id, user_id) references public.documents (id, user_id) on delete cascade
);

-- A late write from another device (a beacon sent when a tab closed) must not move progress back in time; skipping
-- the row keeps upserts from PostgREST working unchanged.
create or replace function private.progress_keep_newest() returns trigger
language plpgsql set search_path = '' as $$
begin
  if new.updated_at < old.updated_at then
    return null;
  end if;
  return new;
end;
$$;
create trigger progress_keep_newest before update on public.progress
  for each row execute function private.progress_keep_newest();

-- ------------------------------------------------------------------------------------------------------ bookmarks

create table public.bookmarks (
  id           uuid primary key default gen_random_uuid(),
  user_id      uuid not null default auth.uid() references auth.users (id) on delete cascade,
  document_id  public.nolabs_id not null,
  char_offset  bigint not null check (char_offset >= 0),
  note         text check (char_length(note) <= 2000),
  created_at   timestamptz not null default now(),
  foreign key (document_id, user_id) references public.documents (id, user_id) on delete cascade
);
create index bookmarks_by_document on public.bookmarks (user_id, document_id);

-- --------------------------------------------------------------------------------------------------------- voices

-- The user's own (cloned) voices. The Worker creates the row and mirrors the preparation state from its render
-- pipeline (D1), which also keeps the reference clip's private details (R2 key, kept transcript span).
create table public.voices (
  id          public.nolabs_id primary key,
  user_id     uuid not null default auth.uid() references auth.users (id) on delete cascade,
  name        text not null check (char_length(btrim(name)) between 1 and 60),
  kind        text not null default 'clone' check (kind in ('clone', 'designed')),
  state       text not null default 'awaiting_reference'
              check (state in ('awaiting_reference', 'preparing', 'ready', 'failed')),
  transcript  text not null check (char_length(transcript) between 1 and 4000),
  ref_sha256  text check (ref_sha256 ~ '^[0-9a-f]{64}$'),
  created_at  timestamptz not null default now(),
  updated_at  timestamptz not null default now()
);
create index voices_by_user on public.voices (user_id, created_at);
create trigger voices_touch before update on public.voices
  for each row execute function private.touch_updated_at();

-- -------------------------------------------------------------------------------------------------- budget_shares

-- Each user's share of the daily ZeroGPU budget (seconds as charged by the scheduler). NULL means "the rest of the
-- cap after every other user's share" and is what the admin gets; invited users default to 60 s.
create table public.budget_shares (
  user_id      uuid primary key references auth.users (id) on delete cascade,
  daily_gpu_s  integer default 60 check (daily_gpu_s is null or daily_gpu_s between 0 and 300),
  updated_at   timestamptz not null default now()
);
create trigger budget_shares_touch before update on public.budget_shares
  for each row execute function private.touch_updated_at();

-- ---------------------------------------------------------------------------------------- privileges and policies

-- Supabase grants every privilege on new public tables to anon and authenticated by default. Start from nothing and
-- grant only what each table needs; anon gets no table access at all.
revoke all on table public.profiles, public.invites, public.documents, public.progress, public.bookmarks,
  public.voices, public.budget_shares from anon, authenticated;

alter table public.profiles enable row level security;
alter table public.invites enable row level security;
alter table public.documents enable row level security;
alter table public.progress enable row level security;
alter table public.bookmarks enable row level security;
alter table public.voices enable row level security;
alter table public.budget_shares enable row level security;

-- profiles: read and edit your own settings; is_admin and the timestamps are not writable.
grant select on table public.profiles to authenticated;
grant update (display_name, locale, default_voice, default_speed, theme) on table public.profiles to authenticated;
create policy profiles_select_own on public.profiles for select to authenticated
  using (id = (select auth.uid()));
create policy profiles_update_own on public.profiles for update to authenticated
  using (id = (select auth.uid())) with check (id = (select auth.uid()));

-- invites: only admins see and manage them; everyone else goes through invite_status() and the sign-up hook.
grant select, insert, delete on table public.invites to authenticated;
create policy invites_admin_select on public.invites for select to authenticated
  using (exists (select 1 from public.profiles p where p.id = (select auth.uid()) and p.is_admin));
create policy invites_admin_insert on public.invites for insert to authenticated
  with check (created_by = (select auth.uid())
              and exists (select 1 from public.profiles p where p.id = (select auth.uid()) and p.is_admin));
create policy invites_admin_delete on public.invites for delete to authenticated
  using (used_by is null
         and exists (select 1 from public.profiles p where p.id = (select auth.uid()) and p.is_admin));

-- documents, progress, bookmarks, voices: owner-only, every operation.
grant select, insert, update, delete on table public.documents, public.progress, public.bookmarks, public.voices
  to authenticated;
create policy documents_owner on public.documents for all to authenticated
  using (user_id = (select auth.uid())) with check (user_id = (select auth.uid()));
create policy progress_owner on public.progress for all to authenticated
  using (user_id = (select auth.uid())) with check (user_id = (select auth.uid()));
create policy bookmarks_owner on public.bookmarks for all to authenticated
  using (user_id = (select auth.uid())) with check (user_id = (select auth.uid()));
create policy voices_owner on public.voices for all to authenticated
  using (user_id = (select auth.uid())) with check (user_id = (select auth.uid()));

-- budget_shares: read your own share; changed only by the owner in SQL.
grant select on table public.budget_shares to authenticated;
create policy budget_shares_select_own on public.budget_shares for select to authenticated
  using (user_id = (select auth.uid()));

-- ------------------------------------------------------------------------------------------- invite-only sign-up

-- Mark an invite used by `user_id`. True when the code was valid and is now spent; the update is the only check, so
-- two sign-ups racing for one code cannot both succeed.
create or replace function private.redeem_invite(code text, user_id uuid) returns boolean
language plpgsql security definer set search_path = '' as $$
declare
  spent integer;
begin
  update public.invites i
     set used_by = redeem_invite.user_id, used_at = now()
   where i.code = private.normalise_invite(redeem_invite.code)
     and i.used_by is null
     and i.expires_at > now();
  get diagnostics spent = row_count;
  return spent = 1;
end;
$$;
revoke all on function private.redeem_invite(text, uuid) from public, anon, authenticated;

-- "Before user created" auth hook (pg-functions://postgres/private/hook_before_user_created): refuses sign-ups
-- without a usable invite with a readable 403 instead of the generic database error the trigger would give.
create or replace function private.hook_before_user_created(event jsonb) returns jsonb
language plpgsql security definer set search_path = '' as $$
declare
  given text := private.normalise_invite(event -> 'user' -> 'user_metadata' ->> 'invite_code');
begin
  if given = '' then
    return jsonb_build_object('error', jsonb_build_object(
      'http_code', 403, 'message', 'nolabs is invite-only: sign up with an invite code'));
  end if;
  if not exists (select 1 from public.invites i
                  where i.code = given and i.used_by is null and i.expires_at > now()) then
    return jsonb_build_object('error', jsonb_build_object(
      'http_code', 403, 'message', 'this invite code is not valid, already used or expired'));
  end if;
  return '{}'::jsonb;
end;
$$;
revoke all on function private.hook_before_user_created(jsonb) from public, anon, authenticated;
grant execute on function private.hook_before_user_created(jsonb) to supabase_auth_admin;

-- After a user row is inserted: spend the invite (atomically, failing the whole sign-up when it was taken in the
-- meantime), then create the profile and the budget share.
create or replace function private.handle_new_user() returns trigger
language plpgsql security definer set search_path = '' as $$
declare
  wanted_locale text := new.raw_user_meta_data ->> 'locale';
begin
  if not private.redeem_invite(new.raw_user_meta_data ->> 'invite_code', new.id) then
    raise exception 'nolabs is invite-only: the invite code is not valid, already used or expired'
      using errcode = 'P0001';
  end if;
  if wanted_locale is null or wanted_locale !~ '^[a-z]{2,3}(-[A-Za-z0-9]{2,8})*$' then
    wanted_locale := 'en';
  end if;
  insert into public.profiles (id, display_name, locale)
  values (new.id, nullif(left(btrim(coalesce(new.raw_user_meta_data ->> 'display_name', '')), 80), ''),
          wanted_locale);
  insert into public.budget_shares (user_id) values (new.id);
  return new;
end;
$$;
revoke all on function private.handle_new_user() from public, anon, authenticated;
create trigger on_auth_user_created after insert on auth.users
  for each row execute function private.handle_new_user();

-- --------------------------------------------------------------------------------------------- RPC for the apps

-- Whether a typed invite code can be used, so the sign-in screen can say so before sending an email.
-- Answers 'valid', 'used', 'expired' or 'unknown'.
create or replace function public.invite_status(code text) returns text
language sql stable security definer set search_path = '' as $$
  select coalesce((
    select case when i.used_by is not null then 'used'
                when i.expires_at <= now() then 'expired'
                else 'valid' end
      from public.invites i where i.code = private.normalise_invite(invite_status.code)), 'unknown')
$$;
revoke all on function public.invite_status(text) from public;
grant execute on function public.invite_status(text) to anon, authenticated;

-- An admin creates an invite that expires after `days` days; returns the code.
create or replace function public.create_invite(days integer default 14, note text default null) returns text
language plpgsql security definer set search_path = '' as $$
declare
  made text;
begin
  if not exists (select 1 from public.profiles p where p.id = auth.uid() and p.is_admin) then
    raise exception 'only admins can create invites' using errcode = '42501';
  end if;
  if days is null or days < 1 or days > 90 then
    raise exception 'days must be between 1 and 90' using errcode = '22023';
  end if;
  insert into public.invites (created_by, note, expires_at)
  values (auth.uid(), left(create_invite.note, 200), now() + make_interval(days => create_invite.days))
  returning code into made;
  return made;
end;
$$;
revoke all on function public.create_invite(integer, text) from public, anon;
grant execute on function public.create_invite(integer, text) to authenticated;

-- The caller's effective daily GPU share in seconds, given the deployment's cap. Admins get the cap minus every
-- non-admin share (a missing row counts as the 60 s default); everyone else gets their own share, at most the cap.
-- The Worker calls it with the user's token when it admits a Ming render.
create or replace function public.gpu_share(daily_cap_s integer) returns integer
language sql stable security definer set search_path = '' as $$
  select case
    when p.is_admin then greatest(0, gpu_share.daily_cap_s - coalesce((
      select sum(coalesce(b.daily_gpu_s, 60))::integer
        from public.profiles q left join public.budget_shares b on b.user_id = q.id
       where not q.is_admin), 0))
    else least(gpu_share.daily_cap_s, coalesce((
      select b.daily_gpu_s from public.budget_shares b where b.user_id = p.id), 60))
  end
  from public.profiles p where p.id = auth.uid()
$$;
revoke all on function public.gpu_share(integer) from public, anon;
grant execute on function public.gpu_share(integer) to authenticated;

-- ------------------------------------------------------------------------------------------------------- realtime

-- Devices subscribe to their own progress and library changes (RLS applies to Realtime postgres_changes).
alter publication supabase_realtime add table public.progress, public.documents;
