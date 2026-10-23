use doene_storage::{Storage, aux};

#[test]
fn trash_activity_and_settings_round_trip() {
    let s = Storage::open_in_memory().unwrap();
    let c = s.conn();
    for (id, at) in [("t1", "2025-01-01T00:00:00.000Z"), ("t2", "2025-01-02T00:00:00.000Z")] {
        aux::insert_trash(c, &aux::TrashRow { id: id.into(), kind: "task".into(), label: Some(id.into()), data: "{}".into(), deleted_at: at.into() }).unwrap();
    }
    let ids: Vec<String> = aux::trash(c).unwrap().into_iter().map(|r| r.id).collect();
    assert_eq!(ids, ["t2", "t1"]);
    assert_eq!(aux::purge_trash_before(c, "2025-01-02T00:00:00.000Z").unwrap(), 1);
    assert!(aux::trash_entry(c, "t1").unwrap().is_none());
    aux::delete_trash(c, "t2").unwrap();
    assert_eq!(aux::empty_trash(c).unwrap(), 0);

    aux::insert_activity(c, "a1", Some("x"), "2025-01-01T00:00:00.000Z", "created", None).unwrap();
    aux::insert_activity(c, "a2", Some("x"), "2025-01-01T00:00:00.000Z", "deleted", Some("d")).unwrap();
    let acts = aux::activity_of(c, "x").unwrap();
    assert_eq!(acts[0].action, "deleted");
    assert_eq!(acts.len(), 2);

    assert_eq!(aux::setting(c, "k").unwrap(), None);
    aux::set_setting(c, "k", "1").unwrap();
    aux::set_setting(c, "k", "2").unwrap();
    assert_eq!(aux::setting(c, "k").unwrap().as_deref(), Some("2"));
}
