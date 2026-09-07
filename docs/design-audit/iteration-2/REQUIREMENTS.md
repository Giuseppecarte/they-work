# Second iteration: real-terminal resize and a living office

The user's five native Terminal screenshots contradict the previous visual
completion claim. Passing golden hashes and a fixed five-worker scene did not
prove a usable office with one or three idle conversations at other sizes.
The earlier work remains a baseline, not evidence that this iteration is done.

## Requirements and evidence needed

1. **Correct character rasterization.** Straight edges and pixel letters must
   preserve all representable two-colour cell patterns. Verify Unicode masks
   independently of the production lookup and compare against the supplied real
   screenshots. Do not confuse an ideal SVG export with a native terminal.
2. **Coherent offices while resizing.** One, three and twenty workers must remain
   readable at 80×24, 120×32, 192×58, 240×70, and a tall viewport. Isometric,
   top and side views must each have proportionate furniture, people and a
   project sign. No giant text, empty grid rows, furniture detached from people,
   overlapping names, or offscreen interaction targets.
3. **Visible, honest motion.** Working figures visibly work; idle figures may
   blink, look around or pause without changing their reported state. Motion
   must survive character rendering, remain stable through resizing, and stop
   when disabled. Test multiple timestamps and actual PTY output, not only a
   frame counter or raw sprite pixels before downsampling.
4. **An inhabited room.** Desks, chairs, lighting/windows and shared furnishings
   must form a coherent space with circulation. A box per worker and an oversized
   title are insufficient even when every label technically fits.
5. **All routes remain usable.** Tower → floor → desk, phone, settings, help,
   source connection and per-project customization must survive resize. Cover
   real-shaped synthetic titles and multiple independent projects; do not commit
   private conversation content to obtain representative captures.
6. **Review the final artifact.** Rebuild the native executable and exercise it
   through a PTY with live resizing; inspect stills and motion sequences. Run
   workspace tests, strict Clippy and formatting on the finished code. Preserve
   reproducible evidence and document every environment that could not be
   inspected directly. The branch remains `audit/design-and-usability` with no
   push, merge or release publication unless separately requested.

## Evidence limits

The provided screenshots are actual native-terminal evidence. New ANSI/buffer
reconstructions must be labelled as such. Computer Use explicitly denied access
to Terminal.app in the previous iteration; no alternate capture path is used to
bypass that restriction. A claimed perfect UI cannot be inferred from passing
snapshots alone. Native graphics protocols and independent user judgment must
not be marked passing without evidence.
