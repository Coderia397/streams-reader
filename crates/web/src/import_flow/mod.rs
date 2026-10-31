//! Getting a document into the library: decide what was given (text, link, file, share), parse it in the browser
//! with `nolabs_import` in a Web Worker (`worker`), then create the document and upload its body
//! (`POST /api/documents`, `PUT …/body`).

pub mod meta;
pub mod run;
pub mod source;
pub mod worker;

pub use run::{Stage, import};
pub use source::ImportSource;
