#![forbid(unsafe_code)]
#![doc = include_str!("../README.md")]

mod converter;
pub mod dom;
mod domutil;
mod embed;
mod encoding;
mod extraction;

pub use extraction::{
    apply, apply_for_file, apply_for_html, apply_for_reader, apply_to_node, Error, Options, Result,
    TimingEntry, TimingInfo,
};
pub mod filter;
pub mod label;
pub mod markup;
pub mod pagination;
pub mod stringutil;
mod table;
mod urlutil;
pub mod webdoc;

pub use dom::Document;

pub fn apply_shared_document(
    document: &impl AsRef<Document>,
    options: &Options,
) -> std::result::Result<Result, Error> {
    apply(document.as_ref(), options)
}

#[cfg(test)]
mod upstream_tests;
