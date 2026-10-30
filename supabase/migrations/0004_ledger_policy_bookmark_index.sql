-- Version 4: the two remaining advisor notes after 0003 (2026-10-30).

-- bookmarks_by_document now leads with document_id, which leaves the plain user_id foreign key (account deletion)
-- without a covering index.
create index bookmarks_by_user on public.bookmarks (user_id);

-- The migration ledger is for the postgres role only; say so with a deny-all policy.
create policy nolabs_migrations_no_access on private.nolabs_migrations as restrictive for all to public
  using (false) with check (false);
