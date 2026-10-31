//! Writes every auth email in every language as HTML and text files, for looking at them in a browser:
//! `cargo run -p nolabs-mail --example preview -- <out dir>`.

use std::fs;
use std::path::PathBuf;

use nolabs_mail::render::{Brand, Email, Kind, render};

const LANGS: [&str; 8] = ["en", "ar", "es", "fr", "de", "pt", "hi", "zh"];

fn main() -> std::io::Result<()> {
    let out = PathBuf::from(std::env::args().nth(1).unwrap_or_else(|| "target/mail-preview".to_owned()));
    fs::create_dir_all(&out)?;
    let brand = Brand { name: nolabs_core::brand::NAME, site_url: nolabs_core::brand::SITE_URL };
    for lang in LANGS {
        for kind in Kind::ALL {
            let email = Email {
                kind,
                lang: lang.to_owned(),
                to: "reader@example.com".to_owned(),
                code: Some("482913".to_owned()),
                link: (kind != Kind::Reauthentication).then(|| format!("{}/app/auth/confirm?token_hash=example", brand.site_url)),
                new_email: (kind == Kind::EmailChange).then(|| "new@example.com".to_owned()),
                minutes: 10,
            };
            let r = render(&brand, &email);
            let name = format!("{lang}-{kind:?}").to_lowercase();
            fs::write(out.join(format!("{name}.html")), &r.html)?;
            fs::write(out.join(format!("{name}.txt")), format!("Subject: {}\n\n{}", r.subject, r.text))?;
        }
    }
    println!("wrote {} emails to {}", LANGS.len() * Kind::ALL.len(), out.display());
    Ok(())
}
