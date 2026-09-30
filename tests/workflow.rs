use hyp::{
    model::*,
    store::{Change, Expected, Store, decode, encode},
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
/// Creates `r` with preconditions stated from a fresh read, as the CLI does.
fn create(store: &Store, r: &Record) {
    let seen = store.snapshot().unwrap();
    store
        .commit(vec![Change::create_seen(r.clone(), &seen)], None)
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
/// Decision-0003 item 2: an assessment may cite only evidence that was
/// already linked to the hypothesis, or to its criteria or predictions, when
/// its author read it. So the review token covers everything it cites.
#[test]
fn assessment_may_cite_only_evidence_already_linked() {
    let (_d, store) = project();
    let i = investigation(&store);
    let unlinked = evidence();
    create(&store, &unlinked);
    let seen = store.snapshot().unwrap();
    let err = rejected(
        &store,
        Change::create_seen(assess(&i.h, &unlinked, &i.f), &seen),
    );
    assert!(
        err.contains(&format!("hyp link {} {}", unlinked.id, i.h.id)),
        "{err}"
    );
    // Linking it in the same batch is not enough: the author never reviewed it.
    let err = store
        .commit(
            vec![
                Change::create_seen(link(&unlinked, &i.h, Relation::Supports), &seen),
                Change::create_seen(assess(&i.h, &unlinked, &i.f), &seen),
            ],
            None,
        )
        .unwrap_err();
    assert_eq!(hyp::cli::exit_code(&err), 1, "{err:#}");
    assert!(format!("{err:#}").contains("earlier write"), "{err:#}");
    // An archived link does not count.
    let criterion_link = link(&unlinked, &i.f, Relation::Supports);
    create(&store, &criterion_link);
    update(&store, &criterion_link.id, |r| r.archived = true);
    rejected(
        &store,
        Change::create_seen(assess(&i.h, &unlinked, &i.f), &store.snapshot().unwrap()),
    );
    // A link to one of its criteria counts, written first and then re-read.
    update(&store, &criterion_link.id, |r| r.archived = false);
    let s = store
        .commit(
            vec![Change::create_seen(
                assess(&i.h, &unlinked, &i.f),
                &store.snapshot().unwrap(),
            )],
            None,
        )
        .unwrap();
    assert!(!s.hypotheses[&i.h.id].needs_review);
}
/// Decision-0003 item 4, enforced for every writer when an assessment is
/// created. Stored assessments from before the rule stay valid.
#[test]
fn judgments_other_than_untested_require_evidence() {
    let (_d, store) = project();
    let i = investigation(&store);
    let without = |judgment: Judgment| {
        let mut a = assess(&i.h, &i.e, &i.f);
        if let Data::Assessment {
            judgment: j,
            evidence,
            criterion,
            ..
        } = &mut a.data
        {
            *j = judgment;
            evidence.clear();
            *criterion = None;
        }
        a
    };
    for judgment in [
        Judgment::Inconclusive,
        Judgment::Supported,
        Judgment::Weakened,
    ] {
        let err = rejected(
            &store,
            Change::create_seen(without(judgment), &store.snapshot().unwrap()),
        );
        let article = if judgment == Judgment::Inconclusive {
            "an"
        } else {
            "a"
        };
        assert!(
            err.contains(&format!(
                "{article} {judgment} assessment must cite evidence"
            )),
            "{err}"
        );
    }
    let untested = without(Judgment::Untested);
    create(&store, &untested);
    // An evidence-less judgment written by an older hyp is not an error.
    let mut legacy = without(Judgment::Supported);
    if let Data::Assessment {
        based_on,
        supersedes,
        ..
    } = &mut legacy.data
    {
        *based_on = "0".repeat(64);
        supersedes.push(untested.id.clone());
    }
    std::fs::write(
        store.root.join(format!("hyp/assessments/{}.md", legacy.id)),
        encode(&legacy).unwrap(),
    )
    .unwrap();
    let s = store.snapshot().unwrap();
    s.assert_writable().unwrap();
    assert_eq!(s.hypotheses[&i.h.id].judgment, Judgment::Supported);
    create(&store, &hypothesis());
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
                    record: hypothesis(),
                    expected: None,
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
                vec![
                    Change::Create {
                        record: h,
                        expected: None,
                    },
                    Change::Create {
                        record: e,
                        expected: None,
                    }
                ],
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
            .commit(
                vec![Change::Create {
                    record: h.clone(),
                    expected: None,
                }],
                None
            )
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
                Change::Create {
                    record: h.clone(),
                    expected: None,
                },
                Change::Create {
                    record: f.clone(),
                    expected: None,
                },
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
    let i = investigation(&store);
    let mut a = assess(&i.h, &i.e, &i.f);
    if let Data::Assessment { criterion, .. } = &mut a.data {
        *criterion = None;
    }
    let err = rejected(&store, Change::create_seen(a, &store.snapshot().unwrap()));
    assert!(err.contains("criterion and evidence"), "{err}");
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
                    record: link(&h2, &h, Relation::DependsOn),
                    expected: None,
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
                    record: hypothesis(),
                    expected: None,
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
    let s = store.attach(&e.id, &attachment).unwrap().snapshot;
    s.assert_writable().unwrap();
    let entry = s.find(&e.id).unwrap();
    let dest = if let Data::Evidence { attachments, .. } = &entry.record.data {
        store.root.join("hyp").join(&attachments[0].path)
    } else {
        panic!();
    };
    std::fs::write(&dest, "modified").unwrap();
    assert!(store.snapshot().unwrap().assert_writable().is_err());
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
    assert!(store.snapshot().unwrap().assert_writable().is_err());
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
            .commit(
                vec![Change::Create {
                    record: e,
                    expected: None,
                }],
                None
            )
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
    let err = store
        .commit(
            vec![Change::Delete {
                id: e.id.clone(),
                expected_revision: s.find(&e.id).unwrap().revision.clone(),
            }],
            None,
        )
        .unwrap_err()
        .to_string();
    assert!(
        err.contains(&format!(
            "cannot delete {}: these records refer to it",
            e.id
        )) && err.contains(&l.id),
        "{err}"
    );
}
#[test]
fn conflicts_are_classified_by_type_even_when_wrapped_in_context() {
    let (_d, store) = project();
    let stale = store
        .commit(
            vec![Change::Create {
                record: hypothesis(),
                expected: None,
            }],
            Some("stale-revision"),
        )
        .unwrap_err();
    assert!(format!("{stale:#}").starts_with("conflict:"));
    assert_eq!(hyp::cli::exit_code(&stale), 3);
    let wrapped = stale.context("while applying a batch");
    assert_eq!(hyp::cli::exit_code(&wrapped), 3);
    let ordinary = anyhow::anyhow!("conflict: looks like one but is only text");
    assert_eq!(hyp::cli::exit_code(&ordinary), 1);
}
#[test]
fn missing_data_directories_are_recreated_but_non_directories_rejected() {
    let (_d, store) = project();
    create(&store, &hypothesis());
    std::fs::remove_dir(store.root.join("hyp/assets")).unwrap();
    std::fs::remove_dir(store.root.join("hyp/gaps")).unwrap();
    std::fs::remove_dir_all(store.root.join(".hyp")).unwrap();
    let reopened = Store::open(&store.root).unwrap();
    assert_eq!(reopened.snapshot().unwrap().objects.len(), 1);
    assert!(store.root.join("hyp/assets").is_dir());
    assert!(store.root.join("hyp/gaps").is_dir());
    assert!(store.root.join(".hyp/.gitignore").is_file());
    let links = store.root.join("hyp/links");
    std::fs::remove_dir(&links).unwrap();
    std::fs::write(&links, "not a directory").unwrap();
    let err = format!("{:#}", Store::open(&store.root).unwrap_err());
    assert!(err.contains("hyp/links"), "{err}");
    std::fs::remove_file(&links).unwrap();
    std::os::unix::fs::symlink(store.root.join("hyp/gaps"), &links).unwrap();
    let err = format!("{:#}", Store::open(&store.root).unwrap_err());
    assert!(err.contains("hyp/links"), "{err}");
}
#[test]
fn io_errors_name_the_offending_path() {
    let (d, store) = project();
    let moved = d.path().join("moved");
    std::fs::rename(store.root.join("hyp"), &moved).unwrap();
    let err = format!("{:#}", store.snapshot().unwrap_err());
    assert!(
        err.contains(&store.root.join("hyp").display().to_string()),
        "{err}"
    );
    std::fs::rename(&moved, store.root.join("hyp")).unwrap();
    std::fs::write(store.root.join("hyp/config.toml"), "schema_version = \"x\"").unwrap();
    let err = format!("{:#}", Store::open(&store.root).unwrap_err());
    assert!(err.contains("hyp/config.toml"), "{err}");
}

/// A hypothesis with a criterion, a prediction, linked evidence and an
/// experiment: everything an assessment, experiment or run can depend on.
struct Investigation {
    h: Record,
    f: Record,
    e: Record,
    x: Record,
}
fn investigation(store: &Store) -> Investigation {
    let h = hypothesis();
    create(store, &h);
    let f = Record::new(
        "Reject if timeout persists without cache",
        Data::Criterion {
            hypothesis: h.id.clone(),
        },
    );
    let p = Record::new(
        "No failure after clean + invalidate",
        Data::Prediction {
            hypothesis: h.id.clone(),
            conditions: "10,000 transfers".into(),
        },
    );
    let e = evidence();
    for r in [&f, &p, &e, &link(&e, &h, Relation::Contradicts)] {
        create(store, r);
    }
    let x = Record::new(
        "Disable cache",
        Data::Experiment {
            hypothesis: h.id.clone(),
            targets: vec![FrozenRef {
                id: p.id.clone(),
                ..FrozenRef::default()
            }],
            status: ExperimentStatus::Planned,
        },
    );
    create(store, &x);
    Investigation { h, f, e, x }
}
fn experiment(i: &Investigation) -> Record {
    Record::new(
        "Replicate",
        Data::Experiment {
            hypothesis: i.h.id.clone(),
            targets: vec![FrozenRef {
                id: i.f.id.clone(),
                ..FrozenRef::default()
            }],
            status: ExperimentStatus::Planned,
        },
    )
}
fn run(i: &Investigation) -> Record {
    Record::new(
        "Run 1",
        Data::Run {
            experiment: i.x.id.clone(),
            plan: FrozenRef {
                id: i.x.id.clone(),
                ..FrozenRef::default()
            },
            outcome: Outcome::Observed,
            evidence: vec![],
        },
    )
}
fn update(store: &Store, id: &str, edit: impl FnOnce(&mut Record)) {
    let s = store.snapshot().unwrap();
    let entry = s.find(id).unwrap();
    let mut r = entry.record.clone();
    edit(&mut r);
    store
        .commit(
            vec![Change::Update {
                record: r,
                expected_revision: entry.revision.clone(),
            }],
            None,
        )
        .unwrap();
}
/// Commits `change` alone and returns the error, asserting it is a conflict
/// (CLI exit code 3) and that nothing was written.
fn conflict(store: &Store, change: Change) -> String {
    let before = store.snapshot().unwrap().revision;
    let err = store.commit(vec![change], None).unwrap_err();
    assert_eq!(hyp::cli::exit_code(&err), 3, "{err:#}");
    assert_eq!(store.snapshot().unwrap().revision, before);
    format!("{err:#}")
}
/// Commits `change` alone and returns the error, asserting it is an ordinary
/// error (CLI exit code 1): retrying without a fix cannot succeed.
fn rejected(store: &Store, change: Change) -> String {
    let err = store.commit(vec![change], None).unwrap_err();
    assert_eq!(hyp::cli::exit_code(&err), 1, "{err:#}");
    format!("{err:#}")
}

#[test]
fn unrelated_concurrent_writes_do_not_conflict_with_any_kind_of_write() {
    let (_d, store) = project();
    let i = investigation(&store);
    let seen = store.snapshot().unwrap();
    // Another agent works on another hypothesis in the meantime.
    let other = hypothesis();
    create(&store, &other);
    let other_evidence = evidence();
    create(&store, &other_evidence);
    create(&store, &link(&other_evidence, &other, Relation::Supports));
    assert_ne!(store.snapshot().unwrap().revision, seen.revision);
    let entry = seen.find(&i.h.id).unwrap();
    let mut renamed = entry.record.clone();
    renamed.title = "Renamed after reading".into();
    let assessment = assess(&i.h, &i.e, &i.f);
    let s = store
        .commit(
            vec![
                Change::create_seen(experiment(&i), &seen),
                Change::create_seen(run(&i), &seen),
                Change::create_seen(evidence(), &seen),
                // Last, so that its fingerprint covers the batch's own records.
                Change::create_seen(assessment.clone(), &seen),
            ],
            None,
        )
        .unwrap();
    assert!(!s.hypotheses[&i.h.id].needs_review);
    assert_eq!(s.hypotheses[&i.h.id].assessment_ids, vec![assessment.id]);
    let planned = seen.find(&i.x.id).unwrap();
    store
        .commit(
            vec![
                Change::Update {
                    record: renamed,
                    expected_revision: entry.revision.clone(),
                },
                Change::Archive {
                    id: planned.record.id.clone(),
                    archived: true,
                    expected_revision: planned.revision.clone(),
                },
            ],
            None,
        )
        .unwrap();
    // The whole-project precondition is still honoured when a caller asks for it.
    assert!(
        conflict_with_project(&store, &seen.revision).contains("project changed"),
        "whole-project revision ignored"
    );
}
fn conflict_with_project(store: &Store, revision: &str) -> String {
    let err = store
        .commit(
            vec![Change::create_seen(
                hypothesis(),
                &store.snapshot().unwrap(),
            )],
            Some(revision),
        )
        .unwrap_err();
    assert_eq!(hyp::cli::exit_code(&err), 3);
    format!("{err:#}")
}

#[test]
fn assessment_is_a_conflict_when_the_hypothesis_changed_after_it_was_read() {
    let (_d, store) = project();
    let i = investigation(&store);
    let seen = store.snapshot().unwrap();
    let assessment = Change::create_seen(assess(&i.h, &i.e, &i.f), &seen);
    // Counter-evidence arrives while the author writes the assessment.
    let counter = evidence();
    create(&store, &counter);
    create(&store, &link(&counter, &i.h, Relation::Contradicts));
    let err = conflict(&store, assessment.clone());
    assert!(
        err.contains(&format!("hypothesis {} changed", i.h.id)) && err.contains("review_token"),
        "{err}"
    );
    // Re-reading and reviewing the new state is what makes it succeed.
    let s = store
        .commit(
            vec![Change::create_seen(
                assess(&i.h, &counter, &i.f),
                &store.snapshot().unwrap(),
            )],
            None,
        )
        .unwrap();
    assert!(!s.hypotheses[&i.h.id].needs_review);
}

#[test]
fn assessment_is_a_conflict_when_another_assessment_arrived_after_it_was_read() {
    let (_d, store) = project();
    let i = investigation(&store);
    let seen = store.snapshot().unwrap();
    let mine = Change::create_seen(assess(&i.h, &i.e, &i.f), &seen);
    // Assessments are not part of the fingerprint: only the heads reveal this one.
    create(&store, &assess(&i.h, &i.e, &i.f));
    assert_eq!(
        store.snapshot().unwrap().hypotheses[&i.h.id].fingerprint,
        seen.hypotheses[&i.h.id].fingerprint
    );
    let err = conflict(&store, mine);
    assert!(err.contains("review_token"), "{err}");
}

/// Preconditions refer to the state the caller read, not to the batch so far:
/// the author's own earlier changes in the batch are not conflicts (review P3, P5).
#[test]
fn own_earlier_changes_in_the_batch_are_not_conflicts() {
    let (_d, store) = project();
    let i = investigation(&store);
    let seen = store.snapshot().unwrap();
    let entry = seen.get(&i.e.id).unwrap();
    let mut clarified = entry.record.clone();
    clarified.body = "Clarified by the author".into();
    let s = store
        .commit(
            vec![
                Change::Update {
                    record: clarified,
                    expected_revision: entry.revision.clone(),
                },
                Change::create_seen(assess(&i.h, &i.e, &i.f), &seen),
            ],
            None,
        )
        .unwrap();
    assert!(!s.hypotheses[&i.h.id].needs_review);
    // Two assessments of one hypothesis in a batch form a chain, not branches.
    let seen = store.snapshot().unwrap();
    let first = assess(&i.h, &i.e, &i.f);
    let second = assess(&i.h, &i.e, &i.f);
    let s = store
        .commit(
            vec![
                Change::create_seen(first.clone(), &seen),
                Change::create_seen(second.clone(), &seen),
            ],
            None,
        )
        .unwrap();
    assert_eq!(s.hypotheses[&i.h.id].assessment_ids, vec![second.id]);
    assert!(!s.hypotheses[&i.h.id].needs_review);
}

#[test]
fn stated_records_deleted_after_the_read_are_conflicts() {
    let (_d, store) = project();
    let i = investigation(&store);
    let archive_and_delete = |id: &str| {
        update(&store, id, |r| r.archived = true);
        let s = store.snapshot().unwrap();
        let e = s.get(id).unwrap();
        store
            .commit(
                vec![Change::Delete {
                    id: id.to_string(),
                    expected_revision: e.revision.clone(),
                }],
                None,
            )
            .unwrap();
    };
    let seen = store.snapshot().unwrap();
    let run = Change::create_seen(run(&i), &seen);
    let gap = seen.get(&i.x.id).unwrap().clone();
    archive_and_delete(&i.x.id);
    let err = conflict(&store, run);
    assert!(err.contains(&format!("{} does not exist", i.x.id)), "{err}");
    let err = conflict(
        &store,
        Change::Update {
            record: gap.record,
            expected_revision: gap.revision,
        },
    );
    assert!(err.contains("does not exist"), "{err}");
}

#[test]
fn incomplete_statements_are_clear_errors_not_conflicts() {
    let (_d, store) = project();
    let i = investigation(&store);
    let seen = store.snapshot().unwrap();
    let with = |record: Record, edit: &dyn Fn(&mut Expected)| {
        let Change::Create {
            record,
            expected: Some(mut expected),
        } = Change::create_seen(record, &seen)
        else {
            unreachable!()
        };
        edit(&mut expected);
        Change::Create {
            record,
            expected: Some(expected),
        }
    };
    let cases: Vec<(Change, &str)> = vec![
        (
            Change::Create {
                record: assess(&i.h, &i.e, &i.f),
                expected: None,
            },
            "requires `expected`",
        ),
        (
            with(assess(&i.h, &i.e, &i.f), &|e| e.hypotheses.clear()),
            "requires expected.hypotheses",
        ),
        (
            with(assess(&i.h, &i.e, &i.f), &|e| {
                e.hypotheses
                    .values_mut()
                    .for_each(|h| h.review_token.clear())
            }),
            "review_token is required",
        ),
        (
            with(assess(&i.h, &i.e, &i.f), &|e| {
                e.hypotheses
                    .values_mut()
                    .for_each(|h| h.review_token = "xyz".into())
            }),
            "64-hex-digit",
        ),
        (
            with(experiment(&i), &|e| e.revisions.clear()),
            "revision you read of experiment target",
        ),
        (
            with(run(&i), &|e| e.revisions.clear()),
            "revision you read of experiment",
        ),
        (
            with(run(&i), &|e| {
                let (id, revision) = e.revisions.pop_first().unwrap();
                e.revisions.insert(id[..12].to_string(), revision);
            }),
            "use the full ID",
        ),
        (
            Change::Update {
                record: seen.get(&i.h.id).unwrap().record.clone(),
                expected_revision: String::new(),
            },
            "expected_revision is required",
        ),
    ];
    for (change, message) in cases {
        let err = rejected(&store, change);
        assert!(err.contains(message), "expected {message:?} in {err}");
    }
    // Server-derived fields are output only.
    let mut stamped = assess(&i.h, &i.e, &i.f);
    if let Data::Assessment { based_on, .. } = &mut stamped.data {
        based_on.clone_from(&seen.hypotheses[&i.h.id].fingerprint);
    }
    let Change::Create { expected, .. } = Change::create_seen(assess(&i.h, &i.e, &i.f), &seen)
    else {
        unreachable!()
    };
    let err = rejected(
        &store,
        Change::Create {
            record: stamped,
            expected,
        },
    );
    assert!(err.contains("set by the server"), "{err}");
    let mut without_hypothesis = experiment(&i);
    if let Data::Experiment { targets, .. } = &mut without_hypothesis.data {
        targets[0].id.clone_from(&i.f.id);
    }
    let err = rejected(
        &store,
        Change::Create {
            record: without_hypothesis,
            expected: Some(Expected::default()),
        },
    );
    assert!(err.contains("must include its hypothesis"), "{err}");
}

/// The contract as an agent writes it for `hyp apply`, from `hyp --json show`.
#[test]
fn assessment_create_as_an_agent_writes_it() {
    let (_d, store) = project();
    let i = investigation(&store);
    let seen = store.snapshot().unwrap();
    let state = &seen.hypotheses[&i.h.id];
    let json = serde_json::json!({
        "op": "create",
        "record": {
            "kind": "assessment", "title": "Weakened by the replication",
            "body": "The timeout persists with the cache disabled.",
            "hypothesis": i.h.id, "judgment": "weakened", "evidence": [i.e.id]
        },
        "expected": {"hypotheses": {i.h.id.clone(): {"review_token": state.review_token}}}
    });
    let change: Change = serde_json::from_value(json).unwrap();
    let s = store.commit(vec![change], None).unwrap();
    assert!(!s.hypotheses[&i.h.id].needs_review);
    assert_eq!(s.hypotheses[&i.h.id].judgment, Judgment::Weakened);
}

#[test]
fn records_created_earlier_in_the_same_batch_need_no_stated_revision() {
    let (_d, store) = project();
    let i = investigation(&store);
    let seen = store.snapshot().unwrap();
    let fresh = evidence();
    let p2 = Record::new(
        "Timeouts drop below 1 in 10,000",
        Data::Prediction {
            hypothesis: i.h.id.clone(),
            conditions: String::new(),
        },
    );
    let mut x2 = experiment(&i);
    if let Data::Experiment { targets, .. } = &mut x2.data {
        targets[0].id.clone_from(&p2.id);
    }
    let s = store
        .commit(
            vec![
                Change::create_seen(fresh.clone(), &seen),
                Change::create_seen(link(&fresh, &i.h, Relation::Supports), &seen),
                Change::create_seen(p2, &seen),
                Change::create_seen(x2, &seen),
                Change::create_seen(assess(&i.h, &i.e, &i.f), &seen),
            ],
            None,
        )
        .unwrap();
    assert!(!s.hypotheses[&i.h.id].needs_review);
}

#[test]
fn experiment_and_run_are_conflicts_when_what_they_freeze_changed_after_it_was_read() {
    let (_d, store) = project();
    let i = investigation(&store);
    let seen = store.snapshot().unwrap();
    let x2 = Change::create_seen(experiment(&i), &seen);
    let r = Change::create_seen(run(&i), &seen);
    update(&store, &i.f.id, |r| {
        r.title = "Reject if any timeout".into()
    });
    let err = conflict(&store, x2);
    assert!(err.contains(&format!("{} changed", i.f.id)), "{err}");
    // Its hypothesis is frozen too, so a changed claim is also a conflict.
    let seen = store.snapshot().unwrap();
    let x2 = Change::create_seen(experiment(&i), &seen);
    update(&store, &i.h.id, |r| r.title = "Narrowed claim".into());
    let err = conflict(&store, x2);
    assert!(err.contains(&format!("{} changed", i.h.id)), "{err}");
    update(&store, &i.x.id, |r| r.body = "Revised procedure".into());
    let err = conflict(&store, r);
    assert!(err.contains(&format!("{} changed", i.x.id)), "{err}");
    // What gets frozen is exactly what the author read.
    let seen = store.snapshot().unwrap();
    let x = store
        .commit(vec![Change::create_seen(experiment(&i), &seen)], None)
        .unwrap();
    let frozen = x
        .objects
        .iter()
        .find_map(|e| match &e.record.data {
            Data::Experiment { targets, .. } if e.record.title == "Replicate" => {
                Some(targets.clone())
            }
            _ => None,
        })
        .unwrap();
    assert_eq!(frozen[0].revision, seen.get(&i.h.id).unwrap().revision);
    assert!(frozen[0].body.contains("Narrowed claim"));
    assert_eq!(frozen[1].revision, seen.get(&i.f.id).unwrap().revision);
}

#[test]
fn stale_archive_and_delete_are_still_conflicts() {
    let (_d, store) = project();
    let i = investigation(&store);
    let gap = Record::new(
        "Does timing change?",
        Data::Gap {
            hypothesis: i.h.id.clone(),
            resolved: false,
        },
    );
    create(&store, &gap);
    let stale = store
        .snapshot()
        .unwrap()
        .find(&gap.id)
        .unwrap()
        .revision
        .clone();
    update(&store, &gap.id, |r| r.archived = true);
    let err = conflict(
        &store,
        Change::Archive {
            id: gap.id.clone(),
            archived: false,
            expected_revision: stale.clone(),
        },
    );
    assert!(err.contains("object changed"), "{err}");
    let err = conflict(
        &store,
        Change::Delete {
            id: gap.id.clone(),
            expected_revision: stale,
        },
    );
    assert!(err.contains("object changed"), "{err}");
}

/// Evidence cited by the hypothesis' runs is part of the basis, but other
/// hypotheses' interpretations of it are not (decision-0003: another
/// hypothesis counts only through a link to this one).
#[test]
fn run_evidence_counts_but_its_interpretations_elsewhere_do_not() {
    let (_d, store) = project();
    let i = investigation(&store);
    let observed = evidence();
    create(&store, &observed);
    let mut r = run(&i);
    if let Data::Run { evidence, .. } = &mut r.data {
        evidence.push(observed.id.clone());
    }
    create(&store, &r);
    let other = hypothesis();
    create(&store, &other);
    create(&store, &assess(&i.h, &i.e, &i.f));
    let needs_review = || store.snapshot().unwrap().hypotheses[&i.h.id].needs_review;
    assert!(!needs_review());
    create(&store, &link(&observed, &other, Relation::Supports));
    update(&store, &other.id, |r| r.title = "Changed far end".into());
    assert!(!needs_review());
    update(&store, &observed.id, |r| {
        r.body = "Corrected reading".into()
    });
    assert!(needs_review());
}

/// A run depends on the evidence it cites as its author read it (review
/// round 2, finding 2).
#[test]
fn run_is_a_conflict_when_cited_evidence_changed_after_it_was_read() {
    let (_d, store) = project();
    let i = investigation(&store);
    let observed = evidence();
    create(&store, &observed);
    let citing = || {
        let mut r = run(&i);
        if let Data::Run { evidence, .. } = &mut r.data {
            evidence.push(observed.id.clone());
        }
        r
    };
    let stale = Change::create_seen(citing(), &store.snapshot().unwrap());
    update(&store, &observed.id, |r| {
        r.body = "Corrected reading".into()
    });
    let err = conflict(&store, stale);
    assert!(err.contains(&format!("{} changed", observed.id)), "{err}");
    // Not stated at all: a statement to add, not a race.
    let Change::Create {
        record,
        expected: Some(mut expected),
    } = Change::create_seen(citing(), &store.snapshot().unwrap())
    else {
        unreachable!()
    };
    expected.revisions.remove(&observed.id);
    let err = rejected(
        &store,
        Change::Create {
            record,
            expected: Some(expected),
        },
    );
    assert!(err.contains("revision you read of cited evidence"), "{err}");
    // Deleted after the read.
    let stale = Change::create_seen(citing(), &store.snapshot().unwrap());
    update(&store, &observed.id, |r| r.archived = true);
    let s = store.snapshot().unwrap();
    store
        .commit(
            vec![Change::Delete {
                id: observed.id.clone(),
                expected_revision: s.get(&observed.id).unwrap().revision.clone(),
            }],
            None,
        )
        .unwrap();
    let err = conflict(&store, stale);
    assert!(
        err.contains(&format!("{} does not exist", observed.id)),
        "{err}"
    );
    create(&store, &run(&i));
}

/// Assesses `i.h` from a fresh read, citing its linked evidence.
fn reassess(store: &Store, i: &Investigation) {
    create(store, &assess(&i.h, &i.e, &i.f));
    assert!(!store.snapshot().unwrap().hypotheses[&i.h.id].needs_review);
}
/// Decision-0003 item 1: only content counts. Each change here left the
/// hypothesis needing review before the fingerprint became a projection.
#[test]
fn changes_outside_the_basis_do_not_flag_an_assessment() {
    let (_d, store) = project();
    let i = investigation(&store);
    let other = hypothesis();
    let gap = Record::new(
        "Does timing change?",
        Data::Gap {
            hypothesis: i.h.id.clone(),
            resolved: false,
        },
    );
    for r in [
        &other,
        &link(&i.h, &other, Relation::CompetesWith),
        &link(&i.e, &other, Relation::Qualifies),
        &gap,
    ] {
        create(&store, r);
    }
    let mut judged = assess(&other, &i.e, &i.f);
    if let Data::Assessment {
        judgment,
        criterion,
        ..
    } = &mut judged.data
    {
        *judgment = Judgment::Weakened;
        *criterion = None;
    }
    create(&store, &judged);
    reassess(&store, &i);
    let unflagged = |what: &str, id: &str, edit: &dyn Fn(&mut Record)| {
        update(&store, id, edit);
        let s = store.snapshot().unwrap();
        assert!(
            !s.hypotheses[&i.h.id].needs_review,
            "{what} flagged the hypothesis"
        );
        assert!(
            !s.hypotheses[&other.id].needs_review,
            "{what} flagged the competing hypothesis"
        );
    };
    unflagged("close the hypothesis", &i.h.id, &|r| {
        set_lifecycle(r, Lifecycle::Closed)
    });
    unflagged("retag the hypothesis", &i.h.id, &|r| {
        r.tags = vec!["dma".into()]
    });
    unflagged("give an untestable reason", &i.h.id, &|r| {
        if let Data::Hypothesis {
            untestable_reason, ..
        } = &mut r.data
        {
            *untestable_reason = "Also hard to test".into();
        }
    });
    unflagged("start the experiment", &i.x.id, &|r| {
        if let Data::Experiment { status, .. } = &mut r.data {
            *status = ExperimentStatus::Running;
        }
    });
    unflagged("revise the experiment's procedure", &i.x.id, &|r| {
        r.body = "Revised".into()
    });
    unflagged("resolve the gap", &gap.id, &|r| {
        if let Data::Gap { resolved, .. } = &mut r.data {
            *resolved = true;
        }
    });
    unflagged("fix a typo in the gap", &gap.id, &|r| {
        r.title = "Does timing change at all?".into()
    });
    unflagged("retag the evidence", &i.e.id, &|r| {
        r.tags = vec!["replicated".into()]
    });
    unflagged("close the competing hypothesis", &other.id, &|r| {
        set_lifecycle(r, Lifecycle::Closed)
    });
    // Renaming it changes its own claim, which flags only its own assessment.
    update(&store, &other.id, |r| r.title = "Bus contention".into());
    let s = store.snapshot().unwrap();
    assert!(
        !s.hypotheses[&i.h.id].needs_review,
        "renaming the competing hypothesis"
    );
    assert!(s.hypotheses[&other.id].needs_review);
}
/// Decision-0003 item 1: content still counts. Each change flags the
/// current assessment; re-assessing clears it for the next.
#[test]
fn changes_to_the_basis_flag_an_assessment() {
    let (_d, store) = project();
    let i = investigation(&store);
    let criterion_link = link(&i.e, &i.f, Relation::Supports);
    create(&store, &criterion_link);
    let p = store
        .snapshot()
        .unwrap()
        .objects
        .iter()
        .find(|e| matches!(e.record.data, Data::Prediction { .. }))
        .unwrap()
        .record
        .id
        .clone();
    let evidence_link = store
        .snapshot()
        .unwrap()
        .objects
        .iter()
        .find(|e| matches!(&e.record.data, Data::Link { to, .. } if *to == i.h.id))
        .unwrap()
        .record
        .id
        .clone();
    reassess(&store, &i);
    let flags = |what: &str, change: &dyn Fn()| {
        change();
        assert!(
            store.snapshot().unwrap().hypotheses[&i.h.id].needs_review,
            "{what} went unnoticed"
        );
        reassess(&store, &i);
    };
    flags("editing the claim", &|| {
        update(&store, &i.h.id, |r| r.title = "Narrowed claim".into())
    });
    flags("editing the scope", &|| {
        update(&store, &i.h.id, |r| {
            if let Data::Hypothesis { scope, .. } = &mut r.data {
                *scope = "rev D".into();
            }
        })
    });
    flags("editing a criterion's title", &|| {
        update(&store, &i.f.id, |r| {
            r.title = "Reject if any timeout".into()
        })
    });
    flags("editing a prediction's conditions", &|| {
        update(&store, &p, |r| {
            if let Data::Prediction { conditions, .. } = &mut r.data {
                *conditions = "5 transfers".into();
            }
        })
    });
    flags("correcting the evidence source", &|| {
        update(&store, &i.e.id, |r| {
            if let Data::Evidence { source, .. } = &mut r.data {
                *source = "logs/run143.txt".into();
            }
        })
    });
    flags("attaching the raw capture", &|| {
        let capture = store.root.join("capture.txt");
        std::fs::write(&capture, "timeout at 8,142").unwrap();
        store.attach(&i.e.id, &capture).unwrap();
    });
    flags("editing the observation", &|| {
        update(&store, &i.e.id, |r| r.body = "Wrong board".into())
    });
    flags("editing a link's reason", &|| {
        update(&store, &evidence_link, |r| {
            r.body = "On second thought".into()
        })
    });
    flags("adding contradicting evidence", &|| {
        let counter = evidence();
        create(&store, &counter);
        create(&store, &link(&counter, &i.h, Relation::Contradicts));
    });
    flags("a new run outcome", &|| create(&store, &run(&i)));
    flags("archiving a link", &|| {
        update(&store, &criterion_link.id, |r| r.archived = true)
    });
    flags("adding a criterion", &|| {
        create(
            &store,
            &Record::new(
                "Reject if slower",
                Data::Criterion {
                    hypothesis: i.h.id.clone(),
                },
            ),
        )
    });
    flags("archiving a criterion", &|| {
        update(&store, &i.f.id, |r| r.archived = true)
    });
}
#[test]
fn an_update_that_changes_nothing_is_not_written() {
    let (_d, store) = project();
    let h = hypothesis();
    create(&store, &h);
    let path = store.root.join(format!("hyp/hypotheses/{}.md", h.id));
    let before = std::fs::read_to_string(&path).unwrap();
    update(&store, &h.id, |_| {});
    update(&store, &h.id, |r| {
        r.updated_at = chrono::Utc::now().to_rfc3339()
    });
    assert_eq!(std::fs::read_to_string(&path).unwrap(), before);
    update(&store, &h.id, |r| r.tags = vec!["x".into()]);
    assert_ne!(std::fs::read_to_string(&path).unwrap(), before);
}
fn set_lifecycle(r: &mut Record, to: Lifecycle) {
    if let Data::Hypothesis { lifecycle, .. } = &mut r.data {
        *lifecycle = to;
    }
}
/// Codex P2: archiving the assessed hypothesis itself is a change to review.
#[test]
fn archiving_the_assessed_hypothesis_needs_review() {
    let (_d, store) = project();
    let i = investigation(&store);
    reassess(&store, &i);
    let stale = Change::create_seen(assess(&i.h, &i.e, &i.f), &store.snapshot().unwrap());
    update(&store, &i.h.id, |r| r.archived = true);
    assert!(store.snapshot().unwrap().hypotheses[&i.h.id].needs_review);
    let err = conflict(&store, stale);
    assert!(err.contains("review_token"), "{err}");
}
/// Evidence reached only through an archived link, or linked only to an
/// archived criterion, is neither in the basis nor citable. The archived link
/// and criterion themselves stay in the basis, so archiving them flags.
#[test]
fn evidence_reached_only_through_archived_records_is_not_in_the_basis() {
    let (_d, store) = project();
    let i = investigation(&store);
    let via_link = evidence();
    let via_criterion = evidence();
    let f2 = Record::new(
        "Reject if slower",
        Data::Criterion {
            hypothesis: i.h.id.clone(),
        },
    );
    let archived_link = link(&via_link, &i.h, Relation::Supports);
    for r in [
        &via_link,
        &via_criterion,
        &f2,
        &archived_link,
        &link(&via_criterion, &f2, Relation::Supports),
    ] {
        create(&store, r);
    }
    update(&store, &archived_link.id, |r| r.archived = true);
    update(&store, &f2.id, |r| r.archived = true);
    let s = store.snapshot().unwrap();
    let basis = s.basis(&i.h.id);
    assert!(basis.contains_key(&archived_link.id) && basis.contains_key(&f2.id));
    for e in [&via_link, &via_criterion] {
        assert!(!basis.contains_key(&e.id), "{} in the basis", e.title);
        let err = rejected(&store, Change::create_seen(assess(&i.h, e, &i.f), &s));
        assert!(err.contains("not linked"), "{err}");
    }
}
/// Architect M1: within one batch the basis may grow only by records the
/// batch creates. A new link to evidence that existed before brings in
/// content the author's review token never covered.
#[test]
fn a_batch_may_not_bring_unreviewed_records_into_the_basis() {
    let (_d, store) = project();
    let i = investigation(&store);
    let other = evidence();
    create(&store, &other);
    let seen = store.snapshot().unwrap();
    update(&store, &other.id, |r| {
        r.title = "Edited by someone else".into()
    });
    let before = store.snapshot().unwrap().revision;
    let err = store
        .commit(
            vec![
                Change::create_seen(link(&other, &i.h, Relation::Supports), &seen),
                Change::create_seen(assess(&i.h, &i.e, &i.f), &seen),
            ],
            None,
        )
        .unwrap_err();
    assert_eq!(hyp::cli::exit_code(&err), 1, "{err:#}");
    let err = format!("{err:#}");
    assert!(
        err.contains(&other.id) && err.contains("earlier write"),
        "{err}"
    );
    assert_eq!(store.snapshot().unwrap().revision, before);
    // Observe, link and assess in one batch: all of it is the author's own.
    let seen = store.snapshot().unwrap();
    let fresh = evidence();
    let s = store
        .commit(
            vec![
                Change::create_seen(fresh.clone(), &seen),
                Change::create_seen(link(&fresh, &i.h, Relation::Supports), &seen),
                Change::create_seen(assess(&i.h, &fresh, &i.f), &seen),
            ],
            None,
        )
        .unwrap();
    assert!(!s.hypotheses[&i.h.id].needs_review);
}
/// Architect M2: `based_on` values are stored, so the canonical form of the
/// basis is a contract. A serde_json `preserve_order` flip, a renamed field
/// or enum value, or a new field changes every fingerprint; this makes that
/// loud.
#[test]
fn the_fingerprint_canonical_form_is_pinned() {
    let entry = |id: &str, title: &str, data: Data| Entry {
        record: Record {
            id: id.into(),
            title: title.into(),
            body: format!("{title}."),
            tags: vec!["tag".into()],
            archived: false,
            created_at: "2026-01-01T00:00:00+00:00".into(),
            updated_at: "2026-01-02T00:00:00+00:00".into(),
            data,
        },
        revision: "0".repeat(64),
    };
    let id = |prefix: &str, n: u8| format!("{prefix}-00000000-0000-4000-8000-0000000000{n:02}");
    let (h, f, p, e, l, x, r) = (
        id("H", 1),
        id("F", 2),
        id("P", 3),
        id("E", 4),
        id("L", 5),
        id("X", 6),
        id("R", 7),
    );
    let s = Snapshot {
        objects: vec![
            entry(
                &h,
                "Claim",
                Data::Hypothesis {
                    scope: "Scope".into(),
                    assumptions: "None".into(),
                    lifecycle: Lifecycle::Investigating,
                    untestable_reason: String::new(),
                },
            ),
            entry(
                &f,
                "Criterion",
                Data::Criterion {
                    hypothesis: h.clone(),
                },
            ),
            entry(
                &p,
                "Prediction",
                Data::Prediction {
                    hypothesis: h.clone(),
                    conditions: "Conditions".into(),
                },
            ),
            entry(
                &e,
                "Observation",
                Data::Evidence {
                    source: "log.txt".into(),
                    locator: "line 1".into(),
                    observed_at: "2026-01-01".into(),
                    attachments: vec![Attachment {
                        path: format!("assets/{}", "a".repeat(64)),
                        sha256: "a".repeat(64),
                    }],
                },
            ),
            entry(
                &l,
                "Interpretation",
                Data::Link {
                    from: e.clone(),
                    to: h.clone(),
                    relation: Relation::Contradicts,
                },
            ),
            entry(
                &x,
                "Experiment",
                Data::Experiment {
                    hypothesis: h.clone(),
                    targets: vec![],
                    status: ExperimentStatus::Completed,
                },
            ),
            entry(
                &r,
                "Run",
                Data::Run {
                    experiment: x.clone(),
                    plan: FrozenRef::default(),
                    outcome: Outcome::Observed,
                    evidence: vec![e.clone()],
                },
            ),
        ],
        ..Snapshot::default()
    };
    assert_eq!(
        serde_json::to_string(&s.basis(&h)).unwrap(),
        r#"{"E-00000000-0000-4000-8000-000000000004":{"archived":false,"attachments":["aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"],"body":"Observation.","locator":"line 1","source":"log.txt","title":"Observation"},"F-00000000-0000-4000-8000-000000000002":{"archived":false,"body":"Criterion.","title":"Criterion"},"H-00000000-0000-4000-8000-000000000001":{"archived":false,"assumptions":"None","body":"Claim.","scope":"Scope","title":"Claim"},"L-00000000-0000-4000-8000-000000000005":{"archived":false,"body":"Interpretation.","from":"E-00000000-0000-4000-8000-000000000004","relation":"contradicts","to":"H-00000000-0000-4000-8000-000000000001"},"P-00000000-0000-4000-8000-000000000003":{"archived":false,"body":"Prediction.","conditions":"Conditions","title":"Prediction"},"R-00000000-0000-4000-8000-000000000007":{"body":"Run.","evidence":["E-00000000-0000-4000-8000-000000000004"],"outcome":"observed","title":"Run"}}"#
    );
    assert_eq!(
        s.fingerprint(&h),
        "6bff2cf94d53b7f9235ed7173e93b7c0011efeaa04429140bb9c3d8a548638ac"
    );
}
/// Frozen history stripped of its content on disk is reported, not loaded
/// silently with empty defaults.
#[test]
fn stripped_frozen_content_is_a_diagnostic() {
    let (_d, store) = project();
    let i = investigation(&store);
    let r = run(&i);
    create(&store, &r);
    assert!(store.snapshot().unwrap().diagnostics.is_empty());
    let strip = |dir: &str, id: &str, edit: &dyn Fn(&mut Data)| {
        let path = store.root.join(format!("hyp/{dir}/{id}.md"));
        let mut record = decode(&std::fs::read_to_string(&path).unwrap()).unwrap();
        edit(&mut record.data);
        std::fs::write(&path, encode(&record).unwrap()).unwrap();
    };
    strip("experiments", &i.x.id, &|d| {
        if let Data::Experiment { targets, .. } = d {
            targets[0].title.clear();
        }
    });
    strip("runs", &r.id, &|d| {
        if let Data::Run { plan, .. } = d {
            plan.body.clear();
        }
    });
    let s = store.snapshot().unwrap();
    let messages: Vec<_> = s.diagnostics.iter().map(|d| d.message.as_str()).collect();
    assert!(
        messages
            .iter()
            .any(|m| m.contains("missing its frozen revision, title or body")),
        "{messages:?}"
    );
    assert!(
        messages
            .iter()
            .any(|m| m.contains("invalid frozen experiment plan")),
        "{messages:?}"
    );
    assert!(s.assert_writable().is_err());
}
