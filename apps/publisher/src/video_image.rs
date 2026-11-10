//! CPU presentation conversion shared with the standalone release benchmark.
use eframe::egui::{Color32, ColorImage};
use livekit::webrtc::video_frame::{VideoBuffer, VideoFormatType};

pub fn to_color_image(buffer: &dyn VideoBuffer) -> ColorImage {
    let width = buffer.width();
    let height = buffer.height();
    let mut image = ColorImage::filled([width as usize, height as usize], Color32::TRANSPARENT);
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
    image
}

#[cfg(test)]
mod tests {
    use super::*;
    use livekit::webrtc::video_frame::I420Buffer;

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
