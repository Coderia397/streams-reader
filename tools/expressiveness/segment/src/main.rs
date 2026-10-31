//! Each argument is a passage file: paragraphs separated by a blank line. Output: JSON {file name: [segment]} with
//! the segment text, its boundary and its sentence char ranges, from `nolabs_core::text` with the Kokoro profile.
use nolabs_core::proto::api::{Block, BlockKind};
use nolabs_core::text::{SegmentProfile, normalise_blocks, segment, sentences};

fn main() {
    let mut out = serde_json::Map::new();
    for path in std::env::args().skip(1) {
        let raw = std::fs::read_to_string(&path).expect("read passage");
        let blocks: Vec<Block> =
            raw.split("\n\n").map(|t| Block { kind: BlockKind::Paragraph, text: t.to_owned() }).collect();
        let blocks = normalise_blocks(blocks);
        let segs: Vec<serde_json::Value> = segment(&blocks, SegmentProfile::KOKORO)
            .iter()
            .map(|s| {
                serde_json::json!({
                    "text": s.text,
                    "boundary": format!("{:?}", s.boundary).to_lowercase(),
                    "sentences": sentences(&s.text),
                })
            })
            .collect();
        let name = std::path::Path::new(&path).file_stem().map(|n| n.to_string_lossy().into_owned()).unwrap_or(path);
        out.insert(name, serde_json::Value::Array(segs));
    }
    println!("{}", serde_json::to_string_pretty(&out).expect("json"));
}
