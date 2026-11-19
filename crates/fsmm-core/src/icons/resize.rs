//! Area-averaging downscale (box filter with fractional pixel coverage).
//!
//! Each output pixel is the average of the source area it covers, where partly covered source
//! pixels count in proportion to the overlap. Colour is weighted by alpha so transparent pixels
//! do not darken or tint their neighbours. All arithmetic is integer, so results are exact and
//! deterministic.

use super::Rgba;

pub(super) fn downscale(image: &Rgba, max_edge: u32) -> Rgba {
    let max_edge = max_edge.max(1);
    let longest = image.width.max(image.height);
    if longest <= max_edge {
        return image.clone();
    }
    let (w, h) = (image.width as usize, image.height as usize);
    let dw = scaled(image.width, max_edge, longest) as usize;
    let dh = scaled(image.height, max_edge, longest) as usize;

    let columns = axis_weights(w, dw);
    let rows = axis_weights(h, dh);

    // Horizontal pass. Per output column: sum of weight * alpha * colour (x3) and weight * alpha.
    let mut across = vec![[0u64; 4]; dw * h];
    for y in 0..h {
        let row = &image.pixels[y * w * 4..(y + 1) * w * 4];
        for (ox, taps) in columns.iter().enumerate() {
            let acc = &mut across[y * dw + ox];
            for &(x, weight) in taps {
                let px = &row[x * 4..x * 4 + 4];
                let weight = u64::from(weight);
                let alpha = u64::from(px[3]);
                for c in 0..3 {
                    acc[c] += weight * alpha * u64::from(px[c]);
                }
                acc[3] += weight * alpha;
            }
        }
    }

    // Vertical pass and normalisation. The total weight of one output pixel is w * h.
    let area = (w * h) as u64;
    let mut pixels = Vec::with_capacity(dw * dh * 4);
    for taps in &rows {
        for ox in 0..dw {
            let mut acc = [0u64; 4];
            for &(y, weight) in taps {
                let src = &across[y * dw + ox];
                for c in 0..4 {
                    acc[c] += u64::from(weight) * src[c];
                }
            }
            let alpha_sum = acc[3];
            for &sum in &acc[..3] {
                let colour = (sum + alpha_sum / 2).checked_div(alpha_sum).unwrap_or(0);
                pixels.push(colour as u8);
            }
            pixels.push(((alpha_sum + area / 2) / area) as u8);
        }
    }
    Rgba {
        width: dw as u32,
        height: dh as u32,
        pixels,
    }
}

/// `n * max_edge / longest`, rounded, at least 1.
fn scaled(n: u32, max_edge: u32, longest: u32) -> u32 {
    let longest = u64::from(longest);
    ((u64::from(n) * u64::from(max_edge) + longest / 2) / longest).max(1) as u32
}

/// For each of `dst` output pixels, the source pixels it covers and their weights.
///
/// Source pixel `i` spans `[i*dst, (i+1)*dst)` and output pixel `o` spans `[o*src, (o+1)*src)`
/// on a common axis of length `src*dst`, so the overlaps are integers and each output pixel's
/// weights sum to `src`.
fn axis_weights(src: usize, dst: usize) -> Vec<Vec<(usize, u32)>> {
    (0..dst)
        .map(|o| {
            let (start, end) = (o * src, (o + 1) * src);
            (start / dst..=(end - 1) / dst)
                .map(|i| {
                    let overlap = end.min((i + 1) * dst) - start.max(i * dst);
                    (i, overlap as u32)
                })
                .collect()
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn image(width: u32, height: u32, pixels: Vec<u8>) -> Rgba {
        assert_eq!(pixels.len(), (width * height * 4) as usize);
        Rgba {
            width,
            height,
            pixels,
        }
    }

    /// Opaque grey image from one value per pixel.
    fn grey(width: u32, height: u32, values: &[u8]) -> Rgba {
        let pixels = values.iter().flat_map(|&v| [v, v, v, 255]).collect();
        image(width, height, pixels)
    }

    fn red_channel(image: &Rgba) -> Vec<u8> {
        image.pixels.chunks(4).map(|p| p[0]).collect()
    }

    #[test]
    fn four_by_four_to_two_by_two_averages_each_2x2_block() {
        #[rustfmt::skip]
        let src = grey(4, 4, &[
            0, 100, 10, 10,
            100, 0, 10, 10,
            200, 200, 255, 253,
            200, 200, 255, 253,
        ]);
        let out = downscale(&src, 2);
        assert_eq!((out.width, out.height), (2, 2));
        assert_eq!(red_channel(&out), [50, 10, 200, 254]);
        assert!(out
            .pixels
            .chunks(4)
            .all(|p| p[3] == 255 && p[0] == p[1] && p[1] == p[2]));
    }

    #[test]
    fn non_integer_ratio_weights_partial_pixels() {
        // Output pixel 0 covers source columns [0, 2.5), pixel 1 covers [2.5, 5).
        // Values 0, 10, 100, 10, 0 across, the same on every row.
        let row = [0u8, 10, 100, 10, 0];
        let values: Vec<u8> = (0..5).flat_map(|_| row).collect();
        let out = downscale(&grey(5, 5, &values), 2);
        assert_eq!((out.width, out.height), (2, 2));
        // (0 + 10 + 0.5 * 100) / 2.5 = 24 and (0.5 * 100 + 10 + 0) / 2.5 = 24.
        assert_eq!(red_channel(&out), [24, 24, 24, 24]);
    }

    #[test]
    fn non_integer_ratio_is_asymmetric_when_the_data_is() {
        // Columns [0, 2.5): (10 + 20 + 0.5 * 30) / 2.5 = 18; [2.5, 5): (0.5 * 30 + 40 + 50) / 2.5 = 42.
        let row = [10u8, 20, 30, 40, 50];
        let values: Vec<u8> = (0..5).flat_map(|_| row).collect();
        let out = downscale(&grey(5, 5, &values), 2);
        assert_eq!(red_channel(&out), [18, 42, 18, 42]);
    }

    #[test]
    fn aspect_ratio_is_kept() {
        let src = grey(512, 256, &vec![77; 512 * 256]);
        let out = downscale(&src, 256);
        assert_eq!((out.width, out.height), (256, 128));
        assert!(out.pixels.chunks(4).all(|p| p == [77, 77, 77, 255]));

        let tall = downscale(&grey(20, 600, &vec![1; 20 * 600]), 256);
        assert_eq!((tall.width, tall.height), (9, 256));

        // A very thin image keeps at least one pixel on the short side.
        let thin = downscale(&grey(1000, 1, &vec![5; 1000]), 100);
        assert_eq!((thin.width, thin.height), (100, 1));
        let thinner = downscale(&grey(8192, 1, &vec![5; 8192]), 4);
        assert_eq!((thinner.width, thinner.height), (4, 1));
    }

    #[test]
    fn odd_sizes_reach_the_requested_edge() {
        let out = downscale(&grey(552, 552, &vec![9; 552 * 552]), 256);
        assert_eq!((out.width, out.height), (256, 256));
        let out = downscale(&grey(500, 500, &vec![9; 500 * 500]), 256);
        assert_eq!((out.width, out.height), (256, 256));
        assert!(out.pixels.chunks(4).all(|p| p == [9, 9, 9, 255]));
    }

    #[test]
    fn images_within_the_limit_are_returned_unchanged() {
        let src = grey(3, 2, &[1, 2, 3, 4, 5, 6]);
        assert_eq!(downscale(&src, 3), src);
        assert_eq!(downscale(&src, 256), src);
    }

    #[test]
    fn transparent_pixels_do_not_darken_the_colour() {
        // One opaque red pixel and one fully transparent blue pixel.
        let src = image(2, 1, vec![255, 0, 0, 255, 0, 0, 255, 0]);
        let out = downscale(&src, 1);
        assert_eq!((out.width, out.height), (1, 1));
        assert_eq!(out.pixels, [255, 0, 0, 128]);
    }

    #[test]
    fn colour_is_weighted_by_alpha() {
        // Opaque white and 25% opaque black average to 255 * 255 / (255 + 64) = 204 grey.
        let src = image(2, 1, vec![255, 255, 255, 255, 0, 0, 0, 64]);
        let out = downscale(&src, 1);
        assert_eq!(out.pixels, [204, 204, 204, 160]);
    }

    #[test]
    fn fully_transparent_areas_stay_transparent() {
        let src = image(2, 2, [9, 9, 9, 0].repeat(4));
        let out = downscale(&src, 1);
        assert_eq!(out.pixels, [0, 0, 0, 0]);
    }
}
