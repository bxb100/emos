-- Add migration script here
create table if not exists video
(
    todb_id     INTEGER primary key,
    tmdb_id     INTEGER not null,
    video_id    INTEGER not null,
    video_type  TEXT,
    video_title TEXT,
    genres      TEXT
);
