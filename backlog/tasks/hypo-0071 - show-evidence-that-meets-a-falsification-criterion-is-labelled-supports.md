---
id: HYPO-0071
title: 'show: evidence that meets a falsification criterion is labelled ''supports'''
status: Done
assignee:
  - '@claude'
created_date: '2026-09-30 17:08'
updated_date: '2026-09-30 17:36'
labels:
  - agents
  - ux
  - webui
dependencies: []
priority: high
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Dogfood v2, 2026-09-30 (Claude Code and Codex resuming yesterday's notebook on a timezone bug; both succeeded; friction reported by the agents). Evidence linked 'supports' to a criterion F (i.e. the falsifying observation was made) appears under 'supports' in plain `hyp show H`, so the proof that H is wrong reads as support for H. Agents and humans misread this. Also applies to the WebUI and the evidence matrix.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Plain show, --json show (a derived field) and the WebUI present evidence by what it means for the hypothesis: 'meets criterion F-... (counts against H)', 'matches prediction P-...', 'supports/contradicts/qualifies H'
- [x] #2 Test: evidence supporting a criterion is shown under the criterion, not as support
<!-- AC:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
- Stance model in model.rs: `Stance {For, Against, Qualifies, Mixed}`, `Stance::of_link(kind, id, relation)` gives stance + meaning; `Snapshot::evidence_bearings(h)` groups links per observation (`EvidenceBearing {evidence, stance, bearings: [Bearing {link, via, relation, stance, meaning}]}`); derive() stores it in `Snapshot.bearings` for the WebUI. Nothing stored changes.
- Plain show groups by stance (against H / for H / qualifies H / mixed); `--json show` adds `.evidence[].stance` and `.evidence[].bearings` (null/[] for run-only evidence); WebUI detail columns FOR/AGAINST (+qualifies, mixed) and matrix cells use snapshot.bearings.
- `contradicts` on a criterion (does not meet) is stance `for`.
- Tests: show_presents_evidence_by_what_it_means_for_the_hypothesis (red before: no `against H` group), DOM test step for criterion-meeting evidence (red with HEAD app.js: timed out).

- Review fix: `Stance::combined`: mixed only with both for and against links; qualifying links do not change the direction; qualifies only when all qualify. "does not meet criterion F-… (counts for H)". Test qualifying_links_do_not_make_an_observation_mixed (red before: for+qualifies was mixed).
- Wording: "counts toward its hypothesis" / "is part of this hypothesis's basis" instead of "counts for".
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Committed in agent-UX batch 1 (see commit message). Review: QA GO, architect GO with a meaning fix (Mixed only for for+against; qualifies never changes direction) plus status listing closed hypotheses that need review; confirmation GO from both. Gate: 111 Rust tests + DOM, nix flake check.
<!-- SECTION:FINAL_SUMMARY:END -->
