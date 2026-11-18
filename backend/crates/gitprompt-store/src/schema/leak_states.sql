-- What you decided about a possible leak (`prompt_index_leak.finding`): `hidden` (its value was
-- replaced in the stored text) or `not_sensitive`. Durable user data like `names`, not part of the
-- rebuildable prompt index: `gitprompt reindex` keeps it, and finding ids are stable across it.
-- `kind` and `masked` are kept so a hidden finding is still listed once its value is gone.
create table if not exists leak_state (
  finding   text primary key,
  value_key text not null,
  kind      text not null,
  masked    text not null,
  place     text not null,
  prompt    integer not null,
  state     text not null,
  at        text not null
) without rowid;
create index if not exists leak_state_value on leak_state (value_key);
