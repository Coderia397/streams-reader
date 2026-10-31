//! Build inputs that are data, not code: the hero demo's texts, voices and clip timings (`assets/demo`, made by
//! `tools/demo_clips.py`) and the classics row (`assets/covers/covers.json`, made by `tools/covers.py`).

use std::path::Path;

use serde::Deserialize;

use crate::Error;
use crate::html::esc;

/// One word: start and end in ms, then the first and last+1 character (Unicode scalar) of the word in the text.
pub type Word = [u32; 4];

#[derive(Debug, Clone, Deserialize)]
pub struct DemoText {
    pub id: String,
    pub text: String,
    pub source: String,
    /// Voice ids (`DemoVoice::id`) with a rendered clip of this text.
    pub clips: Vec<String>,
    /// Word ranges of the first clip (every clip of a text has the same ranges; checked in tests).
    #[serde(skip)]
    pub words: Vec<Word>,
    /// Loudness envelope of the first clip (levels 1-10), drawn as the waveform before anything plays.
    #[serde(skip)]
    pub levels: Vec<u8>,
    /// Audio duration of the first clip in ms: the player's time label before anything plays (the script counts
    /// down from the same value).
    #[serde(skip)]
    pub duration: u32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DemoVoice {
    pub id: String,
    pub voice: String,
    pub name: String,
    pub accent: String,
    pub tier: String,
    #[serde(default = "yes")]
    pub available: bool,
}

fn yes() -> bool {
    true
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct Demo {
    pub texts: Vec<DemoText>,
    pub voices: Vec<DemoVoice>,
}

/// A clip's timing file.
#[derive(Debug, Clone, Deserialize)]
pub struct Clip {
    /// Duration in ms.
    pub d: u32,
    /// Loudness levels 1-10.
    #[serde(default)]
    pub a: Vec<u8>,
    pub w: Vec<Word>,
}

impl Demo {
    /// Read `demo.json` and the first clip of every text. A missing folder gives an empty demo (the hero then shows
    /// only the film).
    pub fn load(dir: &Path) -> Result<Self, Error> {
        let path = dir.join("demo.json");
        if !path.is_file() {
            return Ok(Self::default());
        }
        let mut demo: Demo = read_json(&path)?;
        for t in &mut demo.texts {
            let Some(first) = t.clips.first() else { continue };
            let clip: Clip = read_json(&dir.join(format!("{}-{first}.json", t.id)))?;
            t.words = clip.w;
            t.levels = clip.a;
            t.duration = clip.d;
        }
        Ok(demo)
    }

    pub fn voice(&self, id: &str) -> Option<&DemoVoice> {
        self.voices.iter().find(|v| v.id == id)
    }
}

impl DemoText {
    /// The text as HTML: each timed word in `<span class="w">`, line breaks as `<br>`, straight apostrophes curled.
    pub fn html(&self) -> String {
        let chars: Vec<char> = self.text.chars().collect();
        let mut out = String::with_capacity(self.text.len() * 3);
        let mut at = 0usize;
        let plain = |from: usize, to: usize| -> String {
            let s: String = chars[from.min(chars.len())..to.min(chars.len())].iter().collect();
            esc(&curl(&s)).replace('\n', "<br>")
        };
        for w in &self.words {
            let (a, b) = (w[2] as usize, w[3] as usize);
            if a < at || b <= a || b > chars.len() {
                continue;
            }
            out.push_str(&plain(at, a));
            out.push_str(&format!("<span class=\"w\">{}</span>", plain(a, b)));
            at = b;
        }
        out.push_str(&plain(at, chars.len()));
        out
    }
}

/// Typographic apostrophes for display (same character count, so timings still line up).
pub fn curl(s: &str) -> String {
    s.replace('\'', "\u{2019}")
}

#[derive(Debug, Clone, Deserialize)]
pub struct Book {
    pub slug: String,
    pub title: String,
    pub author: String,
    pub url: String,
    #[serde(default)]
    pub line: String,
    #[serde(default)]
    pub preview_voice: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct Library {
    pub books: Vec<Book>,
}

impl Library {
    pub fn load(path: &Path) -> Result<Self, Error> {
        if !path.is_file() {
            return Ok(Self::default());
        }
        read_json(path)
    }
}

fn read_json<T: serde::de::DeserializeOwned>(path: &Path) -> Result<T, Error> {
    let raw = std::fs::read_to_string(path).map_err(|e| Error::io(path, e))?;
    serde_json::from_str(&raw).map_err(|e| Error::Data(format!("{}: {e}", path.display())))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn words_become_spans_and_breaks() {
        let t = DemoText {
            id: "x".into(),
            text: "It's late,\nso go.".into(),
            source: String::new(),
            clips: vec![],
            words: vec![[0, 1, 0, 4], [1, 2, 5, 9], [2, 3, 11, 13], [3, 4, 14, 16]],
            levels: vec![],
            duration: 0,
        };
        assert_eq!(
            t.html(),
            "<span class=\"w\">It\u{2019}s</span> <span class=\"w\">late</span>,<br><span class=\"w\">so</span> \
             <span class=\"w\">go</span>."
        );
    }
}
