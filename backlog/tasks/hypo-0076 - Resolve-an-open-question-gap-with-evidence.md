---
id: HYPO-0076
title: Resolve an open question (gap) with evidence
status: Done
assignee:
  - '@claude'
created_date: '2026-09-30 17:08'
updated_date: '2026-09-30 18:30'
labels:
  - agents
  - ux
dependencies: []
priority: low
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Dogfood v2, 2026-09-30 (Claude Code and Codex resuming yesterday's notebook on a timezone bug; both succeeded; friction reported by the agents). To close yesterday's gap an agent wanted to cite the evidence that answered it; `hyp link E G` fails ('expected a hypothesis, prediction or criterion, got gap'), so the answer lived only in free text.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 A gap can be resolved with evidence (e.g. hyp set G --resolved true --by E-..., or links from evidence to gaps), shown in hyp show
- [x] #2 Decide whether gap answers enter the basis (decision-0003 says gaps do not; keep it that way unless decided otherwise)
<!-- AC:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
- Representation: `resolved_by: [E-…]` on the gap (skip_serializing_if empty), set by `hyp set G --resolved true --by E-…` (repeatable/comma-separated, replaces); `--resolved false` clears it; Invalid if non-empty on an open gap; references() includes it (dangling detection, delete protection).
- Chosen over links with a new relation `answers`: that is also unreadable by older versions (unknown Relation variant under deny_unknown_fields), and needs new link rules; the field is smaller. Caveat documented in README: older hyp reports a gap WITH resolved_by as malformed (blocks writes); gaps without it are byte-identical to before.
- Shown in plain show (Gaps: [resolved by E-…]), gap show, status (resolved_gaps in JSON, "resolved gaps N: G by E" in plain). WebUI form clears it on reopen; WebUI does not display it (filed).
- AC2: gaps stay out of the basis; test asserts the review token is unchanged.
- Test: a_gap_is_resolved_by_the_evidence_that_answered_it.

- Review round: status JSON open_gaps and resolved_gaps are both objects ({"id"} / {"id","resolved_by"}); status row shape documented in README; `hyp set` about mentions resolving a gap; skill Example resolves its gap with --by.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Committed in agent-UX batch 2 (see commit message). Review: QA GO, architect NO-GO (batch-created record as the target of update/patch/archive/delete returned a retry-forever conflict) -> fixed as invalid_input, plus skill without shell variables under 200 lines; confirmation QA GO, architect GO. Gate: 119 Rust tests + DOM, nix flake check.
<!-- SECTION:FINAL_SUMMARY:END -->
