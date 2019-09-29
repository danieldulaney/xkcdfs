use crate::Comic;
use cairo::{Context, Format, ImageSurface, TextExtents};
use jpeg_decoder::PixelFormat;
use std::io::{Read, Seek, SeekFrom};

const OUTER_MARGIN: f64 = 40.0;

const FONT_FAMILY: &str = "NimbusSans";

const HEADER_FONT_SIZE: f64 = 20.0;
const HEADER_TO_COMIC_SPACING: f64 = 30.0;

const COMIC_TO_ALT_SPACING: f64 = 30.0;
const ALT_WIDTH_TARGET: f64 = 500.0;
const ALT_FONT_SIZE: f64 = 16.0;
const ALT_LEADING: f64 = 5.0;
const ALT_BOX_PADDING: f64 = 10.0;
const ALT_BG_RED: f64 = 1.0;
const ALT_BG_GREEN: f64 = 0.97647058824;
const ALT_BG_BLUE: f64 = 0.74117647059;

fn jpeg_to_cairo(
    old_data: Vec<u8>,
    width: usize,
    height: usize,
    old_format: PixelFormat,
    new_format: Format,
) -> Result<(usize, Vec<u8>), String> {
    debug_assert!(width > 0);
    debug_assert!(height > 0);

    let old_pixel_size = match old_format {
        PixelFormat::RGB24 => 3,
        PixelFormat::L8 => 1,
        other => Err(format!("Unsupported pixel format: {:?}", other))?,
    };

    let new_pixel_size = match new_format {
        Format::Rgb24 => 4,
        other => Err(format!("Unsupported Cairo pixel format: {:?}", other))?,
    };

    // Old stride => byte width of each row
    // New stride => calculated from the format and width
    let old_stride = old_pixel_size * width;
    let new_stride = new_format.stride_for_width(width as u32).map_err(|()| {
        format!(
            "Failed to calculate stride for {} with width {}",
            new_format, width
        )
    })? as usize;

    debug_assert_eq!(old_stride * height, old_data.len());
    debug_assert!(new_pixel_size * width <= new_stride);

    // This is a specific conversion based on what formats are moving
    // It's hard to generalize this bit, because the fields within each pixel
    // have to change.

    match (old_format, new_format) {
        (PixelFormat::RGB24, Format::Rgb24) => {
            let mut new_data: Vec<u8> = Vec::with_capacity(new_stride * height);

            for row in 0..height {
                new_data.resize_with(row * new_stride, Default::default);

                for col in 0..width {
                    let old_index = row * old_stride + col * old_pixel_size;

                    let rgb_data = [
                        0,
                        old_data[old_index],
                        old_data[old_index + 1],
                        old_data[old_index + 2],
                    ];
                    let rgb_data = i32::from_be_bytes(rgb_data);

                    new_data.extend_from_slice(&rgb_data.to_ne_bytes());
                }
            }

            Ok((new_stride, new_data))
        }
        (o, n) => Err(format!(
            "Cannot convert between JPEG pixel format {:?} and Cairo pixel format {:?}",
            o, n
        )),
    }
}

fn create_image_surface<R: Read + Seek>(image: &mut R) -> Result<ImageSurface, String> {
    // Try decoding a PNG
    // Note: Cairo will only ever report "out of memory" on a bad PNG, so no way
    // to distinguish between a non-PNG or any other error.
    match ImageSurface::create_from_png(image) {
        Ok(s) => return Ok(s),
        _ => {}
    }

    // Go back to the beginning of the image
    image.seek(SeekFrom::Start(0)).unwrap();

    // Try decoding a JPEG
    let mut decoder = jpeg_decoder::Decoder::new(image);
    if let Ok(pixels) = decoder.decode() {
        let info = decoder
            .info()
            .ok_or_else(|| "JPEG decode succeeded but could not get metadata".to_string())?;

        // Decide which Cairo pixel format is appropriate for the decoded JPEG pixel format
        let cairo_format = match info.pixel_format {
            PixelFormat::L8 => Format::A8,
            PixelFormat::RGB24 => Format::Rgb24,
            PixelFormat::CMYK32 => {
                return Err("CMYK32 JPEGs are not currently supported".to_string())
            }
        };

        // Convert from JPEG's pixel format to Cairo's
        // There's a bunch of nuance tucked away in this function, and not all
        // format pairs are supported
        let (stride, adjusted_pixels) = jpeg_to_cairo(
            pixels,
            info.width as usize,
            info.height as usize,
            info.pixel_format,
            cairo_format,
        )?;

        // Be sure to use the stride value returned before
        return ImageSurface::create_for_data(
            adjusted_pixels,
            cairo_format,
            info.width as i32,
            info.height as i32,
            stride as i32,
        )
        .map_err(|e| e.to_string());
    }

    panic!("Could not decode the image as either a PNG or a JPEG");
}

pub fn break_text<'t>(
    ctx: Context,
    text: &'t str,
    target_width: f64,
) -> Vec<(TextExtents, &'t str)> {
    use unicode_linebreak::BreakOpportunity::Mandatory;

    let mut segment_start = 0;
    let mut last_location = 0;

    let mut lines = Vec::new();

    let mut last_extents = TextExtents {
        x_bearing: 0.0,
        y_bearing: 0.0,
        width: 0.0,
        height: 0.0,
        x_advance: 0.0,
        y_advance: 0.0,
    };

    for (location, opp_kind) in unicode_linebreak::linebreaks(text) {
        let proposed_line = &text[segment_start..location];
        let proposed_extents = ctx.text_extents(proposed_line);

        trace!("Proposed break: ({}, {:?})", location, opp_kind);

        // If this is a mandatory break, use the proposed line directly
        if opp_kind == Mandatory {
            segment_start = location;
            last_location = location;
            last_extents = proposed_extents;

            lines.push((proposed_extents, proposed_line));

            trace!(
                "Mandatory break line: {:?}, {} wide",
                proposed_line,
                proposed_extents.width
            );

            continue;
        }

        // If we're too wide and there *is* a previous segment, return that
        // segment
        if proposed_extents.width > target_width && last_location != segment_start {
            let last_line = &text[segment_start..last_location];

            segment_start = last_location;
            last_location = location;

            trace!(
                "Overflow, fallback to last line: {:?}, {} wide",
                last_line,
                last_extents.width
            );

            last_extents = proposed_extents;
            lines.push((last_extents, last_line));

            continue;
        }

        // If we're too wide, but there's nothing shorter to fall back on,
        // just roll with it
        if proposed_extents.width > target_width && last_location == segment_start {
            segment_start = location;
            last_location = location;

            trace!(
                "Too wide, but can't break: {:?}, {} wide",
                proposed_line,
                proposed_extents.width
            );

            lines.push((proposed_extents, proposed_line));

            last_extents = proposed_extents;
            continue;
        }

        // If we got here, we're not too wide, but we might not be wide enough
        // Update this location in case the next line wants it, but don't
        // start a new segment
        last_location = location;
        last_extents = proposed_extents;
    }

    lines
}

pub fn aligned_start_points(sizes: &mut [f64]) -> &mut [f64] {
    let widest = sizes.iter().fold(-std::f64::INFINITY, |a, &b| a.max(b));

    for size in sizes.iter_mut() {
        *size = (widest - *size) / 2.0;
    }

    sizes
}

pub fn text_block_extents<'e, I: IntoIterator<Item = &'e TextExtents>>(
    iter: I,
    line_spacing: f64,
) -> TextExtents {
    let mut iter = iter.into_iter();
    let first_extents = iter.next().unwrap().clone();

    iter.fold(first_extents, |mut acc, new| {
        acc.width = acc.width.max(new.width);
        acc.height += line_spacing + new.height;

        acc.x_advance = acc.x_advance.max(new.x_advance);
        acc.y_advance += line_spacing + new.y_advance;

        acc
    })
}

pub fn render<R: Read + Seek>(comic: &Comic, image: &mut R) -> Result<Vec<u8>, String> {
    // Load this first because we need its coordinates
    let comic_surface = create_image_surface(image)?;
    let comic_ctx = Context::new(&comic_surface);

    let comic_width = comic_surface.get_width() as f64;
    let comic_height = comic_surface.get_height() as f64;

    // Set title font settings
    comic_ctx.select_font_face(
        FONT_FAMILY,
        cairo::FontSlant::Normal,
        cairo::FontWeight::Bold,
    );
    comic_ctx.set_font_size(HEADER_FONT_SIZE);

    // Get the title size
    let header_size = comic_ctx.text_extents(&comic.safe_title);

    // Set alt text font settings
    comic_ctx.select_font_face(
        FONT_FAMILY,
        cairo::FontSlant::Normal,
        cairo::FontWeight::Normal,
    );
    comic_ctx.set_font_size(ALT_FONT_SIZE);

    // Set alt text size
    let alt_lines = break_text(comic_ctx, &comic.alt, ALT_WIDTH_TARGET);
    let alt_extents = text_block_extents(alt_lines.iter().map(|(ref e, _)| e), ALT_LEADING);

    trace!(
        "Alt text is {} by {}, {:?}",
        alt_extents.width,
        alt_extents.height,
        alt_lines
    );

    // Set alt box size -- Need to floor and ceil explicitly to avoid bluriness
    let alt_box_width = (ALT_BOX_PADDING + alt_extents.width + ALT_BOX_PADDING).floor();
    let alt_box_height = (ALT_BOX_PADDING + alt_extents.height + ALT_BOX_PADDING).ceil();

    trace!("Alt box is {} by {}", alt_box_width, alt_box_height);

    // Overall width is the largest of the elements, plus the margins
    let overall_width = OUTER_MARGIN
        + header_size
            .width
            .max(comic_surface.get_width() as f64)
            .max(alt_box_width)
        + OUTER_MARGIN;

    // Overall height is the sum of the element heights, plus the margins, plus the spacing
    let overall_height = OUTER_MARGIN
        + header_size.height
        + HEADER_TO_COMIC_SPACING
        + comic_height as f64
        + COMIC_TO_ALT_SPACING
        + alt_box_height
        + OUTER_MARGIN;

    trace!("Overall image: ({}, {})", overall_width, overall_height);

    // X start points
    let mut start_points = [header_size.width, comic_width, alt_box_width];
    let start_points = aligned_start_points(&mut start_points);
    let header_start_x = OUTER_MARGIN + start_points[0].floor();
    let comic_start_x = OUTER_MARGIN + start_points[1].floor();
    let alt_box_start_x = OUTER_MARGIN + start_points[2].floor() + 0.5;

    // Y start points
    let header_start_y = OUTER_MARGIN + header_size.height;
    let comic_start_y = header_start_y + HEADER_TO_COMIC_SPACING;
    let alt_box_start_y = (comic_start_y + comic_height + COMIC_TO_ALT_SPACING).floor() + 0.5;

    // Alt start points
    let alt_start_x = alt_box_start_x + ALT_BOX_PADDING - alt_extents.x_bearing;
    let alt_start_y = alt_box_start_y + ALT_BOX_PADDING - alt_extents.y_bearing;

    trace!("Comic start point: ({}, {})", comic_start_x, comic_start_y);

    // Create a surface with the calculated dimensions
    let surface = ImageSurface::create(Format::ARgb32, overall_width as i32, overall_height as i32)
        .expect("Can't create surface");
    let cr = Context::new(&surface);

    cr.select_font_face(
        FONT_FAMILY,
        cairo::FontSlant::Normal,
        cairo::FontWeight::Bold,
    );
    cr.set_font_size(HEADER_FONT_SIZE);

    cr.set_source_rgb(0.0, 0.0, 0.0);
    cr.move_to(header_start_x, header_start_y);
    cr.show_text(&comic.safe_title);

    cr.set_source_surface(&comic_surface, comic_start_x, comic_start_y);
    cr.paint();

    // Create the alt-text rectangle
    trace!(
        "Drawing alt-text rectangle from ({}, {}), dims {} by {}",
        alt_box_start_x,
        alt_box_start_y,
        alt_box_width,
        alt_box_height,
    );

    cr.set_source_rgb(ALT_BG_RED, ALT_BG_GREEN, ALT_BG_BLUE);
    cr.rectangle(
        alt_box_start_x,
        alt_box_start_y,
        alt_box_width,
        alt_box_height,
    );
    cr.fill();

    cr.set_source_rgb(0.0, 0.0, 0.0);
    cr.set_line_width(1.0);
    cr.rectangle(
        alt_box_start_x,
        alt_box_start_y,
        alt_box_width,
        alt_box_height,
    );
    cr.stroke();

    // Set alt text font settings
    cr.select_font_face(
        FONT_FAMILY,
        cairo::FontSlant::Normal,
        cairo::FontWeight::Normal,
    );
    cr.set_font_size(ALT_FONT_SIZE);

    cr.move_to(alt_start_x, alt_start_y);

    for (extents, line) in alt_lines {
        cr.show_text(line);

        let (_, curr_y) = cr.get_current_point();

        cr.move_to(alt_start_x, curr_y + ALT_LEADING + extents.height)
    }

    // Create the final PNG
    let mut buffer = Vec::new();

    surface
        .write_to_png(&mut buffer)
        .expect("Can't write surface to PNG");

    Ok(buffer)
}
