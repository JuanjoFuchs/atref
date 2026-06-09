//! Per-result enrichment (specs 010 + 011): size · lines · ~tokens and image
//! thumbnails, computed lazily and cloud-safely, plus the bounded cache that
//! keys results by `(path, mtime)`. Pure logic lives here behind a testable
//! seam; the Tauri command in `lib.rs` is a thin async shell that runs this
//! off the UI thread.

use std::collections::{HashMap, VecDeque};
use std::fs::Metadata;
use std::path::{Path, PathBuf};

use base64::Engine;
use serde::Serialize;

/// Source-size caps (spec 010 FR5, spec 011 TC3): past these, contents are
/// not read/decoded. Parameterized so tests exercise the gates without
/// writing megabytes.
pub struct Caps {
    /// Max bytes to read for line/token metrics.
    pub text: u64,
    /// Max raster source bytes to decode for a thumbnail.
    pub raster: u64,
    /// Max SVG source bytes to pass through as a thumbnail.
    pub svg: u64,
}

pub const DEFAULT_CAPS: Caps = Caps {
    text: 10 * 1024 * 1024,
    raster: 30 * 1024 * 1024,
    svg: 512 * 1024,
};

/// Thumbnail bound in px (spec 011 TC2) — 2× the 28 px row display, for hidpi.
pub const THUMB_PX: u32 = 56;

/// How many enrichments the cache keeps (spec 010 NFR2). Metrics rows are
/// tiny, but spec 011 stores thumbnail data URIs in the same entries.
pub const CACHE_CAP: usize = 256;

/// What the frontend receives per enriched file (spec 010 TC4, spec 011 TC2).
/// `None` lines / tokens mean "size only" — cloud-only, binary, unreadable,
/// or over-cap; `thumb` is a bounded data URI for image results.
#[derive(Clone, Serialize)]
pub struct Enrichment {
    pub size: u64,
    pub lines: Option<u64>,
    pub tokens: Option<u64>,
    pub thumb: Option<String>,
}

/// Image results recognized by extension (spec 011 TC4) — content sniffing
/// would require the very read the cloud guard exists to avoid.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ImageKind {
    Raster,
    Svg,
}

pub fn image_kind(path: &Path) -> Option<ImageKind> {
    let ext = path.extension()?.to_str()?.to_ascii_lowercase();
    match ext.as_str() {
        "png" | "jpg" | "jpeg" | "gif" | "webp" => Some(ImageKind::Raster),
        "svg" => Some(ImageKind::Svg),
        _ => None,
    }
}

/// Decode + downscale a raster to a bounded square, re-encoded as a PNG data
/// URI (spec 011 TC2/NFR2). Pure over bytes; `None` on undecodable input.
/// Animated GIFs decode as their first frame.
pub fn make_thumb(bytes: &[u8]) -> Option<String> {
    let img = image::load_from_memory(bytes).ok()?;
    let thumb = img.thumbnail(THUMB_PX, THUMB_PX);
    let mut png = Vec::new();
    thumb
        .write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
        .ok()?;
    let b64 = base64::engine::general_purpose::STANDARD.encode(&png);
    Some(format!("data:image/png;base64,{b64}"))
}

/// SVG passthrough — text the WebView renders natively (spec 011 TC2).
pub fn svg_data_uri(bytes: &[u8]) -> String {
    let b64 = base64::engine::general_purpose::STANDARD.encode(bytes);
    format!("data:image/svg+xml;base64,{b64}")
}

/// Cloud-only / offline placeholder (OneDrive, Dropbox, …): reading its
/// *contents* triggers hydration — the 2026-06-09 `.gitignore` incident — so
/// enrichment must stop at metadata (spec 010 TC1). Shared with spec 011.
pub fn is_cloud_placeholder(md: &Metadata) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        const FILE_ATTRIBUTE_OFFLINE: u32 = 0x1000;
        const FILE_ATTRIBUTE_RECALL_ON_DATA_ACCESS: u32 = 0x40_0000;
        md.file_attributes() & (FILE_ATTRIBUTE_OFFLINE | FILE_ATTRIBUTE_RECALL_ON_DATA_ACCESS) != 0
    }
    #[cfg(not(windows))]
    {
        let _ = md;
        false
    }
}

/// Line count over raw bytes: `\n`s, plus one for an unterminated final line
/// (so a 3-line file without a trailing newline counts 3). Empty → 0.
pub fn count_lines(bytes: &[u8]) -> u64 {
    if bytes.is_empty() {
        return 0;
    }
    let newlines = bytes.iter().filter(|&&b| b == b'\n').count() as u64;
    if bytes.ends_with(b"\n") {
        newlines
    } else {
        newlines + 1
    }
}

/// A NUL byte in the first 8 KiB → treat as binary, no line/token metrics
/// (spec 010 FR5).
pub fn looks_binary(bytes: &[u8]) -> bool {
    bytes.iter().take(8192).any(|&b| b == 0)
}

/// ~token estimate via tiktoken `o200k_base` (spec 010 TC2) — the closest
/// public approximation of modern LLM tokenizers; the UI labels it `~`. The
/// encoder builds lazily on first use (the enrich command already runs off the
/// UI thread). Falls back to a chars/4 heuristic if the encoder can't build.
pub fn estimate_tokens(text: &str) -> u64 {
    use std::sync::OnceLock;
    static BPE: OnceLock<Option<tiktoken_rs::CoreBPE>> = OnceLock::new();
    match BPE.get_or_init(|| tiktoken_rs::o200k_base().ok()) {
        Some(bpe) => bpe.encode_ordinary(text).len() as u64,
        None => (text.chars().count() as u64).div_ceil(4),
    }
}

/// Enrich `path` given its already-fetched metadata: size always; lines/tokens
/// when the file is local, text, and within cap (spec 010 FR4/FR5); a bounded
/// thumbnail for image types (spec 011 FR1, with SVG getting text metrics too —
/// it's text — from the same single read). Never reads the contents of a
/// cloud placeholder (the shared guard, spec 010 TC1 / spec 011 TC1).
pub fn enrich_file(path: &Path, md: &Metadata, caps: &Caps) -> Enrichment {
    let size = md.len();
    let size_only = Enrichment {
        size,
        lines: None,
        tokens: None,
        thumb: None,
    };
    if is_cloud_placeholder(md) {
        return size_only;
    }
    match image_kind(path) {
        Some(ImageKind::Raster) => {
            if size > caps.raster {
                return size_only;
            }
            let Ok(bytes) = std::fs::read(path) else {
                return size_only;
            };
            Enrichment {
                thumb: make_thumb(&bytes),
                ..size_only
            }
        }
        Some(ImageKind::Svg) => {
            if size > caps.text {
                return size_only;
            }
            let Ok(bytes) = std::fs::read(path) else {
                return size_only;
            };
            let text = String::from_utf8_lossy(&bytes);
            Enrichment {
                size,
                lines: Some(count_lines(&bytes)),
                tokens: Some(estimate_tokens(&text)),
                // Over the SVG cap: keep the metrics, skip the thumb (TC3).
                thumb: (size <= caps.svg).then(|| svg_data_uri(&bytes)),
            }
        }
        None => {
            if size > caps.text {
                return size_only;
            }
            let Ok(bytes) = std::fs::read(path) else {
                return size_only;
            };
            if looks_binary(&bytes) {
                return size_only;
            }
            let text = String::from_utf8_lossy(&bytes);
            Enrichment {
                size,
                lines: Some(count_lines(&bytes)),
                tokens: Some(estimate_tokens(&text)),
                thumb: None,
            }
        }
    }
}

/// Bounded enrichment cache keyed `(path, mtime)` (spec 010 TC3/NFR2): an
/// edit changes mtime and misses; once past `cap`, the oldest-inserted entry
/// is evicted (FIFO — approximate LRU is plenty at picker scale).
pub struct EnrichCache {
    cap: usize,
    map: HashMap<PathBuf, (u64, Enrichment)>,
    order: VecDeque<PathBuf>,
}

impl EnrichCache {
    pub fn new(cap: usize) -> Self {
        EnrichCache {
            cap: cap.max(1),
            map: HashMap::new(),
            order: VecDeque::new(),
        }
    }

    /// A hit only when the stored mtime matches — edits invalidate (TC3).
    pub fn get(&self, path: &Path, mtime: u64) -> Option<Enrichment> {
        self.map
            .get(path)
            .filter(|(m, _)| *m == mtime)
            .map(|(_, e)| e.clone())
    }

    pub fn put(&mut self, path: PathBuf, mtime: u64, e: Enrichment) {
        if self.map.insert(path.clone(), (mtime, e)).is_none() {
            self.order.push_back(path);
            if self.order.len() > self.cap {
                if let Some(oldest) = self.order.pop_front() {
                    self.map.remove(&oldest);
                }
            }
        }
    }

    pub fn len(&self) -> usize {
        self.map.len()
    }

    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn tmp_file(name: &str, contents: &[u8]) -> PathBuf {
        let path = std::env::temp_dir().join(format!("atref_enrich_{}_{name}", std::process::id()));
        fs::write(&path, contents).unwrap();
        path
    }

    fn enr(s: u64) -> Enrichment {
        Enrichment {
            size: s,
            lines: None,
            tokens: None,
            thumb: None,
        }
    }

    /// PNG bytes of a `w`×`h` solid-color RGBA image, for generation tests.
    fn png_bytes(w: u32, h: u32, rgba: [u8; 4]) -> Vec<u8> {
        let img = image::RgbaImage::from_pixel(w, h, image::Rgba(rgba));
        let mut out = Vec::new();
        image::DynamicImage::ImageRgba8(img)
            .write_to(&mut std::io::Cursor::new(&mut out), image::ImageFormat::Png)
            .unwrap();
        out
    }

    /// Decode a `data:image/png;base64,…` URI back into an image.
    fn decode_data_uri(uri: &str) -> image::DynamicImage {
        let b64 = uri.strip_prefix("data:image/png;base64,").unwrap();
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(b64)
            .unwrap();
        image::load_from_memory(&bytes).unwrap()
    }

    #[test]
    fn counts_lines_including_unterminated_and_crlf() {
        // Spec 010 AC2: 3 lines without a trailing newline count 3; CRLF
        // counts by \n; a trailing newline doesn't add a phantom line.
        assert_eq!(count_lines(b""), 0);
        assert_eq!(count_lines(b"one"), 1);
        assert_eq!(count_lines(b"a\nb\nc"), 3);
        assert_eq!(count_lines(b"a\nb\nc\n"), 3);
        assert_eq!(count_lines(b"a\r\nb\r\n"), 2);
    }

    #[test]
    fn token_estimate_grows_with_content() {
        // Spec 010 AC2: > 0 for real text and monotonic-ish in length.
        assert_eq!(estimate_tokens(""), 0);
        let short = estimate_tokens("hello world");
        let long = estimate_tokens(&"the quick brown fox jumps over the lazy dog. ".repeat(50));
        assert!(short > 0);
        assert!(long > short);
    }

    #[test]
    fn binary_sniff_finds_nul() {
        // Spec 010 AC5.
        assert!(!looks_binary(b"plain text, no nul"));
        assert!(looks_binary(b"PNG\x00binary"));
    }

    #[test]
    fn enriches_text_file_with_lines_and_tokens() {
        // Spec 010 AC2 end-to-end over a real temp file; spec 011 FR5 — no
        // thumbnail for a non-image.
        let path = tmp_file("text.md", b"# title\n\nbody line");
        let md = fs::metadata(&path).unwrap();
        let e = enrich_file(&path, &md, &DEFAULT_CAPS);
        assert_eq!(e.size, md.len());
        assert_eq!(e.lines, Some(3));
        assert!(e.tokens.unwrap() > 0);
        assert_eq!(e.thumb, None);
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn binary_file_is_size_only() {
        // Spec 010 AC5.
        let path = tmp_file("bin.dat", b"\x89PNG\x00\x01\x02");
        let md = fs::metadata(&path).unwrap();
        let e = enrich_file(&path, &md, &DEFAULT_CAPS);
        assert_eq!(e.size, 7);
        assert_eq!(e.lines, None);
        assert_eq!(e.tokens, None);
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn over_cap_file_is_size_only_without_reading() {
        // Spec 010 AC6: past the cap, contents are not read (tested with a
        // tiny cap so the test doesn't write megabytes).
        let path = tmp_file("big.txt", b"five!");
        let md = fs::metadata(&path).unwrap();
        let caps = Caps {
            text: 4,
            ..DEFAULT_CAPS
        };
        let e = enrich_file(&path, &md, &caps);
        assert_eq!(e.size, 5);
        assert_eq!(e.lines, None);
        assert_eq!(e.tokens, None);
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn thumb_downscales_within_bound_keeping_aspect() {
        // Spec 011 AC1: a 200×100 source becomes a ≤56 px PNG data URI at 2:1.
        let src = png_bytes(200, 100, [200, 30, 30, 255]);
        let uri = make_thumb(&src).unwrap();
        let thumb = decode_data_uri(&uri);
        assert_eq!((thumb.width(), thumb.height()), (56, 28));
        assert!(make_thumb(b"not an image").is_none());
    }

    #[test]
    fn gif_thumbnails_from_first_frame() {
        // Spec 011 AC2: a two-frame GIF (red, then blue) thumbs as red.
        let red = image::RgbaImage::from_pixel(20, 20, image::Rgba([255, 0, 0, 255]));
        let blue = image::RgbaImage::from_pixel(20, 20, image::Rgba([0, 0, 255, 255]));
        let mut buf = Vec::new();
        {
            let mut enc = image::codecs::gif::GifEncoder::new(&mut buf);
            enc.encode_frames(vec![image::Frame::new(red), image::Frame::new(blue)])
                .unwrap();
        }
        let uri = make_thumb(&buf).unwrap();
        let thumb = decode_data_uri(&uri).to_rgba8();
        let px = thumb.get_pixel(thumb.width() / 2, thumb.height() / 2);
        assert!(
            px[0] > 180 && px[2] < 80,
            "center pixel is red (first frame), got {px:?}"
        );
    }

    #[test]
    fn image_kind_recognizes_extensions() {
        // Spec 011 AC2/TC4.
        assert_eq!(image_kind(Path::new("a.png")), Some(ImageKind::Raster));
        assert_eq!(image_kind(Path::new("a.JPG")), Some(ImageKind::Raster));
        assert_eq!(image_kind(Path::new("a.webp")), Some(ImageKind::Raster));
        assert_eq!(image_kind(Path::new("a.svg")), Some(ImageKind::Svg));
        assert_eq!(image_kind(Path::new("a.md")), None);
        assert_eq!(image_kind(Path::new("no-extension")), None);
    }

    #[test]
    fn image_file_enriches_with_thumb_and_no_text_metrics() {
        // Spec 011 AC3: one payload carries the thumbnail; rasters are binary,
        // so lines/tokens stay None.
        let path = tmp_file("pic.png", &png_bytes(64, 64, [10, 200, 10, 255]));
        let md = fs::metadata(&path).unwrap();
        let e = enrich_file(&path, &md, &DEFAULT_CAPS);
        assert!(e.thumb.unwrap().starts_with("data:image/png;base64,"));
        assert_eq!(e.lines, None);
        assert_eq!(e.tokens, None);
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn oversized_raster_has_no_thumb() {
        // Spec 011 AC3/TC3 — tiny cap, real file.
        let path = tmp_file("big.png", &png_bytes(64, 64, [10, 200, 10, 255]));
        let md = fs::metadata(&path).unwrap();
        let caps = Caps {
            raster: 4,
            ..DEFAULT_CAPS
        };
        let e = enrich_file(&path, &md, &caps);
        assert_eq!(e.thumb, None);
        assert_eq!(e.size, md.len(), "size still shown");
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn svg_gets_text_metrics_and_capped_thumb() {
        // Spec 011 AC3: under-cap SVG → metrics + svg data URI; over the SVG
        // cap the metrics survive and only the thumb drops (TC3).
        let svg = b"<svg xmlns=\"http://www.w3.org/2000/svg\">\n<rect/>\n</svg>";
        let path = tmp_file("img.svg", svg);
        let md = fs::metadata(&path).unwrap();
        let e = enrich_file(&path, &md, &DEFAULT_CAPS);
        assert_eq!(e.lines, Some(3));
        assert!(e.tokens.unwrap() > 0);
        assert!(e.thumb.unwrap().starts_with("data:image/svg+xml;base64,"));

        let caps = Caps {
            svg: 4,
            ..DEFAULT_CAPS
        };
        let e = enrich_file(&path, &md, &caps);
        assert_eq!(e.lines, Some(3), "metrics unaffected by the svg cap");
        assert_eq!(e.thumb, None, "thumb dropped over the svg cap");
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn cache_hits_once_and_invalidates_on_mtime() {
        // Spec 010 AC3: same (path, mtime) hits; a bumped mtime misses.
        let mut cache = EnrichCache::new(8);
        let p = PathBuf::from(r"D:\v\a.md");
        assert!(cache.get(&p, 100).is_none());
        cache.put(p.clone(), 100, enr(10));
        assert_eq!(cache.get(&p, 100).unwrap().size, 10);
        assert!(cache.get(&p, 101).is_none(), "edit invalidates");
        cache.put(p.clone(), 101, enr(11));
        assert_eq!(cache.get(&p, 101).unwrap().size, 11);
        assert_eq!(cache.len(), 1, "same path re-keyed, not duplicated");
    }

    #[test]
    fn cache_is_bounded() {
        // Spec 010 AC3/NFR2: the bound holds; the oldest entry is evicted.
        let mut cache = EnrichCache::new(2);
        cache.put(PathBuf::from("a"), 1, enr(1));
        cache.put(PathBuf::from("b"), 1, enr(2));
        cache.put(PathBuf::from("c"), 1, enr(3));
        assert_eq!(cache.len(), 2);
        assert!(cache.get(Path::new("a"), 1).is_none(), "oldest evicted");
        assert!(cache.get(Path::new("c"), 1).is_some());
    }
}
