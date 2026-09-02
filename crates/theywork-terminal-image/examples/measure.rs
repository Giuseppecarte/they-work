use theywork_terminal_image::{
    measure_encoding, CellRect, CellSize, GraphicsProtocol, RgbaImage, TerminalGeometry,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let width = 320_u32;
    let height = 180_u32;
    let mut pixels = Vec::with_capacity((width * height * 4) as usize);
    for y in 0..height {
        for x in 0..width {
            pixels.extend_from_slice(&[
                (x * 255 / width) as u8,
                (y * 255 / height) as u8,
                ((x + y) * 255 / (width + height)) as u8,
                255,
            ]);
        }
    }
    let image = RgbaImage::new(width, height, pixels)?;
    let rectangle = CellRect::new(0, 0, 80, 24);
    let geometry = TerminalGeometry::new(160, 48, Some(CellSize::new(8, 16)));
    for protocol in [
        GraphicsProtocol::Kitty {
            direct_transmission: true,
        },
        GraphicsProtocol::Sixel,
    ] {
        let measurement = measure_encoding(protocol, &image, rectangle, geometry, 3)?;
        println!(
            "protocol={} bytes_per_frame={} encode_us_per_frame={} total_bytes={} iterations={}",
            protocol_name(protocol),
            measurement.bytes_per_frame,
            measurement.per_frame_time().as_micros(),
            measurement.total_bytes,
            measurement.iterations,
        );
    }
    Ok(())
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
        GraphicsProtocol::None => "none",
    }
}
