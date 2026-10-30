// Copyright 2026 RelyBytes S.r.l. <https://relybytes.com>
// SPDX-License-Identifier: Apache-2.0

//! The text of a document set on A4, for the formats with no layout reader yet.

use std::fmt::Write;

use crate::fonts::body_fonts;
use crate::markup::{font_list, string_literal};

/// One paragraph per line of text; a blank line becomes space between paragraphs.
pub(crate) fn text_source(text: &str) -> String {
    let mut source = String::from("#set page(paper: \"a4\", margin: 2cm)\n");
    let _ = writeln!(
        source,
        "#set text(font: {}, size: 11pt, lang: \"it\", hyphenate: false)",
        font_list(&body_fonts(None))
    );
    source.push_str("#set par(spacing: 6pt)\n");
    for line in text.lines().map(str::trim_end) {
        if line.trim().is_empty() {
            source.push_str("#v(6pt)\n");
        } else {
            let _ = writeln!(source, "#par({})\n", string_literal(line));
        }
    }
    source
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sets_each_line_as_a_paragraph() {
        let source = text_source("Primo #a\n\n  \nSecondo ]\n");
        assert!(source.starts_with("#set page(paper: \"a4\", margin: 2cm)\n"));
        assert!(
            source.ends_with("#par(\"Primo #a\")\n\n#v(6pt)\n#v(6pt)\n#par(\"Secondo ]\")\n\n")
        );
    }
}
