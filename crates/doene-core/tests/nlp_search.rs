//! Parity fixtures ported 1:1 from Gör't `test/nlp-search.test.js` (the six
//! NLP tests; the search test lands with ticket #06's search module).

use doene_core::dates::Date;
use doene_core::nlp::{ParsedQuickAdd, ParseOptions, parse_quick_add};

const TODAY: &str = "2025-03-12"; // Wednesday

fn p(text: &str) -> ParsedQuickAdd {
    parse_quick_add(
        text,
        Date::parse(TODAY).expect("valid fixture date"),
        &ParseOptions::default(),
    )
}

#[test]
fn plain_text_is_untouched() {
    let r = p("Buy milk");
    assert_eq!(r.subject, "Buy milk");
    assert_eq!(r.due_date, None);
    assert!(r.chips.is_empty());
}
