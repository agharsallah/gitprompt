
create table if not exists index_meta (key text primary key, value text not null);

create table if not exists prompt_index (
  id            integer primary key,   -- prompt.id
  prompt_id     text,
  source        text not null,
  session       text,
  repo          text,                  -- git root at ingest, else the working directory
  repo_name     text,                  -- owner/name of the origin remote, when there is one
  branch        text,                  -- git branch at ingest
  title         text,                  -- the session's title, when the tool reported one
  received_at   text not null,         -- UTC, as stored on the prompt
  day           text not null,         -- local calendar day of the daemon's machine, YYYY-MM-DD
  hour          integer not null,      -- local hour, 0-23
  weekday       integer not null,      -- local ISO weekday, 1 = Monday
  origin        text not null,
  chars         integer not null,      -- size of the typed text (the text itself is in prompt_fts)
  text_hash     text,                  -- the typed text, normalized and hashed: equal hashes = a reused prompt
  slash_command text,                  -- with its leading slash
  code_blocks   integer not null,      -- in the typed text
  urls          integer not null,
  mentions      integer not null,      -- @-mentions
  secrets       integer not null,      -- kinds of credentials found in the prompt as received
  api_requests  integer not null,
  cost_usd      real not null,
  input_tokens  integer not null,
  output_tokens integer not null,
  cache_read_tokens     integer not null,
  cache_creation_tokens integer not null,
  api_time_s    real not null,         -- time spent in model calls (telemetry)
  api_errors    integer not null,
  response_s    real,                  -- null while the turn has no end
  ended_by      text,                  -- Stop, StopFailure, Interrupt, or null
  tool_calls    integer not null,
  tool_failures integer not null,
  tool_time_s   real not null,         -- summed tool durations (parallel calls add up)
  subagent_tool_calls integer not null,
  subagents     integer not null,      -- subagents started
  files_written integer not null,      -- distinct files the turn wrote
  git_commits   integer not null,      -- git commits the agent ran
  compactions   integer not null,
  denials       integer not null,      -- tool calls the permission system denied
  rejections    integer not null,      -- tool calls you rejected (telemetry)
  model         text,                  -- the turn's main model: most model calls, else the one its hooks reported
  permission_mode text,                -- as the tool reported it with the prompt
  tool_version  text,                  -- the tool's version when the prompt was sent
  reasoning_tokens integer not null,   -- output tokens spent reasoning, when the tool reports them
  answered      integer not null,      -- 1 when the turn's end carried the final answer
  surface       text                   -- where it was typed (insights/surface.py), null = unknown
);
create index if not exists prompt_index_time on prompt_index (received_at);
create index if not exists prompt_index_origin on prompt_index (origin, received_at);
create index if not exists prompt_index_session on prompt_index (session, id);
create index if not exists prompt_index_repo on prompt_index (repo);

create table if not exists prompt_index_model (
  id integer not null, model text, api_requests integer not null, cost_usd real not null,
  input_tokens integer not null, output_tokens integer not null,
  cache_read_tokens integer not null, cache_creation_tokens integer not null
);
create index if not exists prompt_index_model_id on prompt_index_model (id);

create table if not exists prompt_index_tool (id integer not null, tool text, kind text not null, calls integer not null);
create index if not exists prompt_index_tool_id on prompt_index_tool (id);

create table if not exists prompt_index_work (
  id integer not null, work text not null, query_source text, api_requests integer not null, cost_usd real not null,
  input_tokens integer not null, output_tokens integer not null,
  cache_read_tokens integer not null, cache_creation_tokens integer not null
);
create index if not exists prompt_index_work_id on prompt_index_work (id);

-- Telemetry that arrives per session (event_id = event.id): read incrementally, after the last one.
create table if not exists session_metric (
  event_id integer primary key, session text, received_at text not null, name text not null, value real not null
);
create index if not exists session_metric_session on session_metric (session, received_at);
create table if not exists session_command (
  event_id integer primary key, session text, received_at text not null, command text not null, source text
);
create index if not exists session_command_session on session_command (session, received_at);

-- Turns that received events since they were indexed: recomputed on the next read.
create table if not exists index_dirty (prompt_id text primary key);

create virtual table if not exists prompt_fts using fts5(typed_text, tokenize = 'trigram');
