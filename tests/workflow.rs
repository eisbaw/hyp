use hyp::{
    model::*,
    store::{Change, Store, decode, encode},
};
use tempfile::TempDir;
fn project() -> (TempDir, Store) {
    let dir = TempDir::new().unwrap();
    let store = Store::init(dir.path()).unwrap();
    (dir, store)
}
fn hypothesis() -> Record {
    Record::new(
        "DMA timeout is caused by cache coherency",
        Data::Hypothesis {
            scope: "rev C".into(),
            assumptions: String::new(),
            lifecycle: Lifecycle::Draft,
            untestable_reason: String::new(),
        },
    )
}
fn create(store: &Store, r: &Record) {
    store
        .commit(vec![Change::Create { record: r.clone() }], None)
        .unwrap();
}
fn evidence() -> Record {
    Record::new(
        "Timeout with cache disabled",
        Data::Evidence {
            source: "logs/run142.txt".into(),
            locator: "line 42".into(),
            observed_at: String::new(),
            attachments: vec![],
        },
    )
}
fn assess(h: &Record, e: &Record, f: &Record) -> Record {
    let mut r = Record::new(
        "Falsified",
        Data::Assessment {
            hypothesis: h.id.clone(),
            judgment: Judgment::Falsified,
            confidence: Some(0.1),
            evidence: vec![e.id.clone()],
            criterion: Some(f.id.clone()),
            based_on: String::new(),
            supersedes: vec![],
        },
    );
    r.body = "The failure satisfies the stated rejection criterion.".into();
    r
}
fn link(from: &Record, to: &Record, relation: Relation) -> Record {
    let mut r = Record::new(
        "Interpretation",
        Data::Link {
            from: from.id.clone(),
            to: to.id.clone(),
            relation,
        },
    );
    r.body = "Observed under controlled conditions.".into();
    r
}

#[test]
fn markdown_roundtrip_preserves_notes_and_rejects_unknown_fields() {
    let mut h = hypothesis();
    h.body = "# Notes\n\n---\nLiteral YAML boundary in body.\nUnicode: blåbær 🧪\n".into();
    let raw = encode(&h).unwrap();
    assert_eq!(decode(&raw).unwrap().body, h.body);
    assert!(decode(&raw.replacen("scope:", "typo_scope:", 1)).is_err());
}
#[test]
fn complete_workflow_preserves_assessment_and_detects_new_evidence() {
    let (_d, store) = project();
    let h = hypothesis();
    create(&store, &h);
    let f = Record::new(
        "Timeout with cache disabled",
        Data::Criterion {
            hypothesis: h.id.clone(),
        },
    );
    create(&store, &f);
    let e = evidence();
    create(&store, &e);
    create(&store, &link(&e, &h, Relation::Contradicts));
    let a = assess(&h, &e, &f);
    create(&store, &a);
    let s = store.snapshot().unwrap();
    assert_eq!(s.hypotheses[&h.id].judgment, Judgment::Falsified);
    assert!(!s.hypotheses[&h.id].needs_review);
    let e2 = evidence();
    create(&store, &e2);
    create(&store, &link(&e2, &h, Relation::Qualifies));
    let s = store.snapshot().unwrap();
    assert!(s.hypotheses[&h.id].needs_review);
    assert_eq!(s.hypotheses[&h.id].judgment, Judgment::Falsified);
    let mut a2 = assess(&h, &e2, &f);
    if let Data::Assessment { judgment, .. } = &mut a2.data {
        *judgment = Judgment::Weakened;
    }
    create(&store, &a2);
    let s = store.snapshot().unwrap();
    assert!(!s.hypotheses[&h.id].needs_review);
    assert_eq!(s.hypotheses[&h.id].assessment_ids, vec![a2.id]);
    assert!(s.find(&a.id).is_ok());
}
#[test]
fn unlinked_assessment_evidence_changes_trigger_review() {
    let (_d, store) = project();
    let h = hypothesis();
    create(&store, &h);
    let f = Record::new(
        "Reject if timeout",
        Data::Criterion {
            hypothesis: h.id.clone(),
        },
    );
    create(&store, &f);
    let e = evidence();
    create(&store, &e);
    create(&store, &assess(&h, &e, &f));
    let s = store.snapshot().unwrap();
    let entry = s.find(&e.id).unwrap();
    let mut changed = entry.record.clone();
    changed.body = "The source was retracted".into();
    store
        .commit(
            vec![Change::Update {
                record: changed,
                expected_revision: entry.revision.clone(),
            }],
            None,
        )
        .unwrap();
    assert!(store.snapshot().unwrap().hypotheses[&h.id].needs_review);
}
#[test]
fn stale_object_and_project_writes_are_rejected() {
    let (_d, store) = project();
    let h = hypothesis();
    create(&store, &h);
    let s = store.snapshot().unwrap();
    let e = s.find(&h.id).unwrap();
    let mut r = e.record.clone();
    r.title = "Changed once".into();
    store
        .commit(
            vec![Change::Update {
                record: r.clone(),
                expected_revision: e.revision.clone(),
            }],
            None,
        )
        .unwrap();
    r.title = "Stale overwrite".into();
    assert!(
        store
            .commit(
                vec![Change::Update {
                    record: r,
                    expected_revision: e.revision.clone()
                }],
                None
            )
            .unwrap_err()
            .to_string()
            .starts_with("conflict:")
    );
    assert!(
        store
            .commit(
                vec![Change::Create {
                    record: hypothesis()
                }],
                Some(&s.revision)
            )
            .is_err()
    );
}
#[test]
fn invalid_batch_is_all_or_nothing() {
    let (_d, store) = project();
    let h = hypothesis();
    let mut e = evidence();
    if let Data::Evidence { source, .. } = &mut e.data {
        source.clear();
    }
    assert!(
        store
            .commit(
                vec![Change::Create { record: h }, Change::Create { record: e }],
                None
            )
            .is_err()
    );
    assert!(store.snapshot().unwrap().objects.is_empty());
}
#[test]
fn concurrent_writers_do_not_lose_objects() {
    let (_d, store) = project();
    let threads: Vec<_> = (0..12)
        .map(|_| {
            let store = store.clone();
            std::thread::spawn(move || create(&store, &hypothesis()))
        })
        .collect();
    for t in threads {
        t.join().unwrap();
    }
    assert_eq!(store.snapshot().unwrap().objects.len(), 12);
}
#[test]
fn experiments_freeze_full_claim_and_prediction_and_runs_keep_plan() {
    let (_d, store) = project();
    let h = hypothesis();
    create(&store, &h);
    let p = Record::new(
        "No failure",
        Data::Prediction {
            hypothesis: h.id.clone(),
            conditions: "10,000 cycles".into(),
        },
    );
    create(&store, &p);
    let s = store.snapshot().unwrap();
    let mut x = Record::new(
        "Test cache",
        Data::Experiment {
            hypothesis: h.id.clone(),
            targets: vec![s.find(&p.id).unwrap().frozen()],
            status: ExperimentStatus::Planned,
        },
    );
    x.body = "Original procedure".into();
    create(&store, &x);
    let s = store.snapshot().unwrap();
    let old = s.find(&p.id).unwrap();
    let mut changed = old.record.clone();
    if let Data::Prediction { conditions, .. } = &mut changed.data {
        *conditions = "5 cycles".into();
    }
    store
        .commit(
            vec![Change::Update {
                record: changed,
                expected_revision: old.revision.clone(),
            }],
            None,
        )
        .unwrap();
    let s = store.snapshot().unwrap();
    if let Data::Experiment { targets, .. } = &s.find(&x.id).unwrap().record.data {
        assert_eq!(targets.len(), 2);
        assert!(targets[1].body.contains("10,000 cycles"));
        assert!(!targets[1].body.contains("5 cycles"));
    } else {
        panic!();
    }
    let run = Record::new(
        "Run 1",
        Data::Run {
            experiment: x.id.clone(),
            plan: s.find(&x.id).unwrap().frozen(),
            outcome: Outcome::Inconclusive,
            evidence: vec![],
        },
    );
    create(&store, &run);
    let s = store.snapshot().unwrap();
    let entry = s.find(&x.id).unwrap();
    let mut changed = entry.record.clone();
    changed.body = "Revised procedure".into();
    store
        .commit(
            vec![Change::Update {
                record: changed,
                expected_revision: entry.revision.clone(),
            }],
            None,
        )
        .unwrap();
    let s = store.snapshot().unwrap();
    if let Data::Run { plan, .. } = &s.find(&run.id).unwrap().record.data {
        assert!(plan.body.contains("Original procedure"));
    } else {
        panic!();
    }
}
#[test]
fn investigation_requires_criterion_and_referenced_objects_cannot_disappear() {
    let (_d, store) = project();
    let mut h = hypothesis();
    if let Data::Hypothesis { lifecycle, .. } = &mut h.data {
        *lifecycle = Lifecycle::Investigating;
    }
    assert!(
        store
            .commit(vec![Change::Create { record: h.clone() }], None)
            .is_err()
    );
    let f = Record::new(
        "Reject if X",
        Data::Criterion {
            hypothesis: h.id.clone(),
        },
    );
    store
        .commit(
            vec![
                Change::Create { record: h.clone() },
                Change::Create { record: f.clone() },
            ],
            None,
        )
        .unwrap();
    let s = store.snapshot().unwrap();
    assert!(
        store
            .commit(
                vec![Change::Archive {
                    id: f.id.clone(),
                    archived: true,
                    expected_revision: s.find(&f.id).unwrap().revision.clone()
                }],
                None
            )
            .is_err()
    );
}
#[test]
fn falsification_requires_evidence_and_criterion() {
    let (_d, store) = project();
    let h = hypothesis();
    create(&store, &h);
    let mut a = Record::new(
        "Rejected",
        Data::Assessment {
            hypothesis: h.id,
            judgment: Judgment::Falsified,
            confidence: Some(0.1),
            evidence: vec![],
            criterion: None,
            based_on: String::new(),
            supersedes: vec![],
        },
    );
    a.body = "A feeling".into();
    assert!(
        store
            .commit(vec![Change::Create { record: a }], None)
            .unwrap_err()
            .to_string()
            .contains("criterion and evidence")
    );
}
#[test]
fn dependency_cycles_rejected_but_competing_hypotheses_allowed() {
    let (_d, store) = project();
    let h = hypothesis();
    let h2 = hypothesis();
    create(&store, &h);
    create(&store, &h2);
    create(&store, &link(&h, &h2, Relation::DependsOn));
    assert!(
        store
            .commit(
                vec![Change::Create {
                    record: link(&h2, &h, Relation::DependsOn)
                }],
                None
            )
            .is_err()
    );
    create(&store, &link(&h2, &h, Relation::CompetesWith));
}
#[test]
fn manual_edits_are_detected_and_invalid_files_block_writes() {
    let (_d, store) = project();
    let h = hypothesis();
    create(&store, &h);
    let before = store.snapshot().unwrap();
    let path = store.root.join(format!("hyp/hypotheses/{}.md", h.id));
    let raw = std::fs::read_to_string(&path).unwrap();
    std::fs::write(&path, raw.replace("rev C", "rev D")).unwrap();
    assert_ne!(before.revision, store.snapshot().unwrap().revision);
    std::fs::write(&path, "malformed").unwrap();
    let s = store.snapshot().unwrap();
    assert_eq!(s.diagnostics[0].severity, "error");
    assert!(
        store
            .commit(
                vec![Change::Create {
                    record: hypothesis()
                }],
                None
            )
            .is_err()
    );
}
#[test]
fn interrupted_transaction_rolls_forward() {
    let (_d, store) = project();
    let h = hypothesis();
    let journal = serde_json::json!({format!("hypotheses/{}.md",h.id):encode(&h).unwrap()});
    std::fs::write(
        store.root.join(".hyp/transaction.json"),
        serde_json::to_vec(&journal).unwrap(),
    )
    .unwrap();
    let s = store.snapshot().unwrap();
    assert_eq!(s.objects.len(), 1);
    assert!(!store.root.join(".hyp/transaction.json").exists());
}
#[test]
fn immutable_assessments_reject_updates() {
    let (_d, store) = project();
    hyp::cli::seed_demo(&store).unwrap();
    let s = store.snapshot().unwrap();
    let a = s
        .objects
        .iter()
        .find(|e| matches!(e.record.data, Data::Assessment { .. }))
        .unwrap();
    let mut r = a.record.clone();
    r.body = "Rewrite history".into();
    assert!(
        store
            .commit(
                vec![Change::Update {
                    record: r,
                    expected_revision: a.revision.clone()
                }],
                None
            )
            .is_err()
    );
}
#[test]
fn archive_restore_delete_and_attachment_integrity() {
    let (dir, store) = project();
    let e = evidence();
    create(&store, &e);
    let attachment = dir.path().join("log.txt");
    std::fs::write(&attachment, "timeout").unwrap();
    let s = store.attach(&e.id, &attachment).unwrap();
    s.assert_healthy().unwrap();
    let entry = s.find(&e.id).unwrap();
    let dest = if let Data::Evidence { attachments, .. } = &entry.record.data {
        store.root.join("hyp").join(&attachments[0].path)
    } else {
        panic!();
    };
    std::fs::write(&dest, "modified").unwrap();
    assert!(store.snapshot().unwrap().assert_healthy().is_err());
    std::fs::write(&dest, "timeout").unwrap();
    let s = store.snapshot().unwrap();
    let e = s.find(&e.id).unwrap();
    let s = store
        .commit(
            vec![Change::Archive {
                id: e.record.id.clone(),
                archived: true,
                expected_revision: e.revision.clone(),
            }],
            None,
        )
        .unwrap();
    let e = s.find(&e.record.id).unwrap();
    store
        .commit(
            vec![Change::Delete {
                id: e.record.id.clone(),
                expected_revision: e.revision.clone(),
            }],
            None,
        )
        .unwrap();
    assert!(store.snapshot().unwrap().objects.is_empty());
}
#[test]
fn export_escapes_script_injection() {
    let (_d, store) = project();
    let mut h = hypothesis();
    h.title = "</script><script>alert(1)</script>".into();
    create(&store, &h);
    let html = hyp::web::export_html(&store.snapshot().unwrap()).unwrap();
    assert!(!html.contains(&h.title));
    assert!(html.contains("\\u003c/script\\u003e"));
    assert!(!html.contains("src=\"/app.js\""));
    assert!(!html.contains("href=\"/style.css\""));
}
#[cfg(unix)]
#[test]
fn storage_symlinks_are_rejected() {
    let (dir, store) = project();
    let h = hypothesis();
    create(&store, &h);
    let path = store.root.join(format!("hyp/hypotheses/{}.md", h.id));
    let outside = dir.path().join("outside.md");
    std::fs::rename(&path, &outside).unwrap();
    std::os::unix::fs::symlink(&outside, &path).unwrap();
    assert!(store.snapshot().unwrap().assert_healthy().is_err());
}

#[test]
fn merged_assessment_branches_require_explicit_reconciliation() {
    let (_dir, store) = project();
    hyp::cli::seed_demo(&store).unwrap();
    let s = store.snapshot().unwrap();
    let a = s
        .objects
        .iter()
        .find(|e| matches!(e.record.data, Data::Assessment { .. }))
        .unwrap();
    let mut branch = a.record.clone();
    branch.id = format!("A-{}", uuid::Uuid::new_v4());
    let h = branch.data.owner().unwrap().to_string();
    if let Data::Assessment { judgment, .. } = &mut branch.data {
        *judgment = Judgment::Supported;
    }
    std::fs::write(
        store.root.join(format!("hyp/assessments/{}.md", branch.id)),
        encode(&branch).unwrap(),
    )
    .unwrap();
    let merged = store.snapshot().unwrap();
    assert_eq!(merged.hypotheses[&h].assessment_ids.len(), 2);
    assert!(merged.hypotheses[&h].needs_review);
    let mut reconciliation = a.record.clone();
    reconciliation.id = format!("A-{}", uuid::Uuid::new_v4());
    reconciliation.body = "Reconcile branch assessments after checking the source.".into();
    create(&store, &reconciliation);
    let s = store.snapshot().unwrap();
    assert_eq!(s.hypotheses[&h].assessment_ids, vec![reconciliation.id]);
    assert!(!s.hypotheses[&h].needs_review);
}
#[test]
fn invalid_attachments_cannot_be_committed() {
    let (_dir, store) = project();
    let mut e = evidence();
    if let Data::Evidence { attachments, .. } = &mut e.data {
        attachments.push(Attachment {
            path: "assets/missing".into(),
            sha256: "0".repeat(64),
        });
    }
    assert!(
        store
            .commit(vec![Change::Create { record: e }], None)
            .is_err()
    );
    assert!(store.snapshot().unwrap().objects.is_empty());
}
#[test]
fn archived_references_still_prevent_deletion() {
    let (_dir, store) = project();
    let h = hypothesis();
    create(&store, &h);
    let e = evidence();
    create(&store, &e);
    let l = link(&e, &h, Relation::Supports);
    create(&store, &l);
    let s = store.snapshot().unwrap();
    let s = store
        .commit(
            vec![
                Change::Archive {
                    id: e.id.clone(),
                    archived: true,
                    expected_revision: s.find(&e.id).unwrap().revision.clone(),
                },
                Change::Archive {
                    id: l.id.clone(),
                    archived: true,
                    expected_revision: s.find(&l.id).unwrap().revision.clone(),
                },
            ],
            None,
        )
        .unwrap();
    assert!(
        store
            .commit(
                vec![Change::Delete {
                    id: e.id.clone(),
                    expected_revision: s.find(&e.id).unwrap().revision.clone()
                }],
                None
            )
            .unwrap_err()
            .to_string()
            .contains("referenced")
    );
}
