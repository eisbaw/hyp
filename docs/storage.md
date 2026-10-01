# Files, storage and Git

## Files and concurrency

All authoritative content is under `hyp/`:

- `config.toml` — project name and schema version (`schema_version`, see below).
- `hypotheses/`, `predictions/`, `criteria/`, `evidence/`, `links/`, `experiments/`, `runs/`, `assessments/`, `gaps/`, `data/` — one `<full-id>.md` per record.
- `assets/` — the bytes of data records (and legacy attachments), one file per SHA-256, named by it (maximum 32 MiB each).
- `.hyp/` at the project root — lock files (`write.lock`, `gate.lock`, `apply.lock`) and the temporary recovery journal; not project data. It contains a `.gitignore` that ignores it.

## Schema versions

Files are read strictly: a field hyp does not know makes a record `malformed`, which catches typos and corruption. So a field or value that older hyp versions cannot read raises the notebook's schema (decision-0004):

| Schema | First hyp version | Adds                                      |
| ------ | ----------------- | ----------------------------------------- |
| 1      | `0.1.0`           | The record fields of hyp 0.1.0.           |
| 2      | `0.2.0`           | A gap's `resolved_by` (`hyp set G-… --by`). |
| 3      | `0.3.0`           | Data records (`D-`, `hyp capture`) and `data` references; evidence attachments become data records. |

`hyp init` creates schema 1. A write raises `schema_version` when it first stores something only a later schema holds, in the same journaled write as the records, so an interrupted write leaves both or neither; it is never lowered. Reading never raises it. So a notebook stays readable by older hyp versions until a newer feature is used, and then it is not, for anyone who has not upgraded (mixed versions sharing a notebook through Git or a sync must upgrade together).

Raising a notebook to schema 3 (the first capture, `--data` reference or `hyp evidence attach`) also converts every evidence attachment into a data record in the same journaled write. The conversion depends only on the notebook, not on the clock, so two copies of a notebook (parallel worktrees, clones) convert to identical files and merge cleanly: one data record per distinct hash, with an ID derived from the hash (a UUID version 5 in hyp's namespace; a record with that ID is reused), title `Migrated attachment <first 8 hex digits>`, origin `migrated from evidence attachment <path>`, and as its times the earliest `created_at` of the evidence holding the bytes. The evidence references them via `data` instead, first and in attachment order, loses `attachments`, and keeps its `updated_at`. Stored bytes stay where they are, and no fingerprint changes (see [The model](model.md#the-model)), so no assessment needs review because of it. A write to a schema-3 notebook converts any attachment a merge from an older branch brings in the same way. With `--json` the write lists the records it converted as `"migrated": [{"id", "kind", "revision"}]` (the data records it created and the evidence it changed; left out when there are none); without, it names them on stderr. Reading never converts anything.

A hyp that meets a newer schema than it reads refuses the whole notebook with one error, kind `unsupported_schema` (exit `1`), naming the version to upgrade to, and reads and writes nothing: "this notebook uses schema 3 (…/hyp/config.toml), but this hyp 0.2.0 reads schemas 1 to 2: upgrade hyp to >= 0.3.0". From schema 3 on, the write that raises a notebook also stores that version as `min_hyp_version` in its `config.toml`; without it the message asks for a version newer than the running one. hyp 0.1.0 predates this and reports a schema-2 notebook as `unsupported schema version 2`. An unknown key in `config.toml` of a schema hyp reads, or text that is not TOML, is invalid input as before.

## Record files

Markdown files have YAML front matter and an ordinary notes body. IDs are UUIDs with readable type prefixes. Renaming files independently of IDs is rejected. Unknown schema fields, broken references and invalid states are reported by `hyp check`. `hyp check --strict` also fails on warnings such as a hypothesis with neither a falsification criterion nor an untestable reason.

## Stored bytes

Stored bytes are checked by metadata on every read: each file a record names exists, is a regular file inside `hyp/assets/`, and has the length the data record states (a length that differs is then hashed, to tell changed bytes from a changed record). Only `hyp check` hashes every stored file, so bytes changed in place to others of the same length (and any change to a legacy attachment, which records no length) show only there, as `changed_bytes`. hyp still refuses to rely on such bytes: a write that newly cites stored bytes (a capture, `--data`, `hyp evidence attach`) hashes them first, so does an assessment for the stored bytes its hypothesis's basis cites, and `hyp data get` hashes what it returns. Other writes are not blocked by them, before or after `hyp check` reports them.

## Diagnostics and repairs

`hyp check` reports every rule a record breaks, one diagnostic each. Each has a stable `code` (`hyp --json check`); `(path, code)` identifies it, and the message may change. The path is relative to the project root and always under `hyp/`: the record's file (`hyp/hypotheses/H-….md`), for every code including the warnings (`no_criterion`; `bad_observed_at`, see [Starting from an observation](usage.md#starting-from-an-observation)), or for stored bytes that no record file stands for (a legacy attachment, a stray symlink) the file under `hyp/assets/`. (hyp 0.3.0 gave `no_criterion` the bare ID and a legacy attachment a path relative to `hyp/`.) Two kinds of error differ in what they block:

- `malformed` (a file hyp cannot load: bad front matter, a filename or directory that does not match the record, a duplicate ID), `attachment` (stored bytes of a data record, or of a legacy attachment, that are missing, changed or not a regular file, and a symlink in `hyp/assets/` named like a SHA-256 (hyp ignores other entries there); for a data record the repair note says to restore `hyp/assets/<sha256>`, or to capture the original again, which stores the same bytes at the same path) and `invalid` (including a data record whose `size` does not match its intact stored bytes: the record file changed, not the bytes) (a record breaking rules of its own fields, including a reference by short ID or to a record of the wrong kind, which the ID prefix gives) block every write (`blocks_writes: true`). Fix the file by hand, or restore it. An `invalid` record can also be repaired through the CLI (`hyp edit ID`, `hyp set ID …`, a patch in `hyp apply`; the WebUI keeps showing the last readable state while writes are blocked): while only `invalid` records block, a write is accepted whose every change updates, patches, archives or deletes an invalid record, and that leaves each of them valid (or deleted); the repair may change other fields of such a record too; one that leaves a record it changes invalid, a write that changes nothing included, is `blocked`, naming what is still wrong. Any other write is `blocked` before its other checks (preconditions included). Assessments, runs and data records cannot be changed through hyp, so for them only the file can be fixed.
- `changed_bytes` (stored bytes changed in place to bytes of the same length, or a changed legacy attachment; found only by `hyp check`, see above) is an error with `blocks_writes: false`: it blocks only writes that newly cite those bytes and assessments whose basis cites them. Its repair note is the same as for `attachment`.
- `dangling_reference` (a referenced record does not exist), `cycle` (`depends-on` or `supersedes` links, or assessment supersession) and `inconsistent` (other rules between records, such as an investigating hypothesis without an active criterion) are errors between loaded records, as a merge, sync or hand edit leaves them. They do not block writes: a write is rejected if, for any record, it adds a `(path, code)` the project did not already have, and the error names each one it would add (`diagnostics` with `--json`), on whichever record: archiving a criterion can add one to its investigating hypothesis. So an unrelated write still works, and so does the write that repairs them.

Where hyp knows a repair, the diagnostic carries it: `--json` as `"repair": {"note": "...", "commands": [["hyp", "archive", "L-..."], ...]}` (`note` may be null, `commands` empty), and plain `hyp check` as `note:` and `repair:` lines. Run the commands in order, as argv arrays, in the project directory (they carry no `--project`). Plain `hyp check`, the WebUI and the HTML export print each on one line with its arguments shell-quoted where needed (`$'…'` for a newline), so a body in a command cannot break out of it. A cycle's repair archives the link. For a dangling reference the note comes first: restore the missing record from the source of the merge or sync. That loses nothing, and after a partial sync the record may simply not have arrived yet. Only a link also gets commands (archive, then delete) for when the record is gone for good; a delete cannot be undone without version control. So does a gap resolved by evidence that is gone: `hyp set G-… --by` the evidence that remains, or with none `hyp set G-… --resolved false`, which reopens it. Other records whose referenced record is missing get only the note. An `invalid` record's note says how to fix it through hyp, or that its kind cannot be changed.

## Locking

A process-shared advisory lock (`.hyp/write.lock`) serializes tool writes. Each change carries an optimistic precondition on what it depends on (see [Preconditions](agents-contract.md#preconditions)), so stale writes are rejected without turning unrelated concurrent writes into conflicts. Atomic file replacements and an fsynced roll-forward journal recover interrupted multi-file operations. Reads do not take the write lock: they share `.hyp/apply.lock`, which a write holds exclusively only while it writes, applies and removes its journal. So a read waits only for that (milliseconds), never for a whole write, and sees each write whole or not at all. A write that is ready to apply first takes `.hyp/gate.lock`, which every read passes through before it takes its shared lock, so new reads queue behind it and it waits only for the reads already in progress: reads that never stop (the WebUI polling, several tabs) cannot starve it. While a writer waits like that, new reads wait for it, so a read can be held up by the longest read in progress; `hyp check` therefore hashes stored bytes after releasing its lock (they are named by their hash and never rewritten in place, and a file removed meanwhile is reported missing). The lock order is write, gate, apply; a read that finds the journal of a crashed write lets go and rolls it forward as a writer does, under the write lock. The lock files are created with `.hyp/` whenever hyp can write there. Where it cannot (read-only media, another user's notebook), reads open them read-only, which `flock` allows; only when they do not exist and cannot be created (a copy without `.hyp/` on read-only media, where nothing writes through hyp either) do reads go without a lock. A lock file that is not a regular file (a symlink, a FIFO) is refused with the fix. On a read-only NFS mount, where an exclusive lock needs write access, reads skip `gate.lock` and lose only writer precedence. A pending journal there is an error, as it cannot be rolled forward. hyp 0.2.0 and earlier, and 0.3.0 builds from before commit `9a62a67`, do not know `gate.lock` and `apply.lock`, so a read of a later hyp can see half of a write of theirs. `hyp --version` does not tell those 0.3.0 builds apart, so do not run any hyp before 0.4.0 alongside a later one on one notebook. Manual editors, sync tools and version control do not honour that lock: ordinary overlapping saves are detected where possible, but arbitrary simultaneous external writes cannot be made transactional. Avoid a checkout, merge or sync during a tool write. Run `hyp check` after one.

## Archive and delete

Archive is recoverable; deletion is explicit and limited to unreferenced archived records. Stored bytes are retained when their data record is deleted, rather than garbage-collected automatically.

## Using hyp with Git (optional)

The design decision is [decision-0001](../backlog/decisions/decision-0001%20-%20hyp-does-not-require-or-depend-on-Git-but-is-Git-friendly.md).

hyp never runs `git` and does not need a repository; a plain directory is the normal case. It is built to sit well in one:

- One file per record, named by its ID, so concurrent work touches different files.
- Deterministic serialization, and a write rewrites only the records it changes, so diffs show real changes.
- IDs are UUIDs, so records created on different branches or machines do not collide.
- No generated index or cache files under `hyp/`; everything there is authoritative.
- `.hyp/` (lock and journal) ignores itself.

Committing, pushing and resolving conflicts are yours. Commit the complete `hyp/` directory when you want a history checkpoint. After a merge, checkout or sync, run `hyp check`: it reports broken references and malformed files, with how to repair a broken reference or a dependency cycle, and `hyp list --needs-review` shows hypotheses whose current assessment was based on records that have since changed. Two assessments of the same hypothesis made on different branches both remain current heads after the merge; record a new assessment to reconcile them.

## Exports

```bash
hyp export --format markdown --output notebook.md
hyp export --format json --output notebook.json
hyp export --format html --output notebook.html
hyp graph --focus H-… > graph.mmd
```

The HTML export embeds the current dataset and all assets and is an offline, read-only copy of the WebUI. It contains sources, notes and observations; share it deliberately. The graph command emits Mermaid source. Exported reports do not embed the stored bytes of data records; the JSON and HTML exports include the text previews the WebUI shows (`previews`, the first 4 KiB of each `text/*` data record). They also carry `unexplained_observations`: the IDs of the observations no live hypothesis accounts for, in project order, the same set `hyp --json status` lists under that name as `{"id", "title"}` objects (the export has the titles in `objects`). Like `bearings` and `previews` it is derived on read and not part of `revision`.
