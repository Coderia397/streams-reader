# 27. Catalogue rights by region, and upload safety in tools/catalog

Kind: finding + decision + reference. Fixes for five reviewer defects in the Explore catalogue builder. Conclusion: 8 of the 66 titles are still in copyright under life + 70 and are now marked `region: "us"`. The Worker must serve them to U.S. requests only; until it does, the live R2 index (built before this change) has no region data. `--upload` can no longer publish a partial, unchecked or failed index.

Corrects note 23 section 4 (the Blue Castle / We caveat is superseded by this note).

## 2026-10-31

### 1. Titles in copyright under life + 70 (death years: assumed from reference knowledge, 3 translators web-checked)
Life + 70 protection runs to the end of the 70th year after the death of the last-surviving author, translator or illustrator, so a title is public domain there from `died + 71`. In 2026 that means everyone in the edition must have died in 1955 or earlier.

| Title | Repo | Last death | Protected to the end of | Region |
|---|---|---|---|---|
| The Mysterious Affair at Styles | agatha-christie_the-mysterious-affair-at-styles | Agatha Christie 1976 | 2046 | us |
| Poetry | t-s-eliot_poetry | T. S. Eliot 1965 | 2035 | us |
| The Problems of Philosophy | bertrand-russell_the-problems-of-philosophy | Bertrand Russell 1970 | 2040 | us |
| A Room With a View | e-m-forster_a-room-with-a-view | E. M. Forster 1970 | 2040 | us |
| The Souls of Black Folk | w-e-b-du-bois_the-souls-of-black-folk | W. E. B. Du Bois 1963 | 2033 | us |
| The Worst Journey in the World | apsley-cherry-garrard_the-worst-journey-in-the-world | Apsley Cherry-Garrard 1959 | 2029 | us |
| The Art of War | sun-tzu_the-art-of-war_lionel-giles | translator Lionel Giles 1958 | 2028 | us |
| We | yevgeny-zamyatin_we_gregory-zilboorg | translator Gregory Zilboorg 1959 (Zamyatin 1937) | 2029 | us |

- Correction to note 23: *The Blue Castle* is public domain under life + 70 (L. M. Montgomery died 1942, free since 2013). For *We* the term runs from the translator, not Zamyatin.
- A global "check your local laws" notice does not make it lawful for the service itself to send these titles from R2 to UK or EU users. The notice stays as information only.
- Verified by web search on 2026-10-31: W. K. Marriott died 1927, William Robson 1863, F. A. Malleson 1897 (not 1903 as first assumed), G. M. Towle 1893. W. A. McDevitte and W. S. Bohn (Caesar, 1869 translation): no dates found; `died = 1900, died_approx = true` is an upper bound (assumed: translators active in 1851 cannot have lived past 1955).
- Assumed, not checked: some countries have longer terms than life + 70 (Mexico life + 100, Colombia life + 80, Côte d'Ivoire life + 99). `region: "world"` means life + 70 only. `last_death` is in every entry so the Worker can apply a longer term per country if the owner wants that.
- Assumed: the EU and UK "comparison of terms" rule might make some U.S.-origin works (Du Bois, the Zilboorg translation) free there earlier. Not relied on; the conservative life + 70 rule is used.
- Cover paintings are not tracked in `died` (assumed public domain, as Standard Ebooks states for its covers).

### 2. What changed (verified)
- `tools/catalog/shelves.toml`: every pick has `died` (required). `secat/curation.py` rejects a pick without an integer `died`; `Pick.world_from = died + 71`, `Pick.region(year)` is `"world"` or `"us"`.
- `secat/catalog.py`: each entry carries `region`, `world_from`, `last_death`. The index has a top-level `us_only` list of ids, sorted.
- `build.py`:
  - `--upload --only …` exits 2: "--only cannot be combined with --upload (it would replace the live index with a partial one)".
  - `--upload --no-status-check` exits 2: "--upload requires the 451 status check".
  - Failures are checked before the upload. With any failed title, nothing is uploaded and the exit code is 1, unless `--allow-partial` is given (it uploads and still exits 1). Withdrawn (451) titles are marked `withdrawn: true` in `report.json` and do not count as failures.
  - Each title's fetch and its cover/stage step are wrapped in a per-title `except Exception`. A failed title's objects never enter the upload plan (`stage_book` returns them only on success).
- `secat/sources.py`: one `_request` retry loop (4 attempts) for the API and raw files. It retries `httpx.TransportError`, 5xx, 429, and a 403 that carries `Retry-After` (secondary rate limit, wait capped at 60 s). A primary rate limit (`x-ratelimit-remaining: 0`) and other non-2xx answers raise `SourceError("GitHub HTTP <code>: <path>")`. Invalid JSON is a `SourceError`. `StatusChecker.status` retries transport errors 3 times and turns any `httpx.HTTPError` into `SourceError`.

### 3. Checks (verified)
- `cd tools/catalog && uv run --no-project --with pytest --with httpx --with pillow --with resvg-py pytest -q tests`: 43 passed, 1 xfailed (the known importer author bug, note 23 section 6). New: `tests/test_sources.py` (11 tests on a mock transport), `tests/test_build.py` (12 tests with fakes for GitHub, status, converter, covers, R2), and 3 rights tests in `test_metadata.py`. One test pins exactly the 8 titles above as `us` for 2026, and *The Blue Castle* as `world`.
- `uvx ruff check` and `ruff format` (line length 120): clean.
- Real CLI, 2026-10-31: `--upload --only t-s-eliot_poetry` exit 2; `--upload --no-status-check` exit 2; `--only kahlil-gibran_the-prophet,t-s-eliot_poetry` (no upload) built 2/2 in 14 s against GitHub and standardebooks.org, index `us_only = ["t-s-eliot_poetry"]`, Eliot `world_from 2036`, Gibran `region world`. With `GITHUB_TOKEN=invalid` the title fails with "GitHub HTTP 401: /repos/standardebooks/t-s-eliot_poetry" and the run exits 1; before this change the 401 raised `httpx.HTTPStatusError` and aborted the run. Scratch output deleted.
- Not done: no upload to R2. The live `catalog/v1/catalog.json` is still the note 23 build, with no `region` or `us_only`.

### 4. Request to the Worker owner (open)
For `GET /api/catalog` and `GET /api/catalog/{rest}` (note 23 section 5, request 1):
- Read `CF-IPCountry`. If it is not `US` (including `XX` and a missing header), remove every book whose id is in `us_only` from `shelves[].books` before answering, and answer 404 for `books/{id}/…` and `covers/{id}-…` when `{id}` is in `us_only`. The id is the path segment after `books/`, or the part of the cover name before `-(200|400)-<sha8>`.
- Send `Vary: CF-IPCountry` with the index response (or `Cache-Control: private`), so a shared cache does not hand the U.S. index to other countries.
- "Add to library" (`from-catalog`) must apply the same check, because it copies the body server side.
- Alternative the owner may choose instead: drop the 8 titles from `shelves.toml`.

### 5. Decisions (assistant, under the reviewer's fix list; 2026-10-31)
- D-cat-6: store the death year (`died`), not a boolean. Reason: it can be audited against a source and the region expires by itself (Giles becomes `world` in 2029). Rejected: `pd_outside_us = true/false`, which needs a manual edit every year.
- D-cat-7: refuse `--only` with `--upload`. Rejected: download the live index and merge the rebuilt entries, which adds a read-modify-write on production for a case the full build already covers.

### Open items
- Worker region filter (section 4). Unblocked by the Worker owner; needed before Explore ships outside the U.S.
- Re-run the full build with `--upload` so the live index carries `region` and `us_only`. Unblocked by the owner's go-ahead (it writes production); about 200 s with a warm cache, 400 s cold.
- Owner decision: keep the 8 titles U.S.-only, or drop them from the shelves.
- Longer national terms (Mexico, Colombia and others, section 1). Open; needs an owner decision on whether to handle them.
