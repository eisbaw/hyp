---
id: decision-0002
title: >-
  hyp is primarily a tool for agents to work in a structured way with tentative,
  unconfirmed information; humans inspect and steer
date: '2026-09-29 22:27'
status: accepted
---
## Context

The motivating need was "backlog.md, but for hypotheses". Coding and research agents generate hypotheses constantly ("the bug is probably X") and often declare a root cause without a falsification criterion or cited evidence. A structured, file-based record of tentative claims, criteria, evidence and judgments gives agents discipline and gives humans something to inspect.

## Decision

hyp is primarily a tool for agents to work in a structured way with tentative, unconfirmed information. Humans are secondary users: they inspect and steer, mainly through the WebUI. Agents may record assessments, including `falsified`, but every agent assessment must cite evidence and give a rationale.

## Consequences

- Agent onboarding (HYPO-0016) is central to adoption; `hyp` should install instructions/skills the way `backlog init` does.
- Prefer rules the tool enforces with clear, stable errors over guidance in prose, because agents follow tool errors reliably (e.g. evidence required for assessments).
- The CLI's machine interface (`--json`, exit codes, error messages) is a stable contract.
- Ceremony (many record types, UUID IDs) costs agents little. Optimise for correctness of the record over human typing convenience, but keep the WebUI readable for humans.
- Concurrent agent and human writes are normal, so preconditions must be precise enough to avoid spurious conflicts without losing real ones (HYPO-0002).
