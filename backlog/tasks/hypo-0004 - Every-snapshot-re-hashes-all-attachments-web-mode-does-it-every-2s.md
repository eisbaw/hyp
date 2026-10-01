---
id: HYPO-0004
title: Every snapshot re-hashes all attachments (web mode does it every 2s)
status: Done
assignee:
  - '@claude'
created_date: '2026-09-29 22:15'
updated_date: '2026-10-01 02:01'
labels:
  - performance
dependencies: []
priority: high
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Found in the initial review. Measured.

`read_unlocked` reads and SHA-256 hashes every attachment on every snapshot. `commit` does a full read three times, plus another hash pass during validation. The web server polls a snapshot every 2s and on each file event, and every browser tab requests one on each SSE change. Each read also takes the exclusive write lock.

Measured `hyp list` with six 30 MB attachments: 0.28-0.35s release (4.2s debug), vs 0.01s without. This scales linearly with attachment volume, and web mode burns CPU continuously. Attachments are content-addressed and immutable, so full re-hashing on every read is unnecessary.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 `hyp check` still detects a modified or missing attachment
- [x] #2 A single commit performs at most two full reads of the notebook
- [x] #3 Benchmark note in the commit message with before/after timings
- [x] #4 Ordinary snapshots only check attachment existence and containment (no bytes read); hashing happens in `hyp check` and when committing evidence that references the attachment. No stored hash cache
- [x] #5 Reads do not take the exclusive write lock (shared lock or lock-free read with recovery only under the write lock), so the 2s web poll, open tabs and writers do not queue behind each other; a notebook on read-only media can be read
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Reads: a Verify mode (Metadata for every read, Content for hyp check); stored bytes checked by stat (exists, regular, inside assets/, size), hashing only on a size mismatch.
2. Writes hash only bytes a written record newly cites (verify_cited); capture/attach keep hashing.
3. Locks: write.lock stays for writers (whole commit); new apply.lock held exclusively only while the journal is written+applied; reads share apply.lock; a reader seeing a journal under the shared lock rolls it forward under the exclusive lock.
4. Read-only media: no layout creation, shared lock opened read-only, or lockless when the lock file cannot exist; pending journal = clear error.
5. Commit: two full reads; the pre-write check compares record-file hashes (no parse, no blobs).
6. hyp web: ignore inotify open/read events (feedback loop).
7. Tests that bite, README, benchmark.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
HYPO-0090: data records' stored bytes (hyp/assets/<sha256>, up to 32 MiB each) are now verified the same way on every read, once per distinct hash, so this cost grows with captured data.

Raised to high after the HYPO-0090 review: with one 32 MiB capture a debug /api/snapshot takes 4.5-9.6 s and idle hyp web uses ~84% of a core; every read hashes all stored bytes, a write reads the notebook three times. Cheap fixes: per-process hash cache keyed by (inode, len, mtime); skip byte verification in the pre-write re-read.

- Root cause of idle hyp web CPU was not only the 2 s tick: notify 8 watches IN_OPEN, so every snapshot's own file reads woke the monitor 120 ms later for another snapshot (a feedback loop). web.rs now ignores access events other than close-after-write (changes_files, unit-tested).
- Locking: .hyp/write.lock (writers, whole commit, unchanged) + .hyp/apply.lock (writers hold it exclusively only while writing the journal, applying it and removing it; reads hold it shared). A journal visible under the shared lock can only be a crashed write's; the reader drops shared, takes exclusive, rolls forward, reads under it. Lock order write->apply only; readers never take write.lock: no deadlock.
- Read-only media: Store::open and reads create the layout only where writable (PermissionDenied/ReadOnlyFilesystem are tolerated); apply.lock opened read-only if present; if it cannot exist, the read is lockless (no hyp can write there); a pending journal there is an error.
- What an ordinary read no longer detects: stored bytes changed in place to other bytes of the same length. hyp check (Verify::Content) hashes every stored file; a write that newly cites the bytes is refused (verify_cited); hyp data get hashes. A write that does not cite them is no longer blocked until hyp check reports them. Previews still read the first 4 KiB of text/* data records (bounded), and a size mismatch is hashed to tell changed bytes from a changed record, as before.
- Commit: 2 full reads (before, after) + one record-file hash pass (Store::files) replacing the third full read; unit test counts full reads.
- Mixed versions: hyp 0.3.0 writers do not take apply.lock; documented in README (do not run them alongside).
- Benchmark (release, 200 records + 7 x 30 MiB captures = 211 MiB, median of 5, warm cache; HEAD 18c2a3f vs this change): hyp list 0.316 s -> 0.010 s; hyp status 0.248 -> 0.011; GET /api/snapshot 0.356 -> 0.016; hyp add (write) 0.889 -> 0.041; observe --data citing a 30 MiB capture 0.58 -> 0.06; hyp check 0.16 -> 0.16 (still hashes all); idle hyp web CPU over 20 s 60.3% -> 0.2% of a core. Debug build after: list 0.035, snapshot 0.038, write 0.104, idle 1.3%. First cold run before: list 1.0 s, write 3.1 s, idle 76%.
- Filed HYPO-0098 (capture hashes bytes up to 3x) and HYPO-0099 (2 s tick still parses all records); noted the std/fs2 lock_shared MSRV clash on HYPO-0008.

Commit message draft:

Reads without hashing or the write lock (HYPO-0004)

Ordinary reads check stored bytes by metadata (exists, regular file inside
hyp/assets/, size); hyp check hashes every stored file, and a write that
newly cites bytes hashes them first. Reads share .hyp/apply.lock, which a
writer holds exclusively only while it writes and applies its journal, so a
read never waits for a whole write and never sees half a transaction; a
reader that finds a crashed journal rolls it forward. Read-only notebooks
can be read. A commit reads the notebook fully twice. hyp web ignores
inotify open events, which made every snapshot trigger the next one.

Benchmark, release, 200 records + 211 MiB of captures, median of 5:
  hyp list        0.316 s -> 0.010 s
  hyp status      0.248 s -> 0.011 s
  /api/snapshot   0.356 s -> 0.016 s
  hyp add         0.889 s -> 0.041 s
  idle hyp web    60.3% -> 0.2% of a core

Review fix round (deep review: scout starvation, architect F1-F3):
- Writer starvation fixed with .hyp/gate.lock. Lock order write -> gate -> apply. Writer (holding write.lock): gate EX, apply EX, write+apply+remove journal, release. Read: gate EX, apply SH, release gate, read. A waiting writer holds the gate, so new reads queue and it waits only for reads in progress. Scout starve.sh on the 820-file notebook (debug): before, writer killed at 22 s with 4 and 8 looping readers; after, 1.7 s (4 readers) and 2.6 s (8), of which the flock wait for apply.lock was 0.25 s / 0.49 s (idle write there ~0.5 s). Scout webstress (6 pollers + 4 writers, 30 s): max CLI write 4.4 s (scout saw 21.6 s), no violations, no lost writes. stress.py 6 readers/3 writers 25 s: 0 violations, 0 lost, max write 3.3 s. crash.sh: readers never see E!=L after SIGKILL at any rename/unlink.
- F1: a read that finds a journal now drops its shared lock and rolls it forward through Store::lock (write.lock, then gate+apply), then reads under the shared lock again.
- F3: an assessment hashes the stored bytes its basis cites (verify_basis).
- F2: new diagnostic code changed_bytes (blocks_writes false) for bytes changed in place that only hyp check finds; attachment keeps blocks_writes true for what every read sees (missing, not regular, other length). README and Code::blocks_writes document it.
- ensure_layout creates all three lock files; README read-only text corrected.
- recover and journal writing name the file and what to do on failure.
- New tests: write not starved by looping reads; write waits for a read holding the shared lock; read finding a journal waits for write.lock; reads never see a criterion without its hypothesis (bites the no-apply-lock mutant 3/3); assessment over changed bytes refused.

Confirmation round:
- hyp check (Verify::Content) now hashes stored bytes after releasing the shared lock (Store::hash_stored, streaming SHA-256): files are named by their hash and never rewritten in place; one removed meanwhile is reported missing (attachment, blocks writes), one changed as changed_bytes; the revision is re-derived. On the scout's 733 MB notebook (debug): check 15.4 s; with an add queued during it, add 0.03 s and list 0.02 s (scout saw list 12.6 s).
- read_lock wraps only permission errors with "rolling it forward needs write access".
- Lock files: opened O_NONBLOCK|O_NOFOLLOW and checked to be regular files (FIFO/symlink refused, naming the file and the fix); a missing lock file in a read-only .hyp gives "cannot open ...: hyp needs write access to .../.hyp; nothing was written" (kind io) for each lock file.
- NFS: an EBADF on the read-only gate lock skips the gate (documented).
- SKILL.md lists changed_bytes (199 lines).
- Tests: check hashing does not hold up reads behind a queued writer; lock files that are not regular files; broken journal reported as is; write-access message for each missing lock file; bytes deleted/changed between read and hashing.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Reads no longer hash stored bytes or take the write lock; a three-lock protocol (write -> gate -> apply) gives atomic reads with writer preference; hyp check hashes outside the lock; the web watcher ignores open/read events (the idle-CPU loop). Release benchmark (200 records + 211 MiB captures): list 0.316->0.010 s, status 0.248->0.011 s, /api/snapshot 0.356->0.016 s, one write 0.889->0.041 s, idle hyp web 60%->0.2% of a core. Deep review: QA GO; architect conditional GO (F1 recovery under write.lock, F2 blocks_writes semantics, F3 verify basis on assessment); adversarial scout found writer starvation (fixed with the gate: 22 s+ -> ~2-4 s under 8 readers) and long-read stalls (fixed: check hashes outside the lock); confirmation GO from all; final QA gate GO (timing tests clean in 9 runs total). Gate: 173 Rust tests + DOM, nix flake check.
<!-- SECTION:FINAL_SUMMARY:END -->
