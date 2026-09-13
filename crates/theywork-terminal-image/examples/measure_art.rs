//! Measure the actual unlabelled RGB PPM from the living_gallery example.
//! Times cover encoding only, not PTY transfer or terminal presentation.
use std::{error::Error, fs};
use theywork_terminal_image::{
    decode_transmission, encode_kitty, measure_encoding, CellRect, GraphicsProtocol, RgbaImage,
    TerminalGeometry,
};
fn main() -> Result<(), Box<dyn Error>> {
    let path = std::env::args().nth(1).ok_or("provide a P6 PPM export")?;
    let bytes = fs::read(path)?;
    let mut cursor = 0;
    let mut token = || {
        while bytes.get(cursor).is_some_and(u8::is_ascii_whitespace) {
            cursor += 1;
        }
        let start = cursor;
        while bytes
            .get(cursor)
            .is_some_and(|byte| !byte.is_ascii_whitespace())
        {
            cursor += 1;
        }
        String::from_utf8_lossy(&bytes[start..cursor]).into_owned()
    };
    if token() != "P6" {
        return Err("expected P6".into());
    }
    let width = token().parse::<u32>()?;
    let height = token().parse::<u32>()?;
    if token() != "255" {
        return Err("expected RGB8".into());
    }
    cursor += 1;
    let rgba = bytes[cursor..]
        .chunks_exact(3)
        .flat_map(|p| [p[0], p[1], p[2], 255])
        .collect();
    let image = RgbaImage::new(width, height, rgba)?;
    let rectangle = CellRect::new(0, 0, (width / 8) as u16, (height / 16) as u16);
    let geometry = TerminalGeometry::new(rectangle.width, rectangle.height, None);
    let kitty = GraphicsProtocol::Kitty {
        direct_transmission: true,
    };
    let encoded = encode_kitty(&image, rectangle, 1);
    assert_eq!(decode_transmission(kitty, &encoded)?, image);
    println!("{{\"width\":{width},\"height\":{height},\"raw_base64_bytes\":{},\"kitty_lossless_roundtrip\":true,\"encoding_only\":[",image.pixels().len().div_ceil(3)*4);
    for (index, (name, protocol)) in [
        ("kitty", kitty),
        ("iterm2", GraphicsProtocol::Iterm2),
        ("sixel", GraphicsProtocol::Sixel),
    ]
    .into_iter()
    .enumerate()
    {
        let measured = measure_encoding(protocol, &image, rectangle, geometry, 10)?;
        println!(
            "{}{{\"protocol\":\"{}\",\"bytes_per_frame\":{},\"microseconds_per_frame\":{}}}",
            if index > 0 { "," } else { "" },
            name,
            measured.bytes_per_frame,
            measured.per_frame_time().as_micros()
        );
    }
    println!("]}}");
    Ok(())
}
