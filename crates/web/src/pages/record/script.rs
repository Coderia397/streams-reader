//! The guided script. Eight short lines with a range of feeling (calm narration, joy, sorrow, firmness, curiosity,
//! dialogue), so the reference carries more than one register. The script is also the exact transcript sent with the
//! recording, so no speech recognition is needed.

pub struct Line {
    /// Message key of the feeling to read the line with.
    pub mood: &'static str,
    pub text: &'static str,
}

pub const SCRIPT: [Line; 8] = [
    Line { mood: "mood-warm", text: "Hi there. I'm recording a few lines so this app can read to me in my own voice." },
    Line { mood: "mood-calm", text: "The morning was quiet, and the coffee was still too hot to drink." },
    Line { mood: "mood-delighted", text: "Wait, you found it? That's wonderful news. I can't believe it!" },
    Line { mood: "mood-sorry", text: "I'm sorry. I never meant for any of this to happen, truly." },
    Line { mood: "mood-firm", text: "Listen carefully, because I'm only going to say this once." },
    Line { mood: "mood-curious", text: "Do you ever wonder what's on the other side of that hill?" },
    Line { mood: "mood-amused", text: "She laughed, shook her head, and said, well, that's one way to do it." },
    Line { mood: "mood-gentle", text: "And so, as the last light faded, the town finally fell asleep." },
];

/// The transcript of the whole recording, in reading order.
pub fn transcript() -> String {
    SCRIPT.iter().map(|l| l.text).collect::<Vec<_>>().join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn script_is_short_and_complete() {
        assert_eq!(SCRIPT.len(), 8);
        assert!(SCRIPT.iter().all(|l| l.text.len() < 90 && l.mood.starts_with("mood-")));
        for l in &SCRIPT {
            assert_ne!(crate::i18n::tr(l.mood, &[]), l.mood);
        }
        let t = transcript();
        assert!(t.starts_with("Hi there.") && t.ends_with("fell asleep."));
        // No brackets: they would read as Ming paralinguistic tags.
        assert!(!t.contains('[') && !t.contains(']'));
    }
}
