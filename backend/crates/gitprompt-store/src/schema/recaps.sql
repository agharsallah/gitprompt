-- The weekly recaps the daemon handled: `due` is the scheduled instant (ISO-8601, UTC) whose week
-- was summed up, `notification` what was sent (JSON), null when the week held nothing to say.
-- Durable like `names`, not part of the rebuildable prompt index: `gitprompt reindex` keeps it, so
-- a restart never sends a week's recap twice.
create table if not exists recaps (
  due          text primary key,
  notification text
) without rowid;
