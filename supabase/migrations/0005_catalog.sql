-- Version 5: documents added from the Explore catalogue (notes/23-explore-catalog.md).
-- NOT APPLIED; apply with `cargo xtask supabase-migrate` once the app and Worker send `catalog_id`.
--
-- A catalogue book keeps source = 'epub' and source_url = its standardebooks.org URL. `catalog_id` is the catalogue
-- entry id (the Standard Ebooks GitHub repo name, e.g. 'jane-austen_pride-and-prejudice'); it lets "Add to library"
-- find an existing copy instead of adding a duplicate, and lets a withdrawn title (HTTP 451) be flagged in libraries.

alter table public.documents
  add column catalog_id text check (catalog_id ~ '^[a-z0-9]+(-[a-z0-9]+)*(_[a-z0-9]+(-[a-z0-9]+)*)+$'
                                    and char_length(catalog_id) <= 200);

create unique index documents_catalog_once on public.documents (user_id, catalog_id) where catalog_id is not null;
