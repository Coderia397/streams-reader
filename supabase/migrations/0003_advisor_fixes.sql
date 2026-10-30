-- Version 3: the Supabase advisors' findings after 0002 (2026-10-30).

-- Foreign keys get covering indexes in the key's column order, so cascading deletes of a document or an account
-- do not scan whole tables.
drop index if exists public.bookmarks_by_document;
create index bookmarks_by_document on public.bookmarks (document_id, user_id);
create index progress_by_document on public.progress (document_id, user_id);
create index invites_by_creator on public.invites (created_by);

-- private.allowed_emails is reachable by no role through PostgREST; state the deny-all intent as a policy instead of
-- leaving RLS on with no policy (which reads like an oversight).
create policy allowed_emails_no_access on private.allowed_emails as restrictive for all to public
  using (false) with check (false);
