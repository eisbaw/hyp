# The model

| Record     | Purpose                                                                        |
| ---------- | ------------------------------------------------------------------------------ |
| Hypothesis | Claim, scope, assumptions, lifecycle, tags                                     |
| Prediction | Expected observable result, conditions, owning hypothesis                      |
| Criterion  | Observation that would falsify the scoped claim                                |
| Evidence   | Observation, source, locator, date                                             |
| Link       | Interpretation: supports, contradicts, qualifies; or a hypothesis relationship |
| Experiment | Procedure, status, frozen hypothesis/prediction/criterion references           |
| Run        | Immutable snapshot of the experiment plan, outcome and evidence IDs            |
| Assessment | Immutable judgment, subjective confidence, rationale and evidence IDs          |
| Gap        | Missing information, open or resolved, optionally by cited evidence            |
| Data       | Immutable captured bytes: title, origin, captured_at, media type, size, sha256 |

Every record except a data record may reference data records (`data: [D-…]`).

Predictions and criteria are separate Markdown records, which makes them individually addressable and avoids rewriting the hypothesis every time one is added.

Lifecycle is **draft / investigating / paused / closed**. Assessment is **untested / inconclusive / supported / weakened / falsified**. Closing an investigation never declares its hypothesis true.

- Drafts may be incomplete. Investigating requires an active criterion or an explicit `untestable_reason`.
- Every assessment needs a rationale. Every judgment except untested must cite evidence linked to the hypothesis (or its active criteria or predictions). Falsification also names an active criterion belonging to it, and at least one cited observation must meet that criterion: have an active `supports` link to it (`hyp evidence add F-…`, or `hyp link E-… F-… --relation supports`). Evidence merely against the hypothesis does not falsify it. `hyp assess --help` lists each judgment's requirements. These rules apply when an assessment is created; a stored one stays valid if a link it relied on is archived later. Whoever records the assessment, agent or human, judges whether the observation actually satisfies the criterion; hyp checks only that it was recorded as meeting it.
- Meeting a criterion and not meeting it are not symmetric. Evidence that meets a falsification criterion is decisive: it is what `falsified` rests on. Evidence that a criterion was not met (`--against` on it) counts for the hypothesis only as mild corroboration, one refutation it survived; it proves nothing, and `supported` never means proven.
- Confidence is optional, subjective, and in `[0, 1]`. Evidence counts never calculate it.
- An assessment records the hypothesis's fingerprint: the SHA-256 of its `basis` (`hyp --json show`) as compact JSON with sorted keys. The basis holds content only: the claim (title, body, scope, assumptions, archived); its criteria and predictions (title, body, conditions, archived); links touching the hypothesis or those (ends, relation, reason, archived), so another hypothesis counts only through its link; the evidence with an active link to the hypothesis or an active criterion or prediction; and runs of its experiments (title, body, outcome, cited evidence) with the evidence they cite. Evidence counts with its provenance (title, body, source, locator, attachment hashes, archived). Each of these records, the hypothesis included, that references data records counts with their SHA-256s, in order: under `data`, and for evidence after its attachment hashes under `attachments` (so converting an attachment into a data record changes no fingerprint); the data records' other metadata does not count. A change to it shows **needs review** without rewriting the judgment. Lifecycle, tags, the untestable reason, experiments, gaps and timestamps are not part of it, so closing a hypothesis does not flag it; archiving it does.
- A new assessment supersedes the current assessment heads. Divergent heads after any merge or sync require explicit reconciliation; neither silently wins by timestamp.
- Experiments freeze complete target content and revisions at creation. Runs freeze the complete experiment plan at execution-record creation. `hyp run` records an execution; it does not execute commands.
- A gap can name the evidence that resolved it (`resolved_by`, `hyp set G-… --resolved true --by E-…`); `--resolved false` forgets it. Gaps stay outside the basis (decision-0003), so resolving one never flags an assessment. The first `resolved_by` raises the notebook to schema 2 (see [Schema versions](storage.md#schema-versions)).
- Historic assessments and runs cannot be edited or deleted through the tool. To keep a history of all file edits, use any version control, e.g. Git. There is no claim of tamper-proof auditing.

## Conceptual model

Records kept beside code differ by *direction of fit*: when record and world disagree, which one must change? Zave and Jackson draw this line for software between indicative statements, the environment as it is, and optative ones, the environment as we want it (“Four Dark Corners of Requirements Engineering”, ACM TOSEM 6(1), 1997); philosophers call the same distinction direction of fit, after Anscombe's shopping list (*Intention*, 1957) and Searle's words-to-world and world-to-words directions ("A Taxonomy of Illocutionary Acts", 1975; in *Expression and Meaning*, 1979).

| Kind         | Holds                               | Direction of fit                         | Example                   |
| ------------ | ----------------------------------- | ---------------------------------------- | ------------------------- |
| Definitional | What there is: ontology, vocabulary | A definition fixes the terms             | glossary, schema          |
| Descriptive  | Observations, hypotheses, facts     | Record to world: the record is corrected | **hyp**                   |
| Normative    | Specifications, invariants          | World to record, for as long as it holds | specs, tests, type checks |
| Imperative   | Tasks: what to do                   | World to record, until it is done        | backlog.md                |

Normative and imperative records share the world-to-record direction and differ by persistence: a specification keeps holding, a task closes.

Descriptive claims also differ in epistemic status:

```text
observation -> hypothesis -> corroborated -> fact
                          \-> refuted
any of these -> stale, when what it rested on changes
```

An observation is a dated record of something seen. A hypothesis is a claim that would explain observations or predict new ones. Checks move it: each is an evidential event (an experiment run, an observation linked to a criterion or prediction) followed by a judgment that cites it. A hypothesis that survives checks that could have refuted it is corroborated; one whose check meets a falsification criterion is refuted; a claim corroborated well enough that people stop questioning it is treated as a fact.

hyp covers the descriptive kind up to corroboration. Observations are evidence (`hyp observe`; captured bytes are data records), hypotheses carry their criteria and predictions, experiments and runs are the checks, and assessments are the judgments: `supported` for corroborated, which never means proven, and `falsified` for refuted. Staleness shows as **needs review** when anything an assessment's basis holds changes; a world that changes without a new record goes unnoticed. Facts are out of scope: hyp has no "true" state and closing an investigation declares nothing, so a claim that has become a fact moves out, into documentation or into a test, where its direction of fit turns normative.

## Scope of this release

This is a local, single-worktree tool. It supports multiple CLI processes and browser tabs, not networked multi-user collaborative editing. It reads the notebook into memory and rescans the record files on each read (stored bytes only by metadata; `hyp check` hashes them); it is intended for small and medium research/debugging notebooks, not millions of evidence records. Full snapshot refreshes favour correctness and simplicity over incremental-index complexity.

hyp itself makes no judgments: agents and humans record them. No automated experiment execution, Bayesian scoring, MCP server, remote hosting, user accounts or statistical-analysis engine are included.

See [VALIDATION.md](../VALIDATION.md) for the checks run and the commit they cover.

## When not to use hyp

hyp records an investigation; it does not carry one out, and it holds tentative claims, not settled ones. Its technical limits (local and single-worktree, small and medium notebooks, no experiment execution, scoring or accounts) are in [Scope of this release](#scope-of-this-release). It is also the wrong tool for:

- **Statistical analysis.** Use a statistics environment and cite its output as evidence (`hyp capture` keeps the bytes). A confidence in hyp is a number someone entered, not a computed one.
- **Automated hypothesis testing.** Agentic falsification frameworks design and run the experiments; hyp can record their results.
- **Literature search or hypothesis generation.** hyp neither searches sources nor proposes claims.
- **Debate and argument mapping.** Argument-mapping tools model premises, objections and the structure of an argument; hyp models claims, evidence and judgments in one investigation.
- **A shared knowledge base or agent memory.** hyp has no retrieval and no access control. A claim that is no longer tentative belongs in documentation, a specification or a test (see [Conceptual model](#conceptual-model)).
- **Publishing a one-off evidence map.** When one question is settled in one sitting and the result is a document for readers, an evidence-map tool fits better than a notebook with a lifecycle and review tracking.
- **Plans and tasks.** Use a task tracker such as backlog.md. A hyp experiment is the procedure for testing a claim, not a work item.
