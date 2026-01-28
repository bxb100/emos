CREATE TABLE IF NOT EXISTS genre (
    id INTEGER PRIMARY KEY,
    name TEXT NOT NULL UNIQUE
);

CREATE TABLE IF NOT EXISTS video_genre (
    video_id INTEGER NOT NULL,
    genre_id INTEGER NOT NULL,
    PRIMARY KEY (video_id, genre_id),
    FOREIGN KEY (video_id) REFERENCES video(todb_id) ON DELETE CASCADE,
    FOREIGN KEY (genre_id) REFERENCES genre(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_video_genre_genre_id_video_id ON video_genre(genre_id, video_id);

-- Migrate existing genres
INSERT OR IGNORE INTO genre (id, name)
SELECT DISTINCT
    json_extract(value, '$.id'),
    json_extract(value, '$.name')
FROM video, json_each(genres);

-- Migrate existing video-genre relationships
INSERT OR IGNORE INTO video_genre (video_id, genre_id)
SELECT
    video.todb_id,
    json_extract(value, '$.id')
FROM video, json_each(genres);
