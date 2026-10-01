# WebUI

Run `hyp web [--port 7432]`. The server binds to IPv4 loopback only. Browse manually to the printed URL; it does not automatically launch a browser.

- Hypothesis overview: search, assessment/tag filters, needs-review and archived records, and the unexplained observations `hyp status` lists.
- Hypothesis detail: falsification criteria, predictions, evidence by what it means for the hypothesis (for, against, qualifying; evidence that meets a falsification criterion counts against), each observation once with all its links, experiments with their runs (outcome, cited evidence), gaps, assessment history, and relationships named from the hypothesis's side.
- Experiment queue and immutable runs: an experiment page lists its frozen targets and marks those changed since; a run page shows its outcome, frozen plan and cited evidence. Record pages label referenced records by role; link pages show the direction.
- Reusable evidence and interpretations: an evidence page lists the hypotheses it bears on with their stance (as `hyp show E-…`) and says when none explains it; "Explain with a new hypothesis" creates one linked to it (supports) in the same write. The WebUI's edit form cannot change a link's ends (create a new link and archive the old one); the store does not enforce this yet, so `hyp apply` still can.
- Evidence matrix to compare alternative hypotheses, each cell by what the observation means for that hypothesis.
- Focused relationship graph with navigable nodes.
- Create/edit/archive/restore/delete forms, plus advanced JSON editing for mutable records.
- Incoming changes preserve dirty forms; conflicting saves are rejected and the draft remains available to copy/reconcile.
- External editor and CLI saves update open tabs using filesystem notifications and SSE. A two-second reconciliation scan catches missed notifications. Reconnects fetch a full snapshot. Every read the server makes announces the state it saw, so a tab that read the project while it was briefly invalid or unreadable gets an event when it is valid again. As a backstop, should that event be lost, a tab whose last read failed or was blocked reads again every two seconds, up to 15 times in a row.
- Malformed files show diagnostics and block writes. An already open browser preserves the last readable state and marks it stale. Errors between records (see [Files and concurrency](storage.md#files-and-concurrency)) show as a notice with their repair; saving still works unless it adds a new error.

- Data records with their metadata and the records that reference them; for `text/*` media types a preview of the first 4 KiB, as escaped text.

Notes are displayed as escaped, pre-wrapped text. Markdown is retained in files and exports; the UI does not execute raw HTML. Data is captured through the CLI; the WebUI shows metadata and text previews and never serves the stored bytes themselves, so nothing captured can run as browser content.
