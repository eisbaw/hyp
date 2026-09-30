---
id: HYPO-0054
title: CLI ergonomics follow-ups
status: To Do
assignee: []
created_date: '2026-09-30 12:18'
updated_date: '2026-09-30 16:36'
labels:
  - cli
  - ux
  - hardening
dependencies: []
priority: low
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Non-blocking findings from the CLI ergonomics review (2026-09-30): Status::parse would silently prefer Judgment if value sets ever overlap (add a disjointness test); Kind is the single source only for kind(), while directory()/prefix() and JSON show still match on strings (kind() -> Kind plus as_str()); a list row shows 'untested' for conflicting assessment heads without saying why; serde_json::to_value(...).unwrap_or_default() in show.rs swallows errors (use expect); hints still say 'hyp --json show' for the review token though plain show prints it (a deliberate message change); hyp list --kind experiment has no status column; list rows use one space before the title after needs-review; 'Referenced by' in plain show uses auto link titles and lists superseded assessments; plain show of an assessment prints the based_on hash; no-op writes still appear in written (consider a changed flag).
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Kind value sets tested disjoint; kind() returns Kind
- [ ] #2 Experiment rows show status; list rows explain conflicting heads
<!-- AC:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Interactive terminal test-drive 2026-09-30 (xterm+tmux, first-time user, release build of fda0faa). More: experiment status never advances and does not constrain runs (two runs on a 'planned' experiment; runs accepted on a 'completed' one without warning); list shows no experiment status. Plain show E- does not show which hypotheses the evidence supports/contradicts (only link IDs titled 'Evidence for ...'); fields alphabetical (locator before source); mixed key styles (observed_at vs created); show R- repeats plan: next to experiment:. Full 38-char IDs make list rows wrap at 120 columns and unreadable at 60 (fixed columns ~70 chars): drop the kind column when listing one kind and consider short IDs in human output. Closing a hypothesis does not warn about open gaps/experiments.

From the human CLI batch 1 review: 'hyp set --title -' is not idempotent (re-running appends the stdin rest to the body again, so agent retries duplicate text); 'evidence attach' on a non-evidence record uses old wording instead of the 'argument ...: expected evidence, got ...' form; evidence add names its argument <HYPOTHESIS> although it accepts H-/P-/F- (rename to <TARGET>); the help test's about.contains('\n') check is a no-op and it does not assert the delete op; link titles store the typed prefix instead of the full ID, and pre-change links keep 'depends_on' in titles.

From the human CLI batch 2 review: an agent using --json cannot detect a no-op write without comparing revisions (consider exposing Written.changed in JSON); summary() pairs actions with written records by position, so extra records from attach would silently drop; nothing keeps the list of stdin-capable text arguments in sync when a new one is added; --confidence -0.5 without '=' is parsed by clap as a flag (misleading tip); the stdin hint names positional args TITLE but flags --body; kept edit copies live in $TMPDIR (mode 0600, may be wiped at reboot) — document.

From the batch-2 confirmation (QA): an empty VISUAL (set but empty) counts as set, so EDITOR is ignored and sh -c ' "$1"' tries to execute the temp file (exit 126, confusing message); treat empty VISUAL/EDITOR as unset like git. Kept /tmp/hyp-edit-*.md copies accumulate. YAML syntax errors in hyp edit are not labelled as front-matter errors.
<!-- SECTION:NOTES:END -->
