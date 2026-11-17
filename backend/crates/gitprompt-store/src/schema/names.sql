-- The names you gave things (a session now, a chapter later): `kind` is a `NameKind`, `id` what it
-- names (a session id). Durable user data like `annotation` and `usage_counts`, not part of the
-- rebuildable prompt index: `gitprompt reindex` and every clean-up leave it alone.
create table if not exists names (
  kind text not null,
  id   text not null,
  name text not null,
  primary key (kind, id)
) without rowid;
