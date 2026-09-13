// Inspect the installed font's real glyph metrics and rasterise via CoreText.
// This is not a screenshot of a terminal; terminals may draw block glyphs themselves.
import AppKit
import CoreText
import Foundation

// Optional frame mode: OUTPUT_PREFIX FRAME.cells.json [FONT_POINTS].
// The input is the actual terminal buffer, without any invented scene pixels.
func renderFrame(_ source: String, _ output: String, _ points: Double) throws {
    let object = try JSONSerialization.jsonObject(with: Data(contentsOf: URL(fileURLWithPath: source))) as! [String: Any]
    let columns = object["columns"] as! Int, rows = object["rows"] as! Int
    let cells = object["cells"] as! [[[String: Any]]]
    let font = CTFontCreateWithName("Menlo-Regular" as CFString, points, nil)
    var character: UniChar = 77, glyph = CGGlyph(), advance = CGSize()
    CTFontGetGlyphsForCharacters(font, &character, &glyph, 1)
    CTFontGetAdvancesForGlyphs(font, .default, &glyph, &advance, 1)
    let cellWidth = ceil(advance.width), cellHeight = ceil(CTFontGetAscent(font) + CTFontGetDescent(font) + CTFontGetLeading(font))
    let padding = 12.0
    let width = Int(Double(columns) * cellWidth + 2 * padding)
    let height = Int(Double(rows) * cellHeight + 2 * padding)
    let context = CGContext(data: nil, width: width, height: height, bitsPerComponent: 8, bytesPerRow: width * 4,
        space: CGColorSpace(name: CGColorSpace.sRGB)!, bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue)!
    func colour(_ value: String) -> NSColor {
        let rgb = UInt32(value.trimmingCharacters(in: CharacterSet(charactersIn: "#")), radix: 16) ?? 0
        return NSColor(srgbRed: Double((rgb >> 16) & 255) / 255, green: Double((rgb >> 8) & 255) / 255, blue: Double(rgb & 255) / 255, alpha: 1)
    }
    context.setFillColor(colour(cells[0][0]["bg"] as? String ?? "#11111b").cgColor)
    context.fill(CGRect(x: 0, y: 0, width: width, height: height))
    var fonts: [String: Int] = [:]
    var fallbackSymbols = Set<String>()
    for row in 0..<rows {
        for column in 0..<columns {
            let cell = cells[row][column]
            let x = padding + Double(column) * cellWidth
            let y = padding + Double(rows - 1 - row) * cellHeight
            let rect = CGRect(x: x, y: y, width: cellWidth, height: cellHeight)
            context.saveGState()
            context.clip(to: rect)
            context.setShouldAntialias(false)
            context.setFillColor(colour(cell["bg"] as? String ?? "#11111b").cgColor)
            context.fill(rect)
            let symbol = cell["text"] as? String ?? " "
            if symbol != " " && !symbol.isEmpty {
                let name = cell["bold"] as? Bool == true ? "Menlo-Bold" : "Menlo-Regular"
                let attributed = NSAttributedString(string: symbol, attributes: [.font: NSFont(name: name, size: points)!, .foregroundColor: colour(cell["fg"] as? String ?? "#dddddd")])
                let line = CTLineCreateWithAttributedString(attributed)
                for run in CTLineGetGlyphRuns(line) as! [CTRun] {
                    let actual = (CTRunGetAttributes(run) as NSDictionary)[kCTFontAttributeName] as! CTFont
                    let actualName = CTFontCopyPostScriptName(actual) as String
                    fonts[actualName, default: 0] += 1
                    if actualName == "LastResort" { fallbackSymbols.insert(symbol) }
                }
                context.setShouldAntialias(true)
                context.textPosition = CGPoint(x: x, y: y + CTFontGetDescent(font))
                CTLineDraw(line, context)
            }
            context.restoreGState()
        }
    }
    let bitmap = NSBitmapImageRep(cgImage: context.makeImage()!)
    try bitmap.representation(using: .png, properties: [:])!.write(to: URL(fileURLWithPath: output + ".png"))
    let metadata: [String: Any] = ["source": source, "columns": columns, "rows": rows, "font": "Menlo-Regular", "font_points": points,
        "cell_width": cellWidth, "cell_height": cellHeight, "baseline_from_bottom": CTFontGetDescent(font),
        "actual_font_runs": fonts, "unsupported_symbols": fallbackSymbols.sorted(),
        "method": "Actual CoreText glyph raster with full cell backgrounds and clipping; captured buffer replay, not a Terminal.app screenshot"]
    try JSONSerialization.data(withJSONObject: metadata, options: [.prettyPrinted, .sortedKeys]).write(to: URL(fileURLWithPath: output + ".font.json"))
}

// Run with -module-cache-path docs/design-audit/tmp/iteration-2/swift-cache.
let output = CommandLine.arguments[1]
if CommandLine.arguments.count > 2 {
    try renderFrame(CommandLine.arguments[2], output, CommandLine.arguments.count > 3 ? Double(CommandLine.arguments[3])! : 18)
    exit(0)
}
let font = CTFontCreateWithName("Menlo-Regular" as CFString, 32, nil)
let specimens: [(String, String)] = [("left half", "▌"), ("right half", "▐"), ("upper half", "▀"), ("lower half", "▄"), ("quadrant UL", "▘"), ("full", "█"), ("sextant 12", "🬂"), ("sextant 3456", "🬹"), ("sextant 1356", "🬲")]
let width = 1080, height = 880
let space = CGColorSpace(name: CGColorSpace.sRGB)!
let ctx = CGContext(data: nil, width: width, height: height, bitsPerComponent: 8, bytesPerRow: width * 4, space: space, bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue)!
ctx.setFillColor(CGColor(red: 0.06, green: 0.06, blue: 0.10, alpha: 1)); ctx.fill(CGRect(x: 0, y: 0, width: width, height: height))
func text(_ text: String, _ x: Double, _ y: Double, _ size: Double = 18) {
    let attr = NSAttributedString(string: text, attributes: [.font: NSFont(name: "Menlo-Regular", size: size)!, .foregroundColor: NSColor.white])
    ctx.textPosition = CGPoint(x: x, y: y)
    CTLineDraw(CTLineCreateWithAttributedString(attr), ctx)
}
text("Installed Menlo / CoreText: actual block glyphs (not terminal UI)", 20, 846)
text("Sample           metrics      6 adjacent glyphs       repeated rows", 20, 808)
var report: [[String: Any]] = []
for (index, item) in specimens.enumerated() {
    let y = Double(height - 150 - index * 76)
    let attr = NSAttributedString(string: String(repeating: item.1, count: 6), attributes: [.font: NSFont(name: "Menlo-Regular", size: 32)!, .foregroundColor: NSColor.systemRed])
    let line = CTLineCreateWithAttributedString(attr)
    let runs = CTLineGetGlyphRuns(line) as! [CTRun]
    var metrics: [[String: Any]] = []
    for run in runs {
        let attrs = CTRunGetAttributes(run) as NSDictionary
        let actualFont = attrs[kCTFontAttributeName] as! CTFont
        var glyph = CGGlyph(); var advance = CGSize()
        CTRunGetGlyphs(run, CFRange(location: 0, length: 1), &glyph)
        CTRunGetAdvances(run, CFRange(location: 0, length: 1), &advance)
        let bounds = CTFontGetBoundingRectsForGlyphs(actualFont, .default, &glyph, nil, 1)
        metrics.append(["font": CTFontCopyPostScriptName(actualFont) as String, "glyph": glyph, "advance": advance.width, "bounds": [bounds.origin.x, bounds.origin.y, bounds.width, bounds.height], "ascent": CTFontGetAscent(actualFont), "descent": CTFontGetDescent(actualFont)])
    }
    report.append(["label": item.0, "symbol": item.1, "codepoint": String(format: "U+%04X", item.1.unicodeScalars.first!.value), "runs": metrics])
    text(item.0, 20, y + 8, 16)
    text(String(format: "U+%04X", item.1.unicodeScalars.first!.value), 190, y + 8, 15)
    ctx.textPosition = CGPoint(x: 330, y: y); CTLineDraw(line, ctx)
    for row in 0..<2 { ctx.textPosition = CGPoint(x: 710, y: y + Double(row) * 38); CTLineDraw(line, ctx) }
}
let cg = ctx.makeImage()!
let bitmap = NSBitmapImageRep(cgImage: cg)
try bitmap.representation(using: .png, properties: [:])!.write(to: URL(fileURLWithPath: output + ".png"))
let data = try JSONSerialization.data(withJSONObject: report, options: [.prettyPrinted, .sortedKeys])
try data.write(to: URL(fileURLWithPath: output + ".json"))

// Equal source geometry through real glyphs at fixed terminal-like cell pitch.
// Uniform cells retain equal foreground/background, as the renderer does.
let sceneWidth = 1300, sceneHeight = 570
let scene = CGContext(data: nil, width: sceneWidth, height: sceneHeight, bitsPerComponent: 8, bytesPerRow: sceneWidth * 4, space: space, bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue)!
let wall = NSColor(srgbRed: 0.07, green: 0.07, blue: 0.12, alpha: 1)
let ink = NSColor(srgbRed: 0.92, green: 0.18, blue: 0.14, alpha: 1)
scene.setShouldAntialias(false)
scene.setFillColor(wall.cgColor); scene.fill(CGRect(x: 0, y: 0, width: sceneWidth, height: sceneHeight))
func caption(_ message: String, _ x: Double, _ y: Double, _ size: Double = 18) {
    scene.textPosition = CGPoint(x: x, y: y)
    CTLineDraw(CTLineCreateWithAttributedString(NSAttributedString(string: message, attributes: [.font: NSFont(name: "Menlo-Regular", size: size)!, .foregroundColor: NSColor.white])), scene)
}
caption("Same rectangle: real Menlo glyphs, full cell backgrounds, 19.266 x 38 cell pitch", 20, 540)
caption("Quadrants (real glyphs)", 20, 497)
caption("Lower half blocks (real glyphs)", 450, 497)
caption("Source pixels (ideal geometry)", 880, 497)
let cellWidth = 19.265625, cellHeight = 38.0
let quadrants: [String] = [" ", "▘", "▝", "▀", "▖", "▌", "▞", "▛", "▗", "▚", "▐", "▜", "▄", "▙", "▟", "█"]
func inside(_ x: Int, _ y: Int) -> Bool { x >= 3 && x < 33 && y >= 3 && y < 17 }
func cell(_ symbol: String, _ fg: NSColor, _ bg: NSColor, _ x: Double, _ y: Double) {
    scene.saveGState()
    scene.clip(to: CGRect(x: x, y: y, width: cellWidth, height: cellHeight))
    scene.setFillColor(bg.cgColor); scene.fill(CGRect(x: x, y: y, width: cellWidth, height: cellHeight))
    scene.textPosition = CGPoint(x: x, y: y + CTFontGetDescent(font))
    CTLineDraw(CTLineCreateWithAttributedString(NSAttributedString(string: symbol, attributes: [.font: NSFont(name: "Menlo-Regular", size: 32)!, .foregroundColor: fg])), scene)
    scene.restoreGState()
}
for row in 0..<10 {
    for column in 0..<18 {
        let y = 70 + Double(9-row) * cellHeight
        var mask = 0
        for bit in 0..<4 { if inside(2 * column + bit % 2, 2 * row + bit / 2) { mask |= 1 << bit } }
        if mask == 0 || mask == 15 {
            let colour = mask == 0 ? wall : ink
            cell("█", colour, colour, 20 + Double(column) * cellWidth, y)
        } else {
            let inverse = 15 ^ mask
            let useInverse = inverse.nonzeroBitCount > mask.nonzeroBitCount || (inverse.nonzeroBitCount == mask.nonzeroBitCount && inverse < mask)
            cell(quadrants[useInverse ? inverse : mask], useInverse ? wall : ink, useInverse ? ink : wall, 20 + Double(column) * cellWidth, y)
        }
        let top = inside(2 * column + 1, 2 * row) ? ink : wall
        let bottom = inside(2 * column + 1, 2 * row + 1) ? ink : wall
        cell("▄", bottom, top, 450 + Double(column) * cellWidth, y)
        for bit in 0..<4 {
            scene.setFillColor(inside(2 * column + bit % 2, 2 * row + bit / 2) ? ink.cgColor : wall.cgColor)
            scene.fill(CGRect(x: 880 + Double(column) * cellWidth + Double(bit % 2) * cellWidth / 2, y: y + Double(1-bit / 2) * cellHeight / 2, width: cellWidth / 2, height: cellHeight / 2))
        }
    }
}
caption("Same colours and glyph inversion as the encoder; font output is clipped per cell.", 20, 32, 16)
let sceneBitmap = NSBitmapImageRep(cgImage: scene.makeImage()!)
try sceneBitmap.representation(using: .png, properties: [:])!.write(to: URL(fileURLWithPath: output + "-geometry.png"))
