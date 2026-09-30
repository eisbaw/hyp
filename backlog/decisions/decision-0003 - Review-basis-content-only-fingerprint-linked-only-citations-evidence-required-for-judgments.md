---
id: decision-0003
title: >-
  Review basis: content-only fingerprint, linked-only citations, evidence
  required for judgments
date: '2026-09-30 05:53'
status: accepted
---
## Context

An assessment records the state it was based on (`based_on`, the hypothesis fingerprint). When that state changes, the assessment shows *needs review*. The fingerprint hashed the raw bytes of every record reachable from the hypothesis, including a one-hop closure over links from cited evidence. In the 2026-09-30 dogfood, both agents ended with every hypothesis flagged, because closing a hypothesis (the skill's last step) changed its bytes. The closure over cited evidence was also the root cause of most of the precondition machinery (`relevant_with`, `assessment_additions`, the JS copy `expectedFrom`) and of several Codex review findings. The tool enforced evidence only for `falsified`, leaving other judgments to prose in the skill.

## Decision

1. **Content-only fingerprint.** Only substantive content counts:
   - title and body of the hypothesis, its criteria, predictions and evidence (for these kinds the title is the claim or observation itself);
   - the hypothesis's scope and assumptions;
   - links: relation and reason;
   - evidence and criteria being added or archived;
   - run outcomes and cited evidence.
   Lifecycle, tags, experiment status, gap resolution and `updated_at` do not count. For linked or competing hypotheses, only the link counts, not the other hypothesis's content. The review token also covers the current assessments.
2. **Linked-only citations.** An assessment may cite only evidence that is already linked to the hypothesis (or its criteria or predictions). To use other evidence, link it first (`hyp link E H`).
3. **One migration wave.** No scheme version. Existing assessments may show needs-review once after the change.
4. **Evidence required.** Every judgment except `untested` must cite at least one evidence ID, for all writers, agents and humans alike. `falsified` additionally needs a criterion.

## Consequences

- Closing, retagging or changing experiment status no longer flags assessments. Editing a claim, criterion, prediction or observation still does.
- The fingerprint hashes a canonical projection of the listed fields, not file bytes. Object revisions stay raw-byte hashes, which is right for detecting external edits.
- With linked-only citations, an assessment adds nothing to the basis, so `relevant_with`, `assessment_additions`, the README's 'state the evidence, its links and both ends' recipe and `expectedFrom` in web/app.js can go. `expected.revisions` for assessments shrinks or disappears.
- HYPO-0031, HYPO-0009 and HYPO-0026 are implemented as one deep-gated change. HYPO-0025 loses its cited-evidence AC. The skill's Assessing section and the README precondition contract are rewritten once, after this change.
- The evidence rule is enforced by the tool, and the skill's 'your rule' wording goes. The WebUI assessment form requires evidence.
