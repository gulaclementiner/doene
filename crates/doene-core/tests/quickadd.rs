//! Parity port of Gör't `test/quickadd.test.js` — the quick-add wrapper that
//! turns raw composer text + the current view into final task fields.

use doene_core::dates::Date;
use doene_core::entities::{MatchMode, Tag, View};
use doene_core::nlp::ImportanceLevel;
use doene_core::quickadd::{QuickAddOptions, QuickAddView, quick_add};

const TODAY: &str = "2025-03-12";

fn views() -> Vec<View> {
    let mk = |id: &str, name: &str, tags: &[&str], mode: MatchMode| View {
        id: id.into(),
        name: name.into(),
        emoji: None,
        match_mode: mode,
        show_done: false,
        position: 0.0,
        tags: tags
            .iter()
            .map(|t| Tag::new(t).expect("valid tag"))
            .collect(),
    };
    vec![
        mk("V1", "Work", &["work", "urgent"], MatchMode::All),
        mk("V2", "Home", &["home", "errands"], MatchMode::Any),
        mk("V3", "Everything", &[], MatchMode::Any),
    ]
}

fn qa(raw: &str, view: QuickAddView) -> doene_core::quickadd::TaskDraft {
    quick_add(
        raw,
        Some(&view),
        &QuickAddOptions {
            today: Date::parse(TODAY).expect("valid fixture date"),
            views: views(),
            literal: false,
            locale: None,
        },
    )
    .expect("quick add succeeds")
}

fn qa_literal(raw: &str) -> doene_core::quickadd::TaskDraft {
    quick_add(
        raw,
        Some(&QuickAddView::Smart("all".into())),
        &QuickAddOptions {
            today: Date::parse(TODAY).expect("valid fixture date"),
            views: views(),
            literal: true,
            locale: None,
        },
    )
    .expect("quick add succeeds")
}

#[test]
fn plain_text_has_no_view_defaults_or_tags() {
    let b = qa("Buy milk", QuickAddView::Smart("all".into()));
    assert_eq!(b.subject, "Buy milk");
    assert!(!b.shortlisted);
    assert_eq!(b.importance, ImportanceLevel::Normal);
    assert_eq!(b.due_date, None);
    assert_eq!(b.recurrence, None);
    assert!(b.tags.is_empty());
}

#[test]
fn view_defaults_my_day_shortlisted_important_planned() {
    assert!(qa("Water plants", QuickAddView::Smart("my-day".into())).shortlisted);
    assert!(qa("Water plants", QuickAddView::Smart("shortlisted".into())).shortlisted);
    assert_eq!(
        qa("Water plants", QuickAddView::Smart("important".into())).importance,
        ImportanceLevel::High
    );
    assert_eq!(
        qa("Water plants", QuickAddView::Smart("planned".into())).due_date,
        Some(Date::parse(TODAY).unwrap())
    );
}

#[test]
fn parsed_fields_beat_view_defaults_and_hash_word_becomes_a_tag() {
    let b = qa(
        "Pay rent tomorrow !low #home",
        QuickAddView::Smart("important".into()),
    );
    assert_eq!(b.due_date, Some(Date::parse("2025-03-13").unwrap()));
    assert_eq!(b.importance, ImportanceLevel::Low);
    let tags: Vec<&str> = b.tags.iter().map(|t| t.as_str()).collect();
    assert_eq!(tags, vec!["home"]);
}

#[test]
fn inside_a_view_a_new_task_is_tagged_to_stay_in_it() {
    // Every tag for "all", the first for "any".
    fn tags(b: &doene_core::quickadd::TaskDraft) -> Vec<&str> {
        b.tags.iter().map(|t| t.as_str()).collect()
    }
    assert_eq!(
        tags(&qa("x", QuickAddView::Custom("V1".into()))),
        vec!["work", "urgent"]
    );
    assert_eq!(
        tags(&qa("x", QuickAddView::Custom("V2".into()))),
        vec!["home"]
    );
    // A view with no tags adds none.
    assert!(tags(&qa("x", QuickAddView::Custom("V3".into()))).is_empty());
}

#[test]
fn view_tags_and_typed_tags_are_merged_without_duplicates_in_lower_case() {
    let b = qa("x #Home #later", QuickAddView::Custom("V2".into()));
    let tags: Vec<&str> = b.tags.iter().map(|t| t.as_str()).collect();
    assert_eq!(tags, vec!["home", "later"]);
}

#[test]
fn literal_mode_keeps_the_whole_text_as_the_subject() {
    let b = qa_literal("at 9am #home !high");
    assert_eq!(b.subject, "at 9am #home !high");
    assert_eq!(b.reminder_at, None);
    assert_eq!(b.importance, ImportanceLevel::Normal);
}

#[test]
fn recurrence_without_a_date_gets_the_first_occurrence_as_its_due_date() {
    let b = qa("Water plants every day", QuickAddView::Smart("all".into()));
    assert_eq!(
        b.recurrence.map(|r| r.kind),
        Some(doene_core::entities::RecurrenceType::Daily)
    );
    assert_eq!(b.due_date, Some(Date::parse(TODAY).unwrap()));
}

#[test]
fn empty_text_throws() {
    let err = quick_add(
        "   ",
        Some(&QuickAddView::Smart("all".into())),
        &QuickAddOptions {
            today: Date::parse(TODAY).expect("valid fixture date"),
            views: views(),
            literal: false,
            locale: None,
        },
    )
    .expect_err("empty text must fail");
    let msg = format!("{err:?}");
    assert!(msg.contains("text required"), "{msg}");
}
