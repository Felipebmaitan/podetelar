//! CPU presentation conversion shared with the standalone release benchmark.
use eframe::egui::{Color32, ColorImage};
use livekit::webrtc::video_frame::{VideoBuffer, VideoFormatType};
use std::sync::Arc;

pub fn to_color_image(buffer: &dyn VideoBuffer) -> ColorImage {
    let width = buffer.width();
    let height = buffer.height();
    let mut image = ColorImage::filled([width as usize, height as usize], Color32::TRANSPARENT);
    write_color_image(buffer, &mut image);
    image
}

fn write_color_image(buffer: &dyn VideoBuffer, image: &mut ColorImage) {
    let width = buffer.width();
    let height = buffer.height();
    assert_eq!(image.size, [width as usize, height as usize]);
    // The pinned SDK converts our decoded I420/NV12 pictures to opaque RGBA
    // bytes in ABGR mode. With alpha=255 these already satisfy Color32's
    // premultiplied layout: write once into the exclusively owned final pixels.
    buffer.to_argb(
        VideoFormatType::ABGR,
        image.as_raw_mut(),
        width * 4,
        width as i32,
        height as i32,
    );
}

/// Exclusive mutation comes from Arc::get_mut, including while egui retains an
/// image for deferred upload. No buffer is recycled while another owner reads it.
pub struct ImagePool {
    images: Vec<Arc<ColorImage>>,
    limit: usize,
    #[cfg(feature = "performance-experiments")]
    fallback: std::sync::Weak<ColorImage>,
}
impl Default for ImagePool {
    fn default() -> Self {
        #[cfg(feature = "performance-experiments")]
        let limit = std::env::var("FREEDOMSTREAM_IMAGE_POOL_LIMIT")
            .ok()
            .and_then(|value| value.parse::<usize>().ok())
            .filter(|limit| (1..=2).contains(limit))
            .unwrap_or(2);
        #[cfg(not(feature = "performance-experiments"))]
        let limit = 2;
        Self {
            images: Vec::new(),
            limit,
            #[cfg(feature = "performance-experiments")]
            fallback: std::sync::Weak::new(),
        }
    }
}
impl ImagePool {
    #[cfg(feature = "performance-experiments")]
    pub fn allocate_fallback(&mut self, buffer: &dyn VideoBuffer) -> Option<Arc<ColorImage>> {
        if self.limit != 1 || self.fallback.strong_count() != 0 {
            return None;
        }
        let image = Arc::new(to_color_image(buffer));
        self.fallback = Arc::downgrade(&image);
        Some(image)
    }
    pub fn clear(&mut self) {
        self.images.clear();
        #[cfg(feature = "performance-experiments")]
        {
            self.fallback = std::sync::Weak::new();
        }
    }
    pub fn convert(&mut self, buffer: &dyn VideoBuffer) -> Option<Arc<ColorImage>> {
        let size = [buffer.width() as usize, buffer.height() as usize];
        if self.images.first().is_some_and(|image| image.size != size) {
            self.clear();
        }
        let free = self
            .images
            .iter()
            .position(|image| Arc::strong_count(image) == 1 && Arc::weak_count(image) == 0);
        let index = if let Some(index) = free {
            index
        } else {
            if self.images.len() >= self.limit {
                return None;
            }
            self.images
                .push(Arc::new(ColorImage::filled(size, Color32::TRANSPARENT)));
            self.images.len() - 1
        };
        write_color_image(buffer, Arc::get_mut(&mut self.images[index])?);
        Some(self.images[index].clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use livekit::webrtc::video_frame::I420Buffer;
    #[test]
    fn one_image_pool_never_overwrites_a_held_image() {
        let buffer = I420Buffer::new_black(17, 9);
        let mut pool = ImagePool {
            limit: 1,
            ..Default::default()
        };
        let held = pool.convert(buffer.as_ref()).unwrap();
        let address = held.pixels.as_ptr();
        let pixels = held.pixels.clone();
        assert!(pool.convert(buffer.as_ref()).is_none());
        assert_eq!(held.pixels, pixels);
        drop(held);
        assert_eq!(
            pool.convert(buffer.as_ref()).unwrap().pixels.as_ptr(),
            address
        );
    }

    #[test]
    #[cfg(feature = "performance-experiments")]
    fn temporary_fallback_keeps_the_cache_bounded_and_held_pixels_unchanged() {
        let black = I420Buffer::new_black(17, 9);
        let mut red = I420Buffer::new(17, 9);
        let (y, u, v) = red.data_mut();
        y.fill(82);
        u.fill(90);
        v.fill(240);
        let mut pool = ImagePool {
            limit: 1,
            ..Default::default()
        };
        let held = pool.convert(black.as_ref()).unwrap();
        let before = held.pixels.clone();
        let image = pool
            .convert(red.as_ref())
            .or_else(|| pool.allocate_fallback(red.as_ref()))
            .unwrap();
        assert_eq!(held.pixels, before);
        assert_eq!(image.pixels, to_color_image(red.as_ref()).pixels);
        assert_eq!(pool.images.len(), 1);
        assert!(pool.allocate_fallback(red.as_ref()).is_none());
        drop(image);
        assert!(pool.allocate_fallback(red.as_ref()).is_some());
    }
    #[test]
    fn bounded_pool_preserves_live_images_and_reuses_only_released_storage() {
        let mut black = I420Buffer::new(17, 9);
        let (y, u, v) = black.data_mut();
        y.fill(16);
        u.fill(128);
        v.fill(128);
        let mut red = I420Buffer::new(17, 9);
        let (y, u, v) = red.data_mut();
        y.fill(82);
        u.fill(90);
        v.fill(240);
        let mut pool = ImagePool {
            limit: 2,
            ..Default::default()
        };
        let first = pool.convert(black.as_ref()).unwrap();
        let expected = first.pixels.clone();
        let address = first.pixels.as_ptr();
        let second = pool.convert(red.as_ref()).unwrap();
        assert_eq!(first.pixels, expected);
        assert!(pool.convert(red.as_ref()).is_none());
        assert_eq!(pool.images.len(), 2);
        drop(first);
        let reused = pool.convert(red.as_ref()).unwrap();
        assert_eq!(reused.pixels.as_ptr(), address);
        assert_eq!(reused.pixels, second.pixels);
        let changed = I420Buffer::new_black(2, 2);
        let resized = pool.convert(changed.as_ref()).unwrap();
        assert_eq!(resized.size, [2, 2]);
        assert_eq!(second.size, [17, 9]);
        pool.clear();
        assert!(pool.images.is_empty());
        assert_eq!(reused.pixels, second.pixels);
    }

    #[test]
    fn padded_odd_frames_match_the_existing_color_and_alpha_contract() {
        for (width, height) in [(2, 2), (17, 9), (321, 181)] {
            let mut buffer = I420Buffer::with_strides(
                width,
                height,
                width + 7,
                width.div_ceil(2) + 5,
                width.div_ceil(2) + 3,
            );
            let (y, u, v) = buffer.data_mut();
            for (plane, offset) in [(y, 16), (u, 70), (v, 140)] {
                for (index, byte) in plane.iter_mut().enumerate() {
                    *byte = ((index * 37 + offset) % 256) as u8;
                }
            }
            let mut reference = vec![0; width as usize * height as usize * 4];
            buffer.as_ref().to_argb(
                VideoFormatType::ABGR,
                &mut reference,
                width * 4,
                width as i32,
                height as i32,
            );
            let expected =
                ColorImage::from_rgba_unmultiplied([width as usize, height as usize], &reference);
            let image = to_color_image(buffer.as_ref());
            assert_eq!(image.size, expected.size);
            assert_eq!(image.pixels, expected.pixels);
            assert!(image.pixels.iter().all(|pixel| pixel.a() == 255));
        }
    }

    #[test]
    fn red_fixture_preserves_channel_order() {
        let mut buffer = I420Buffer::new(2, 2);
        let (y, u, v) = buffer.data_mut();
        y.fill(82);
        u.fill(90);
        v.fill(240);
        for pixel in to_color_image(buffer.as_ref()).pixels {
            assert!(pixel.r() > 240 && pixel.g() < 5 && pixel.b() < 5);
            assert_eq!(pixel.a(), 255);
        }
    }
}
