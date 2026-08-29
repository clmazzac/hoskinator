# Google Sheets sync decisions

Decisions behind the OAuth-based live sync between the Applications tracker and a linked Google
Sheet. Repo-wide decisions live in `CLAUDE.md`; architectural ones in `docs/adr/`.

The feature ships in stages. Stage 1 (OAuth connection plumbing) is built; Stages 2–3 (reading and
writing spreadsheet data) are decided ahead of time so each stage's own review does not re-litigate
them, but neither is built yet — each still gets its own plan and review pass first.

## Reversing "no Google Sheets API" (Stage 1)

`docs/decisions/workspace.md` chose a public-CSV-export importer over the Sheets API specifically
to avoid OAuth, a client secret, and token storage. This reverses that choice: applications now
sync live, in both directions, which a public read-only export link cannot do.

**Why now:** the CSV importer only reads, and only on request. A tracker a user edits by hand in a
spreadsheet needs the sheet and the store to stay in step without a manual export/import step each
time.

**What stays:** the CSV importer (`crates/core/src/sheets.rs`) is untouched, for anyone who never
connects a Google account.

## The client id and secret are the user's own (Stage 1)

Google's OAuth client id and secret are entered through `google.set_credentials`, from a Google
Cloud project the user creates themselves — never compiled into the Hoskinator binary.

**Why:** `docs/decisions/workspace.md` rejected an OAuth flow of Hoskinator's own partly because it
would mean "holding a secret in a binary anyone can read." A per-user client sidesteps that: the
secret belongs to the user's own project, the same trust boundary as their `gh` login or their
Anthropic API key.

## The refresh token is plaintext in `config.toml`, not a keychain (Stage 1)

`google_refresh_token` follows the exact precedent `anthropic_api_key` set: plaintext on disk, file
mode 0600, never round-tripped back to the client. See `docs/decisions/braindump.md`.

**Accepted cost:** a refresh token grants ongoing write access to the linked sheet, longer-lived
than an API key someone can rotate at will. An OS keychain (the `keyring` crate) would close that
gap, but it is a repo-wide security-model change — it would apply equally to `anthropic_api_key` —
and this feature does not make that call alone.

## No persistent session store for the in-flight OAuth flow (Stage 1)

The `state`/PKCE verifier pending between `google.begin_auth` and the loopback callback lives in
memory (`PendingGoogleAuth`, mirroring `ActiveRepository`), not in SQLite. It expires after five
minutes and does not survive a daemon restart.

**Why:** the flow is seconds long, single-user, and single-browser. A migration for state this
short-lived would outlive the data it holds.

## `google.status` caches the account email in memory (Stage 1)

The signed-in email is fetched once, right after token exchange, and cached in memory
(`GoogleAccountCache`) rather than re-fetched on every `google.status` call.

**Why:** `docs/decisions/workspace.md` already names the cost of the alternative — `workspace.status`
calls `gh api user` on every call, at roughly 700ms, which made every application list, create,
update, and delete pay for two fields it never read. `google.status` avoids repeating that mistake.

**Accepted cost:** a restart clears the cache, so the display email is blank until the next
`google.status` call refreshes it once (a single refresh-token exchange), which then repopulates the
cache for the rest of the daemon's life.

## Upgrading a pre-existing CSV-linked sheet auto-matches by company and position (Stage 2)

When live sync first reconciles a sheet that predates its Id column, a blank-Id row is matched
against an existing application by an exact `(company, position)` match; only a genuinely
unmatched row becomes a new application.

**Accepted cost:** two different applications that happen to share a company and position collapse
into one link on this one-time upgrade. Given the tracker already treats `(company, position)` as
close to a natural key elsewhere (`web/src/lib/sheet.ts`'s CSV import filters out rows missing
either), this is treated as acceptable rather than requiring a confirmation step for every match.

## Same-field edits within one poll window: the sheet wins, silently (Stage 3)

Each poll tick pulls sheet rows into the store first, then pushes the reconciled records back. If
the same field of the same application changed in both places within one ~30-second window, the
sheet's value overwrites the local edit with no warning.

**Why no `updated_at` migration:** `update_application` is already a full-record replace with no
patch or version semantics, so "what did the user actually just change locally" is ambiguous even
with a timestamp. A migration is permanent — "a migration is never edited once it has shipped"
(`crates/core/migrations/`) — and this feature declines to add one for a comparison only the sync
loop itself needs.

**Accepted cost:** genuine same-field, same-window edits in both places lose the local edit
silently. For a personal, single-user tracker, this is judged unlikely enough to accept rather than
build real last-writer-wins.
