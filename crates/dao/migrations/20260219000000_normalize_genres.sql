-- Create genre table
CREATE TABLE IF NOT EXISTS genre (
    id INTEGER PRIMARY KEY,
    name TEXT NOT NULL UNIQUE
);

-- Create video_genre table
CREATE TABLE IF NOT EXISTS video_genre (
    todb_id INTEGER NOT NULL,
    genre_id INTEGER NOT NULL,
    PRIMARY KEY (todb_id, genre_id),
    FOREIGN KEY (todb_id) REFERENCES video(todb_id) ON DELETE CASCADE,
    FOREIGN KEY (genre_id) REFERENCES genre(id) ON DELETE CASCADE
);

-- Index for filtering by genre_id efficiently
CREATE INDEX IF NOT EXISTS idx_video_genre_genre_todb ON video_genre(genre_id, todb_id);

-- Migrate data
-- Insert unique genres
INSERT OR IGNORE INTO genre (id, name)
SELECT
    json_extract(json_each.value, '$.id'),
    json_extract(json_each.value, '$.name')
FROM
    video,
    json_each(genres);

-- Insert video_genre relationships
INSERT OR IGNORE INTO video_genre (todb_id, genre_id)
SELECT
    video.todb_id,
    json_extract(json_each.value, '$.id')
FROM
    video,
    json_each(genres);
