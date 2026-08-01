# Home and config decisions

Decisions about where Hoskinator's data lives and how it is configured, newest last. Repo-wide
decisions live in `CLAUDE.md`; architectural ones in `docs/adr/`.

## The store lives in Home; the resume repo does not (Slice 1, #2)

Home holds only the store, at `store/hoskinator.db` — its own directory, because SQLite writes
`-wal` and `-shm` files beside the database and backups may join them later. The rendercv git
repository is located separately, by configuration.

**Why:** the store is tool-managed and opaque, so burying it in a platform directory is right. The
repo is the opposite — it is the user's own artifact, which they will want to `cd` into, run `git`
in, and push to their own remote. ADR-0001's "database beside the repo" is satisfied by the two
being independently located, and the repo stays vanilla either way.

**Accepted cost:** two paths to resolve rather than one, and first run must either ask where the
repo goes or create a default.

## Platform paths via `directories` (Slice 1, #2)

Native paths per platform (XDG on Linux, `Application Support` on macOS, `%APPDATA%` on Windows)
rather than forcing XDG everywhere, since ADR-0003 makes multi-device a v1 goal.

**Accepted cost:** no single documented path.

## The config file is TOML, with `deny_unknown_fields` (Slice 1, #2)

**Why `deny_unknown_fields`:** a typo is an error rather than a setting that silently does nothing.
