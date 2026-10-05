//! Thumbnails (deferred beyond release 0.1.0).
//!
//! Planned: `request(&ThumbKey) -> Thumb` decoding a mod icon or preview with size and dimension
//! caps, resizing to 64, 128 or 256 pixels and caching the result below `thumbnails/` in the cache
//! root through `rimstudio-io`. The scanner already records `icon_path` and `preview_path` per mod.
//! Nothing is implemented here on purpose: the 0.1.0 slice has no mod list screen.
