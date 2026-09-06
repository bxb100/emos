# emos-cache

`FileCache<V>` stores a map with string keys in a local `.mpbr` file. Values use
MessagePack and Brotli compression, and the map is loaded lazily on first access.

The public API provides `new`, `get`, `set`, `delete`, and `save`. Call `save().await`
to persist pending changes and report write errors. The loaded map stays available
until the cache is dropped, including after an empty save. Drop also attempts to
save pending changes synchronously. An explicit save detects an external write
when the file size differs from the previously observed size; equal-size writes
and writes after that check can go undetected.

Entries do not expire. The on-disk `expiry_timestamp` field is retained so existing
cache files remain readable; new entries write zero.

The implementation and its tests live in the private `file` module. Import the
public type with `use emos_cache::FileCache`.

Originally adapted from <https://gitlab.com/lib.rs/main>.
