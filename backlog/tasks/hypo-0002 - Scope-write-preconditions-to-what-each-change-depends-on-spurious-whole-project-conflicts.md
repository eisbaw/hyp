---
id: HYPO-0002
title: >-
  Scope write preconditions to what each change depends on (spurious
  whole-project conflicts)
status: Done
assignee:
  - '@claude'
created_date: '2026-09-29 22:15'
updated_date: '2026-09-30 01:19'
labels:
  - bug
  - webui
  - concurrency
  - cli
  - mvp
dependencies: []
priority: high
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Found in the initial review; the fix was corrected by the architect review.

Symptom (reproduced): the WebUI sends `expected_revision: draftRevision`, the whole-project revision from when the editor opened. Any unrelated write (an agent adding another hypothesis via the CLI) makes the save fail with 409 `conflict: project changed`. Most CLI commands also commit with `Some(&s.revision)` in `cli.rs`, so agents get a spurious exit 3 when an unrelated write lands between their read and their commit. Archive/restore/delete in the WebUI (`transact()` default) have the same problem.

Root cause: the precondition is too coarse. Dropping it would be wrong: `Store::commit` computes an assessment's `based_on` fingerprint on the server at commit time, and freezes experiment targets and run plans at commit time too. Without a precondition, a human writing an assessment while an agent adds counter-evidence would record an assessment "based on" evidence nobody saw, with `needs_review` false. That is the exact failure hyp exists to prevent.

Fix: each change carries the precondition it actually depends on:
- update/archive/delete: the object's revision (already present);
- assessment create: the hypothesis fingerprint the author saw; the server rejects if it differs;
- experiment create: the target revisions the author saw (the client already sends `FrozenRef.revision`; the server currently ignores it and re-freezes);
- run create: the experiment revision the author saw.
The whole-project `expected_revision` stays available (`hyp apply --expected-revision`) but is optional and not used for ordinary edits.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 WebUI saves and CLI commands succeed when only unrelated records changed since they read the project
- [x] #2 An assessment is rejected as a conflict when the hypothesis fingerprint changed after the author read it (e.g. new counter-evidence), and the WebUI draft is preserved
- [x] #3 Experiment/run creation is rejected when the frozen targets/plan changed after the author read them
- [x] #4 Updates, archives and deletes of a record that changed are still rejected as conflicts
- [x] #5 Tests cover each case above, including an unrelated concurrent CLI write during a WebUI save
- [x] #6 The WebUI decides 'conflict' from the HTTP status (409), not from the message prefix "conflict:" (web/app.js save() currently uses e.message.startsWith)
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Store: per-create preconditions checked against the state before the batch (the snapshot the caller read): assessment based_on == hypothesis fingerprint, supersedes == current heads, cited evidence revisions via a new optional Create field expected_revisions; experiment targets (hypothesis required) and run plan revisions. Empty/missing precondition = ordinary error, stale = Conflict. Server still stores computed based_on/supersedes and freezes content from the post-batch state.
2. Change::create_seen(record, &snapshot) states the preconditions for Rust callers (CLI, seed_demo, tests).
3. CLI commits with None; /api/transaction expected_revision optional.
4. WebUI: editor keeps the snapshot it opened on and derives all preconditions from it; no project revision; conflict = HTTP 409.
5. Tests: workflow (store), api, cli (hyp edit with an EDITOR script doing an unrelated write), dom-test. Red/green by reverting pieces.
6. README contract; forward-carry to HYPO-0016.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Forward-carried from HYPO-0007: conflicts are now a type, store::Conflict(String). Raise new preconditions (hypothesis fingerprint, frozen target and plan revisions) with bail!(Conflict("...".into())) and do not write the 'conflict: ' prefix in the message (Display adds it). CLI exit 3 (cli::exit_code) and HTTP 409 (web::ApiError) then follow automatically, even through .context(). web/app.js still checks e.message.startsWith('conflict:') to show the preserve-your-draft hint, so do not wrap conflicts in context on the API path. Test templates: tests/cli.rs stale_cli_write_exits_with_code_3_and_a_conflict_message (real binary, 'apply --expected-revision'), tests/api.rs api_and_cli_store_share_state_and_reject_stale_writes (409 plus 'conflict:' body).

Implementation (HYPO-0002, uncommitted, for review):

Design as implemented
- All preconditions refer to the project before the batch (the snapshot the caller read), not to the state including earlier changes of the batch. Records created earlier in the same batch are the caller's own and need no stated revision. Derived/frozen content (based_on, supersedes, frozen targets and plan) is still computed from the post-batch state, as before.
- Missing statement = ordinary error (exit 1 / HTTP 422): retrying cannot help. Stale statement = store::Conflict (exit 3 / HTTP 409).
- Assessment create: based_on must equal the hypothesis fingerprint in the read snapshot (hypotheses[H].fingerprint); supersedes must equal its assessment heads (hypotheses[H].assessment_ids), set-compared; every cited evidence record that existed before the batch must appear in a new optional Change::Create field expected_revisions {id: revision}, which is checked generically (any create may state revisions of records it depends on).
- Experiment create: targets must include the hypothesis (the server used to add it silently, without any precondition on the claim text it froze); each existing target's revision must match.
- Run create: plan.revision must equal the experiment's revision.
- Change::create_seen(record, &snapshot) states all of this for Rust callers (CLI commands, seed_demo, tests). web/app.js stateSeen() mirrors it from the snapshot the editor opened on (editing.seen). CLI commits with None; /api/transaction expected_revision is Option (still honoured); WebUI no longer sends it and classifies conflicts by status 409.

Deviation from the orchestrator's design, and why
- The design said: compare the client's based_on to the fingerprint recomputed "on the basis that includes earlier changes in the same batch" (today's basis also includes the new assessment itself). Concrete flaw: that basis fingerprint includes the assessment's own cited evidence (and links from it). hypotheses[H].fingerprint, which the WebUI and agents can read, does not. So an assessment citing evidence not already linked to H (a documented feature, see unlinked_assessment_evidence_changes_trigger_review; the WebUI evidence picker lists all evidence) would conflict on every attempt, and a batch creating evidence + assessment could never state a matching value (revisions depend on server timestamps). Fix: compare against the fingerprint of the read state, and cover the cited evidence separately with expected_revisions.
- supersedes as a precondition: needed. Assessments are excluded from the fingerprint, so a concurrent assessment citing only already-relevant evidence leaves the fingerprint unchanged; without the heads check the new assessment would silently supersede a judgment its author never saw (test asserts the fingerprint is unchanged in that case). Conflict chosen over storing the stated heads (which would create two heads): the author should see the other judgment first.

Gotchas
- A batch's assessment fingerprints the batch state at its position: records created later in the same batch (e.g. an experiment after the assessment) flag it needs_review immediately. Pre-existing behaviour; put assessments last.
- serde treats Option fields as defaulted even without #[serde(default)]; kept it for explicitness.
- WebUI: while a dirty form is open, refresh() does not update the global snapshot; after cancelling, a new form opened before closeEditor()'s refresh completes still uses the old snapshot and gets a (safe, conservative) conflict. The DOM test waits for the refresh.
- CLI test determinism: hyp edit runs $VISUAL between its read and commit, so an editor script that runs another hyp write makes the race deterministic (tests/cli.rs edit_during_concurrent_write).

Remaining precondition gap (limit, not fixed): citing evidence also pulls links from that evidence and their endpoints (e.g. another hypothesis H2) into the stored fingerprint. A concurrent change to those, between read and commit, is absorbed without a conflict. They concern other hypotheses; noted on HYPO-0009, which owns the fingerprint scope.

Red/green (each check disabled in turn, then restored): CLI commit with Some(&s.revision) -> cli unrelated_write test fails (exit 3); no fingerprint check -> workflow stale-hypothesis test and DOM 'stale assessment' step fail; no heads check -> another_assessment test fails; no expected_revisions check -> cited_unlinked test fails; cited evidence may go unstated -> without_stated test fails; no target / no plan check / hypothesis target optional -> experiment_and_run test fails; empty based_on accepted -> workflow and cli apply tests fail (exit 3 instead of 1); /api/transaction expected_revision back to required String -> api test fails; WebUI prefix-based conflict -> DOM 409 stub step fails; WebUI sending the open-time project revision -> DOM unrelated-write step fails. Gate: fmt-check 0, lint 0, e2e 0 twice (api 5, cli 7, workflow 30, DOM PASS; DOM also 5 extra runs), nix flake check -L 0.

Cycle 2 (after the deep gate: Codex NO-GO, architect conditional). Supersedes the contract described above (expected_revisions, client-set based_on/supersedes, target/plan revisions in the record are gone).

Final contract
- Change::Create { record, expected: Option<Expected> }. Expected { hypotheses: {H: {fingerprint, assessment_ids: Option<Vec>}}, revisions: {full ID: revision} }. Required (Some) for assessment, experiment and run creates; optional and still checked for other kinds.
- based_on, supersedes, FrozenRef revision/title/body and a run's plan are output-only: a create that fills them in is rejected (exit 1, "set by the server"). FrozenRef fields other than id and Run.plan now deserialize with defaults so clients send {"id": ...} and omit plan; on-disk serialization is unchanged (a file missing plan now fails validation instead of parsing; both are error diagnostics).
- Closure rule (review P1/P2): Snapshot::relevant(H) is the fingerprint's record set (fingerprint = fingerprint_of(relevant)); Snapshot::relevant_with(assessment) adds the new assessment. store::assessment_additions(read, current, record) = current.relevant_with(record) minus read.relevant(H), limited to IDs existing in read. The server requires every such ID (read = before, current = after) in expected.revisions; unstated, changed or vanished ones are one Conflict listing the IDs. create_seen uses the same function on the read snapshot; web/app.js mirrors it minimally (cited evidence, links touching it, their other ends; stating already-covered records is harmless) and relies on the server listing what it missed.
- Error classes: stated record gone -> Conflict (also update/archive/delete of a deleted record); empty expected_revision, missing expected / hypotheses entry / fingerprint / assessment_ids, missing target or experiment revision, prefix keys ("use the full ID") -> ordinary error. Statement-shape errors are checked before kind-specific ones.
- hyp assess requires --based-on <fingerprint> (clap: missing flag exits 2); mismatch exits 3. Heads and addition revisions come from the command's read. Other CLI commands protect only their own read-to-write window; README says so.
- Batch semantics tested (review P3/P5): [update cited unlinked E, assess citing E] succeeds; [assess H, assess H] succeeds and chains (second supersedes first).

Gotcha: a batch that links existing evidence into H and then assesses H must state that evidence in the assessment's expected.revisions; create_seen does not know the batch, so Rust/JSON callers add it (the conflict names it). Test: batch_linking_existing_evidence_must_state_what_it_brings_in.

Red/green cycle 2 (each guard disabled, then restored): statements checked against after -> own_earlier_changes test; additions covered from after -> batch_linking test; unstated additions ignored -> P2 and batch tests and DOM "unseen link" step; deleted update target / vanished statement not a conflict -> stated_records_deleted test; empty expected_revision, prefix keys, output-only based_on, absent assessment_ids, target statement optional, hypothesis target optional -> incomplete_statements test; fingerprint / heads / revision compare removed -> the respective conflict tests; hyp assess ignoring --based-on -> cli assess test (exit 0 instead of 3); WebUI stating only cited evidence -> DOM shared-evidence save.
Gate cycle 2: fmt-check 0, lint 0, e2e 0 twice (api 5, cli 8, workflow 35, DOM PASS; plus 5 DOM reruns), nix flake check -L 0.

Cycle 3 (round-2 gate: QA GO, architect GO, Codex NO-GO).
- Fingerprint fix (Codex High, predates HYPO-0002): Snapshot::relevant now adds the evidence cited by the hypothesis's runs to the direct set before expanding links, so links touching run evidence and their far ends are covered. assessment_additions and web/app.js are unaffected in shape (additions still come only from the new assessment's cited evidence). This changes fingerprints of existing notebooks whose runs cite linked evidence: some assessments flip to needs-review once.
- Run creates state the revision of each cited evidence in expected.revisions (missing -> exit 1; changed/deleted -> Conflict). create_seen, the WebUI and hyp run (own read) fill them.
- validate requires non-empty title and body (besides the revision) for experiment targets and a run's plan, so stripped frozen history is a diagnostic despite the input serde defaults.
- CLI review token: HypothesisState.review_token = sha256(fingerprint + "\n" + sorted assessment IDs), in hyp show/list --json and the snapshot. `hyp assess --reviewed <token>` replaces --based-on; it compares the token with its own read (mismatch -> Conflict, exit 3), then states that read in expected as before. Malformed token (not 64 hex) -> exit 1. The store contract (expected.hypotheses) is unchanged. hyp --json list rows now carry "state" for hypotheses (additive).
- Wording: "creating a run"; "does not exist (deleted since you read it, or never existed)".
- README: recipe for assessment statements (evidence, links in .related, both ends; covered records harmless), batch gotcha, --reviewed.
Red/green cycle 3: run-evidence requirement removed -> run_is_a_conflict... test; create_seen omitting run evidence -> links_on_run_evidence and run tests; old relevant order -> links_on_run_evidence test (red before the fix too); target title/body and plan body validation removed -> stripped_frozen_content test; token over fingerprint only -> cli assess test (exit 0 instead of 3 after another agent's assessment); token format unchecked -> cli assess test (exit 3 instead of 1); WebUI omitting run evidence -> DOM run step.
Gate cycle 3: fmt-check 0, lint 0, e2e 0 twice (api 5, cli 8, workflow 38, DOM PASS), nix flake check -L 0.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Per-change preconditions replace the whole-project revision; see the commit message for the final contract. Deep gate over three rounds: QA GO (round 3 conditional on a clean rebuild; re-run by the orchestrator after cargo clean: api 5, cli 8, workflow 38, DOM PASS), architect GO (probes P1-P11 pass). Codex round 3 remained NO-GO: the orchestrator triaged its findings as fingerprint-scope questions that predate this change (moved to HYPO-0009, raised to high), the documented CLI read window (HYPO-0025), and hand-edited assessments (HYPO-0026). None is a regression introduced here.
<!-- SECTION:FINAL_SUMMARY:END -->
