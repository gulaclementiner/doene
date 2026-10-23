//! Slice 2: the task repository round-trips core entities through the plan §5 schema.

use doene_core::{
    Date, HydratedTask, Importance, RecurrenceRule, RecurrenceType, RecurrenceUnit, Step, Tag,
    Task, TaskStatus, Weekday,
};
use doene_storage::{Storage, tasks};

fn sample_task() -> Task {
    Task {
        id: "11111111-1111-4111-8111-111111111111".into(),
        subject: "Water plants".into(),
        status: TaskStatus::Open,
        importance: Importance::High,
        due_date: Some(Date::parse("2025-03-14").unwrap()),
        reminder_at: Some("2025-03-14T08:30".into()),
        body: Some("the ferns too".into()),
        focus_date: Some(Date::parse("2025-03-12").unwrap()),
        postponed_date: None,
        created_at: "2025-03-01T09:00:00.000Z".into(),
        completed_at: None,
        position: 2.5,
        spawned_from: None,
        parent_id: None,
        recurrence: Some(RecurrenceRule {
            kind: RecurrenceType::Custom,
            interval: Some(2),
            unit: Some(RecurrenceUnit::Week),
            days: Some(vec![Weekday::Mon, Weekday::Wed]),
        }),
    }
}

#[test]
fn task_round_trips_with_recurrence_tags_and_steps() {
    let store = Storage::open_in_memory().expect("open");
    let task = sample_task();
    let hydrated = HydratedTask {
        task: task.clone(),
        tags: vec![Tag::new("garden").unwrap(), Tag::new("home").unwrap()],
        steps: vec![Step {
            id: "22222222-2222-4222-8222-222222222222".into(),
            task_id: task.id.clone(),
            subject: "fetch can".into(),
            status: TaskStatus::Open,
            position: 1.0,
        }],
    };
    tasks::insert_task(store.conn(), &hydrated.task).expect("insert task");
    tasks::set_tags(store.conn(), &task.id, &hydrated.tags).expect("tags");
    tasks::insert_step(store.conn(), &hydrated.steps[0]).expect("step");

    let row = tasks::task(store.conn(), &task.id)
        .expect("read")
        .expect("present");
    assert_eq!(row, task);
    let hydrated_read = tasks::all_tasks(store.conn()).expect("all");
    assert_eq!(hydrated_read, vec![hydrated]);
}

#[test]
fn recurrence_is_stored_in_normalized_columns() {
    let store = Storage::open_in_memory().expect("open");
    tasks::insert_task(store.conn(), &sample_task()).expect("insert");
    let (kind, interval, unit, days): (String, i64, String, String) = store
        .conn()
        .query_row(
            "SELECT recurrence_type, recurrence_interval, recurrence_unit, recurrence_days FROM tasks",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        )
        .expect("row");
    assert_eq!(
        (kind.as_str(), interval, unit.as_str(), days.as_str()),
        ("custom", 2, "week", "1,3")
    );
}

#[test]
fn all_tasks_is_ordered_by_created_at_then_position() {
    let store = Storage::open_in_memory().expect("open");
    let mut late = sample_task();
    late.id = "b".into();
    late.created_at = "2025-03-02T00:00:00.000Z".into();
    late.recurrence = None;
    late.position = 1.0;
    let mut early = late.clone();
    early.id = "a".into();
    early.created_at = "2025-03-01T00:00:00.000Z".into();
    tasks::insert_task(store.conn(), &late).expect("late");
    tasks::insert_task(store.conn(), &early).expect("early");
    let ids: Vec<String> = tasks::all_tasks(store.conn())
        .expect("all")
        .into_iter()
        .map(|t| t.task.id)
        .collect();
    assert_eq!(ids, ["a", "b"]);
}

#[test]
fn deleting_a_task_cascades_steps_and_tags() {
    let store = Storage::open_in_memory().expect("open");
    let task = sample_task();
    tasks::insert_task(store.conn(), &task).expect("insert");
    tasks::set_tags(store.conn(), &task.id, &[Tag::new("x").unwrap()]).expect("tags");
    tasks::insert_step(
        store.conn(),
        &Step {
            id: "s".into(),
            task_id: task.id.clone(),
            subject: "s".into(),
            status: TaskStatus::Open,
            position: 1.0,
        },
    )
    .expect("step");
    tasks::delete_tasks(store.conn(), &[task.id.clone()]).expect("delete");
    let count = |sql: &str| -> i64 { store.conn().query_row(sql, [], |r| r.get(0)).unwrap() };
    assert_eq!(count("SELECT COUNT(*) FROM steps"), 0);
    assert_eq!(count("SELECT COUNT(*) FROM task_tags"), 0);
}

#[test]
fn deleting_an_origin_keeps_its_spawned_occurrence() {
    let store = Storage::open_in_memory().expect("open");
    let mut origin = sample_task();
    origin.recurrence = None;
    let mut spawn = origin.clone();
    spawn.id = "spawn".into();
    spawn.spawned_from = Some(origin.id.clone());
    tasks::insert_task(store.conn(), &origin).unwrap();
    tasks::insert_task(store.conn(), &spawn).unwrap();
    tasks::delete_tasks(store.conn(), &[origin.id.clone()]).expect("delete");
    let kept = tasks::task(store.conn(), "spawn").unwrap().expect("kept");
    assert_eq!(kept.spawned_from, None);
}

#[test]
fn subtree_descendants_and_sibling_order() {
    let store = Storage::open_in_memory().expect("open");
    let mut root = sample_task();
    root.recurrence = None;
    let mk = |id: &str, parent: &str, pos: f64| {
        let mut t = root.clone();
        t.id = id.into();
        t.parent_id = Some(parent.into());
        t.position = pos;
        t
    };
    tasks::insert_task(store.conn(), &root).unwrap();
    tasks::insert_task(store.conn(), &mk("b", &root.id, 2.0)).unwrap();
    tasks::insert_task(store.conn(), &mk("a", &root.id, 1.0)).unwrap();
    tasks::insert_task(store.conn(), &mk("c", "a", 1.0)).unwrap();
    assert_eq!(
        tasks::descendants(store.conn(), &root.id).unwrap(),
        ["a", "b", "c"]
    );
    let sib: Vec<String> = tasks::siblings(store.conn(), Some(&root.id))
        .unwrap()
        .into_iter()
        .map(|s| s.0)
        .collect();
    assert_eq!(sib, ["a", "b"]);
    // deleting the whole subtree in one statement honours the parent FK
    tasks::delete_tasks(
        store.conn(),
        &["a".into(), "b".into(), "c".into(), root.id.clone()],
    )
    .unwrap();
    assert!(tasks::all_tasks(store.conn()).unwrap().is_empty());
}
