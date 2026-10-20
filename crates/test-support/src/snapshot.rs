//! Pixel checks on headless renders of the iced UIs.

use iced_test::simulator::Snapshot;
use std::fs::File;
use std::io::BufReader;

/// A pixel counts as coloured when its channels differ by more than this:
/// greys (the dark theme, white text, anti-aliasing) stay well below, the
/// colours of an emoji font far above.
const CHROMA_THRESHOLD: u8 = 64;

/// How many pixels of `snapshot` are coloured rather than grey.
///
/// Text in a monochrome font has none on the dark theme; a colour emoji
/// font has many.
pub fn colored_pixels(snapshot: &Snapshot) -> usize {
    let bytes = rgba(snapshot);
    let (pixels, _) = bytes.as_chunks::<4>();
    pixels
        .iter()
        .filter(|[r, g, b, _]| {
            let min = *r.min(g).min(b);
            let max = *r.max(g).max(b);
            max - min > CHROMA_THRESHOLD
        })
        .count()
}

/// The RGBA bytes of `snapshot`, which keeps them private: it writes a
/// missing comparison image (`<stem>-<renderer>.png`), so write one in a
/// fresh directory and read it back.
fn rgba(snapshot: &Snapshot) -> Vec<u8> {
    let dir = tempfile::tempdir().expect("create temp dir");
    assert!(
        snapshot
            .matches_image(dir.path().join("snapshot.png"))
            .expect("write the snapshot"),
        "a fresh directory has no image to differ from"
    );
    let image = std::fs::read_dir(dir.path())
        .expect("read the temp dir")
        .next()
        .expect("the snapshot was written")
        .expect("read the temp dir")
        .path();

    let decoder = png::Decoder::new(BufReader::new(
        File::open(image).expect("open the snapshot"),
    ));
    let mut reader = decoder.read_info().expect("decode the snapshot");
    let mut bytes = vec![
        0;
        reader
            .output_buffer_size()
            .expect("snapshot fits in memory")
    ];
    let info = reader.next_frame(&mut bytes).expect("decode the snapshot");
    assert_eq!(
        info.color_type,
        png::ColorType::Rgba,
        "iced_test writes RGBA"
    );
    bytes.truncate(info.buffer_size());
    bytes
}
