
create table if not exists prompt (
  id          integer primary key,
  source      text not null,
  received_at text not null,
  prompt      text,                -- null = the mapping found no prompt (drift signal)
  fields      text not null,       -- JSON: values extracted from raw by the source spec
  context     text not null,       -- JSON: observed by GitPrompt at ingest (git state, tool version)
  raw         text not null        -- JSON: the payload exactly as received
);
create unique index if not exists prompt_dedup on prompt (source, json_extract(fields, '$.prompt_id'));
create index if not exists prompt_recent on prompt (received_at desc);
create index if not exists prompt_turn on prompt (json_extract(fields, '$.prompt_id'));

-- Every other event (tool calls, turn end, telemetry...). Joined to prompts by prompt_id/session_id.
create table if not exists event (
  id          integer primary key,
  source      text not null,
  received_at text not null,
  event       text,
  session_id  text,
  prompt_id   text,
  fields      text not null,
  raw         text not null        -- long strings are clipped (see core/payload.py clip)
);
create index if not exists event_prompt on event (prompt_id);
create index if not exists event_session on event (session_id);

-- The user's own notes on a prompt: {starred, rating, tags, note}.
create table if not exists annotation (id integer primary key, data text not null);

create index if not exists prompt_session on prompt (source, coalesce(json_extract(fields, '$.session_id'), ''));
drop view if exists prompt_view;
create view prompt_view as 
select p.*, coalesce(a.data, '{}') annotation,
       row_number() over w turn,
       lag(p.received_at) over w prev_at,
       lead(json_extract(p.context, '$.git_head')) over w next_head
from prompt p left join annotation a on a.id = p.id
where 1=1
window w as (partition by p.source, coalesce(json_extract(p.fields, '$.session_id'), '') order by p.id);
