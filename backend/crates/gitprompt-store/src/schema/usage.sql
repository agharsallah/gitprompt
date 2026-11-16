-- How often each dashboard feature is used, per day, on this machine only (nothing leaves it).
-- Durable user data like `annotation`, not part of the rebuildable prompt index: `gitprompt reindex`
-- and every clean-up leave it alone. `day` is `YYYY-MM-DD` (UTC, from the app clock).
create table if not exists usage_counts (
  action text not null,
  day    text not null,
  n      integer not null,
  primary key (action, day)
) without rowid;
