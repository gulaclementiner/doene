use doene_core::{MatchMode, SortDirection, SortKey, SortLevel, Tag, View};
use doene_storage::{Storage, views};

fn sv(id: &str, pos: f64) -> views::SavedView {
    views::SavedView {
        view: View {
            id: id.into(),
            name: format!("View {id}"),
            emoji: Some("🏠".into()),
            match_mode: MatchMode::All,
            show_done: true,
            position: pos,
            tags: vec![Tag::new("b").unwrap(), Tag::new("a").unwrap()],
        },
        sorts: vec![
            SortLevel {
                by: SortKey::Due,
                dir: SortDirection::Desc,
            },
            SortLevel {
                by: SortKey::Name,
                dir: SortDirection::Asc,
            },
        ],
    }
}

#[test]
fn round_trip_update_delete() {
    let s = Storage::open_in_memory().unwrap();
    let c = s.conn();
    views::insert(c, &sv("v1", 1.0)).unwrap();
    views::insert(c, &sv("v2", 2.0)).unwrap();
    let got = views::get(c, "v1").unwrap().unwrap();
    let tags: Vec<_> = got.view.tags.iter().map(|t| t.as_str()).collect();
    assert_eq!(tags, ["a", "b"]);
    assert_eq!(got.sorts, sv("v1", 1.0).sorts);
    assert!(got.view.show_done && got.view.match_mode == MatchMode::All);

    let mut upd = sv("v1", 3.0);
    upd.sorts.truncate(1);
    upd.view.tags.truncate(1);
    views::update(c, &upd).unwrap();
    assert_eq!(views::get(c, "v1").unwrap().unwrap().sorts.len(), 1);
    let ids: Vec<_> = views::all(c)
        .unwrap()
        .into_iter()
        .map(|v| v.view.id)
        .collect();
    assert_eq!(ids, ["v2", "v1"]);
    assert_eq!(views::max_position(c).unwrap(), Some(3.0));

    views::delete(c, "v1").unwrap();
    assert!(views::get(c, "v1").unwrap().is_none());
    let n: i64 = c
        .query_row(
            "SELECT COUNT(*) FROM view_sorts WHERE view_id='v1'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(n, 0);
}
