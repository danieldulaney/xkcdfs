use crate::Comic;
use cairo::{Context, Format, ImageSurface};
use jpeg_decoder::PixelFormat;
use std::io::{Read, Seek, SeekFrom};

const OUTER_MARGIN: f64 = 40.0;

const HEADER_FONT_SIZE: f64 = 20.0;
const HEADER_FONT_FACE: &str = "NimbusSans";
const HEADER_TO_COMIC_SPACING: f64 = 30.0;

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

    eprintln!(
        "{} by {} moving from a stride of {} to a stride of {}",
        width, height, old_stride, new_stride
    );

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

pub fn render<R: Read + Seek>(comic: &Comic, image: &mut R) -> Result<Vec<u8>, String> {
    // Load this first because we need its coordinates
    let comic_surface = create_image_surface(image)?;
    let comic_ctx = Context::new(&comic_surface);

    let comic_width = comic_surface.get_width() as f64;
    let comic_height = comic_surface.get_height() as f64;

    // Make these settings consistent
    comic_ctx.select_font_face(
        HEADER_FONT_FACE,
        cairo::FontSlant::Normal,
        cairo::FontWeight::Bold,
    );
    comic_ctx.set_font_size(HEADER_FONT_SIZE);

    // Get the title size
    let header_size = comic_ctx.text_extents(&comic.safe_title);

    eprintln!("Header content: {}", comic.safe_title);
    eprintln!(
        "Header size: ({}, {})",
        header_size.width, header_size.height
    );

    // Overall width is the larger of the two elements, plus the margins
    let overall_width =
        OUTER_MARGIN + header_size.width.max(comic_surface.get_width() as f64) + OUTER_MARGIN;

    // Overall height is the sum of the element heights, plus the margins, plus the spacing
    let overall_height = OUTER_MARGIN
        + header_size.height
        + HEADER_TO_COMIC_SPACING
        + comic_height as f64
        + OUTER_MARGIN;

    eprintln!("Overall image: ({}, {})", overall_width, overall_height);

    // Header starting point
    let header_start_x = OUTER_MARGIN + 0f64.max((comic_width - header_size.width) / 2.0);
    let header_start_y = OUTER_MARGIN + header_size.height;

    eprintln!(
        "Header start point: ({}, {})",
        header_start_x, header_start_y
    );

    // Comic starting point
    let comic_start_x = OUTER_MARGIN + 0f64.max((header_size.width - comic_width) / 2.0);
    let comic_start_y = OUTER_MARGIN + header_size.height + HEADER_TO_COMIC_SPACING;

    eprintln!("Comic start point: ({}, {})", comic_start_x, comic_start_y);

    // Create a surface with the calculated dimensions
    let surface = ImageSurface::create(Format::ARgb32, overall_width as i32, overall_height as i32)
        .expect("Can't create surface");
    let cr = Context::new(&surface);

    cr.select_font_face(
        HEADER_FONT_FACE,
        cairo::FontSlant::Normal,
        cairo::FontWeight::Bold,
    );
    cr.set_font_size(HEADER_FONT_SIZE);

    cr.set_source_rgb(0.0, 0.0, 0.0);
    cr.move_to(header_start_x, header_start_y);
    cr.show_text(&comic.safe_title);

    cr.set_source_surface(&comic_surface, comic_start_x, comic_start_y);
    cr.paint();

    let mut buffer = Vec::new();

    surface
        .write_to_png(&mut buffer)
        .expect("Can't write surface to PNG");

    Ok(buffer)
}
