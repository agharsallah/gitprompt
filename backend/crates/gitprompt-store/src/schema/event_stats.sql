-- Rows, stored bytes and the newest time per (source, event), kept exact by triggers, so the Data
-- page and the Connectors page read a few rows instead of scanning every stored event. New in the
-- Rust port and additive: the Python schema is untouched, and the triggers also count rows the
-- legacy Python daemon writes. A cache: `Store::rebuild_event_stats` recomputes it from `event`.
-- `event` NULL (an event the mapping found no name for) is its own group, as in `event is ?`.
create table event_stats (
  source  text not null,
  event   text,
  rows    integer not null,
  bytes   integer not null,   -- length(fields) + length(raw), as bytes: what `event_usage` reports
  last_at text                -- newest received_at; exact for inserts and for deletions by age
);
create unique index event_stats_key on event_stats (source, ifnull(event, char(0)));

create trigger event_stats_insert after insert on event begin
  insert into event_stats (source, event, rows, bytes, last_at)
  values (new.source, new.event, 1, length(cast(new.fields as blob)) + length(cast(new.raw as blob)), new.received_at)
  on conflict (source, ifnull(event, char(0))) do update
    set rows = rows + 1, bytes = bytes + excluded.bytes, last_at = max(coalesce(last_at, ''), excluded.last_at);
end;

create trigger event_stats_delete after delete on event begin
  update event_stats set rows = rows - 1, bytes = bytes - (length(cast(old.fields as blob)) + length(cast(old.raw as blob)))
  where source = old.source and event is old.event;
  delete from event_stats where source = old.source and event is old.event and rows <= 0;
end;

create trigger event_stats_update after update of source, event, fields, raw on event begin
  update event_stats set rows = rows - 1, bytes = bytes - (length(cast(old.fields as blob)) + length(cast(old.raw as blob)))
  where source = old.source and event is old.event;
  delete from event_stats where source = old.source and event is old.event and rows <= 0;
  insert into event_stats (source, event, rows, bytes, last_at)
  values (new.source, new.event, 1, length(cast(new.fields as blob)) + length(cast(new.raw as blob)), new.received_at)
  on conflict (source, ifnull(event, char(0))) do update
    set rows = rows + 1, bytes = bytes + excluded.bytes, last_at = max(coalesce(last_at, ''), excluded.last_at);
end;
