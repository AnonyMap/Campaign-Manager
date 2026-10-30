//! Image loading that never hands the graphics card a texture bigger than it can take.
//!
//! egui's renderer stops the whole program when an image is wider or taller than the GPU's
//! maximum texture size (often 8192 or 16384 pixels). Big battle or world maps easily go
//! past that, so this loader (tried before egui_extras' own) shrinks such images to fit.
//! Map pins are stored as fractions of the image, so shrinking doesn't move them.

use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use std::task::Poll;

use egui::ColorImage;
use egui::load::{BytesPoll, ImageLoadResult, ImageLoader, ImagePoll, LoadError, SizeHint};
use egui::mutex::Mutex;

/// The largest side we'll show, even when the graphics card allows more: an 8192 × 8192
/// image already takes 256 MB of video memory, and maps look fine at that size.
pub const MAX_SIDE: usize = 8192;

/// How much memory decoding one image may use (a 16,000-pixel square photo needs ~1 GB).
const DECODE_LIMIT: u64 = 1536 * 1024 * 1024;

type Entry = Poll<Result<Arc<ColorImage>, String>>;

#[derive(Default)]
pub struct FitImageLoader {
    cache: Arc<Mutex<HashMap<String, Entry>>>,
}

impl FitImageLoader {
    pub const ID: &'static str = egui::generate_loader_id!(FitImageLoader);
}

/// Decode an image, shrinking it (keeping its shape) so neither side is over `max_side`.
pub fn decode(bytes: &[u8], max_side: usize) -> Result<ColorImage, String> {
    let mut reader = image::ImageReader::new(std::io::Cursor::new(bytes))
        .with_guessed_format()
        .map_err(|e| e.to_string())?;
    let mut limits = image::Limits::default();
    limits.max_alloc = Some(DECODE_LIMIT);
    reader.limits(limits);
    let img = reader.decode().map_err(|e| match e {
        image::ImageError::Limits(_) => {
            "This image is too large to open. Save a smaller copy (under about 16,000 pixels on its longest side) and use that instead.".to_string()
        }
        e => e.to_string(),
    })?;
    let max_side = max_side.max(1) as u32;
    let img = if img.width() > max_side || img.height() > max_side {
        img.resize(max_side, max_side, image::imageops::FilterType::Triangle)
    } else {
        img
    };
    let rgba = img.to_rgba8();
    Ok(ColorImage::from_rgba_unmultiplied(
        [rgba.width() as usize, rgba.height() as usize],
        rgba.as_raw(),
    ))
}

fn supported_extension(uri: &str) -> bool {
    match Path::new(uri).extension().and_then(|e| e.to_str()) {
        Some(ext) => image::ImageFormat::from_extension(ext).is_some_and(|f| f.reading_enabled()),
        None => true,
    }
}

impl ImageLoader for FitImageLoader {
    fn id(&self) -> &str {
        Self::ID
    }

    fn load(&self, ctx: &egui::Context, uri: &str, _: SizeHint) -> ImageLoadResult {
        // egui adds a frame index ("#0") to gif / webp URIs
        let uri = egui::decode_animated_image_uri(uri).map_or(uri, |(u, _)| u);
        if uri.starts_with("file://") && !supported_extension(uri) {
            return Err(LoadError::NotSupported);
        }
        if let Some(entry) = self.cache.lock().get(uri).cloned() {
            return match entry {
                Poll::Ready(Ok(image)) => Ok(ImagePoll::Ready { image }),
                Poll::Ready(Err(err)) => Err(LoadError::Loading(err)),
                Poll::Pending => Ok(ImagePoll::Pending { size: None }),
            };
        }
        let bytes = match ctx.try_load_bytes(uri) {
            Ok(BytesPoll::Ready { bytes, mime, .. }) => {
                if let Some(m) = mime.as_deref() {
                    let deferred = m.contains("octet-stream") || m.contains("download");
                    if !deferred && !image::ImageFormat::from_mime_type(m).is_some_and(|f| f.reading_enabled()) {
                        return Err(LoadError::FormatNotSupported {
                            detected_format: Some(m.to_string()),
                        });
                    }
                }
                bytes
            }
            Ok(BytesPoll::Pending { size }) => return Ok(ImagePoll::Pending { size }),
            Err(e) => return Err(e),
        };
        let max_side = ctx.input(|i| i.max_texture_side).clamp(1, MAX_SIDE);
        let key = uri.to_owned();
        self.cache.lock().insert(key.clone(), Poll::Pending);
        let cache = self.cache.clone();
        let ctx = ctx.clone();
        let spawned = std::thread::Builder::new().name("image decode".into()).spawn(move || {
            let result = decode(&bytes, max_side).map(Arc::new);
            // only store it if nobody forgot the image meanwhile; never hold the cache lock
            // while touching the context (that can deadlock)
            let stored = match cache.lock().get_mut(&key) {
                Some(slot) => {
                    *slot = Poll::Ready(result);
                    true
                }
                None => false,
            };
            if stored {
                ctx.request_repaint();
            }
        });
        if let Err(e) = spawned {
            self.cache.lock().insert(uri.to_owned(), Poll::Ready(Err(e.to_string())));
        }
        Ok(ImagePoll::Pending { size: None })
    }

    fn forget(&self, uri: &str) {
        self.cache.lock().remove(uri);
    }

    fn forget_all(&self) {
        self.cache.lock().clear();
    }

    fn byte_size(&self) -> usize {
        self.cache
            .lock()
            .values()
            .map(|e| match e {
                Poll::Ready(Ok(img)) => img.pixels.len() * 4,
                Poll::Ready(Err(err)) => err.len(),
                Poll::Pending => 0,
            })
            .sum()
    }

    fn has_pending(&self) -> bool {
        self.cache.lock().values().any(|e| e.is_pending())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn png(w: u32, h: u32) -> Vec<u8> {
        let img = image::RgbImage::from_pixel(w, h, image::Rgb([200, 100, 50]));
        let mut out = Vec::new();
        image::DynamicImage::ImageRgb8(img)
            .write_to(&mut std::io::Cursor::new(&mut out), image::ImageFormat::Png)
            .unwrap();
        out
    }

    #[test]
    fn big_images_are_shrunk_to_fit() {
        // too wide: shrunk, keeping its shape
        let img = decode(&png(3000, 1000), 1024).unwrap();
        assert_eq!(img.size, [1024, 341]);
        // small enough: untouched
        let img = decode(&png(300, 200), 1024).unwrap();
        assert_eq!(img.size, [300, 200]);
        assert!(decode(b"not an image", 1024).is_err());
    }
}
