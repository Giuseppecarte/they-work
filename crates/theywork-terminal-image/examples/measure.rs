use std::io::Write;
use theywork_terminal_image::{
    decode_transmission, encode_png, measure_encoding, CellRect, CellSize, GraphicsProtocol,
    ImageSurface, RgbaImage, TerminalGeometry,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let width = 1600_u32;
    let height = 960_u32;
    let noise = std::env::args().any(|argument| argument == "--noise");
    let arguments: Vec<_> = std::env::args().collect();
    let dump = arguments
        .iter()
        .position(|argument| argument == "--dump")
        .map(|index| {
            arguments
                .get(index + 1)
                .ok_or("--dump requires a new directory")
        })
        .transpose()?;
    if let Some(directory) = dump {
        std::fs::create_dir(directory)?;
    }
    let mut random = 1_u32;
    let mut pixels = Vec::with_capacity((width * height * 4) as usize);
    for y in 0..height {
        for x in 0..width {
            let gradient = [
                (x * 255 / width) as u8,
                (y * 255 / height) as u8,
                ((x + y) * 255 / (width + height)) as u8,
                255,
            ];
            random ^= random << 13;
            random ^= random >> 17;
            random ^= random << 5;
            pixels.extend_from_slice(&if noise {
                [random as u8, (random >> 8) as u8, (random >> 16) as u8, 255]
            } else {
                gradient
            });
        }
    }
    let image = RgbaImage::new(width, height, pixels)?;
    let rectangle = CellRect::new(0, 0, 160, 48);
    let geometry = TerminalGeometry::new(160, 48, Some(CellSize::new(10, 20)));
    if let Some(directory) = dump {
        write_new(
            std::path::Path::new(directory).join("frame.png"),
            &encode_png(&image)?,
        )?;
    }
    println!(
        "width={width} height={height} pattern={}",
        if noise { "noise" } else { "gradient" }
    );
    for protocol in [
        GraphicsProtocol::Kitty {
            direct_transmission: true,
        },
        GraphicsProtocol::Sixel,
        GraphicsProtocol::Iterm2,
    ] {
        let measurement = measure_encoding(protocol, &image, rectangle, geometry, 30)?;
        let bytes = ImageSurface::new(protocol, geometry).capture(&image, rectangle)?;
        let decoded = decode_transmission(protocol, &bytes)?;
        let max_channel_error =
            max_channel_error(&image, &decoded, protocol == GraphicsProtocol::Sixel);
        let allowed_error = if protocol == GraphicsProtocol::Sixel {
            26
        } else {
            0
        };
        if max_channel_error > allowed_error {
            return Err(format!(
                "{} reconstruction error {max_channel_error} exceeds {allowed_error}",
                protocol_name(protocol)
            )
            .into());
        }
        if let Some(directory) = dump {
            write_new(
                std::path::Path::new(directory).join(format!("{}.bin", protocol_name(protocol))),
                &bytes,
            )?;
            write_new(
                std::path::Path::new(directory)
                    .join(format!("{}-decoded.png", protocol_name(protocol))),
                &encode_png(&decoded)?,
            )?;
        }
        println!(
            "protocol={} bytes_per_frame={} encode_us_per_frame={} total_bytes={} iterations={} decoded={}x{} max_channel_error={}",
            protocol_name(protocol),
            measurement.bytes_per_frame,
            measurement.per_frame_time().as_micros(),
            measurement.total_bytes,
            measurement.iterations,
            decoded.width(),
            decoded.height(),
            max_channel_error,
        );
    }
    Ok(())
}

fn write_new(path: std::path::PathBuf, bytes: &[u8]) -> std::io::Result<()> {
    std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)?
        .write_all(bytes)
}

fn max_channel_error(source: &RgbaImage, decoded: &RgbaImage, ignore_alpha: bool) -> u8 {
    source
        .pixels()
        .chunks_exact(4)
        .zip(decoded.pixels().chunks_exact(4))
        .flat_map(|(source, decoded)| {
            let channels = if ignore_alpha { 3 } else { 4 };
            (0..channels).map(move |channel| source[channel].abs_diff(decoded[channel]))
        })
        .max()
        .unwrap_or(0)
}

fn protocol_name(protocol: GraphicsProtocol) -> &'static str {
    match protocol {
        GraphicsProtocol::Kitty {
            direct_transmission: true,
        } => "kitty-direct",
        GraphicsProtocol::Kitty {
            direct_transmission: false,
        } => "kitty-no-direct",
        GraphicsProtocol::Sixel => "sixel",
        GraphicsProtocol::Iterm2 => "iterm2",
        GraphicsProtocol::None => "none",
    }
}
