# Home and config decisions

Decisions about where Hoskinator's data lives and how it is configured, newest last. Repo-wide
decisions live in `CLAUDE.md`; architectural ones in `docs/adr/`.

## The store lives in Home; the resume repo does not, but defaults under Home (Slice 1, #2; resume-builder-ui)

Home holds the store, at `store/hoskinator.db` — its own directory, because SQLite writes `-wal`
and `-shm` files beside the database and backups may join them later. The rendercv git repository
is located separately, by configuration, and is never forced under Home.

**Why the repo stays a location of its own:** it is the user's own artifact, which they will want
to `cd` into, run `git` in, and push to their own remote. ADR-0001's "database beside the repo" is
satisfied by the two being independently located, and the repo stays vanilla either way.

**Why the *default* destination is `Home::repositories_dir()`** (`<Home>/repositories/<name>`),
rather than a path under the OS home directory: the web UI once suggested a hardcoded developer
path (`/home/cam/<name>`) as that default, which is wrong for anyone else. Home is already
resolved per-platform and is guaranteed to exist or be creatable, so it makes a default that works
on the first run without asking. The user is always free to connect an existing repository
anywhere else; only the "create a new one" default lives under Home now.

**Accepted cost:** two paths to resolve rather than one. First run no longer needs to ask where the
repo goes — it has a default — but a user who wants their resumes somewhere more visible than app
data has to say so via "Connect an existing one" or by moving the repo and updating `resume_repo`.

## Platform paths via `directories` (Slice 1, #2)

Native paths per platform (XDG on Linux, `Application Support` on macOS, `%APPDATA%` on Windows)
rather than forcing XDG everywhere, since ADR-0003 makes multi-device a v1 goal.

**Accepted cost:** no single documented path.

## The config file is TOML, with `deny_unknown_fields` (Slice 1, #2)

**Why `deny_unknown_fields`:** a typo is an error rather than a setting that silently does nothing.
