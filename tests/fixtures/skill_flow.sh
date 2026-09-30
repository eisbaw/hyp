# The skill's method as one script, run by the drift test in tests/cli.rs
# (skill_flow_runs_against_this_binary) under `bash -euo pipefail` with hyp on
# PATH, in a fresh project. It uses shell variables to carry printed IDs from
# one command to the next; agents read and type them instead (see
# agents/hyp/SKILL.md, Example). Keep its commands and flags in step with the
# skill's Example and Commands.
hyp status                               # where things stand
hyp search "login"                       # anything recorded already?
H1=$(hyp add "Flaky login test is caused by a shared temp dir" \
  --scope "tests/login.rs on CI" --tags ci,flaky)
F1=$(hyp falsify-if "$H1" "Test still fails with per-test temp dirs")
hyp set "$H1" --lifecycle investigating
H2=$(hyp add "Flaky login test is caused by clock skew" --scope "CI")
hyp link "$H1" "$H2" --relation competes-with --reason "Both explain it"
G1=$(hyp gap "$H1" "Does isolation alone stop the failures?")
X1=$(hyp experiment add "$H1" "Rerun with per-test temp dirs" --targets "$F1" \
  --body "cargo test login -- --test-threads=8, 200 iterations")
E1=$(hyp evidence add "$F1" "200/200 passes with per-test temp dirs" --against \
  --source "cargo test login -- --test-threads=8" --locator "CI job 4411" \
  --reason "Isolation alone removed the failures" \
  --body "Before: 23/200 failed on CI job 4402. Clock not varied." \
  | sed -n 1p)                           # the E- line; the L- line follows
hyp run "$X1" "200 iterations" --outcome observed --evidence "$E1"
hyp set "$X1" --experiment-status completed
hyp link "$E1" "$H2" --relation contradicts \
  --reason "Clock unchanged, yet the failures stopped"
SHOW=$(hyp show "$H1"); printf '%s\n' "$SHOW"   # review it all before judging
TOKEN=$(printf '%s\n' "$SHOW" | sed -n '1s/.*  review //p')   # from line 1
hyp assess "$H1" --reviewed "$TOKEN" --status supported --confidence 0.7 \
  --evidence "$E1" --reason "Isolation removed all failures in 200 runs"
SHOW=$(hyp show "$H2"); printf '%s\n' "$SHOW"   # each judgment its own review
TOKEN=$(printf '%s\n' "$SHOW" | sed -n '1s/.*  review //p')   # from line 1
hyp assess "$H2" --reviewed "$TOKEN" --status weakened --confidence 0.3 \
  --evidence "$E1" --reason "Failures stopped with the clock unchanged"
hyp set "$G1" --resolved true --by "$E1"
hyp set "$H1" --lifecycle closed         # the assessment still holds
hyp list --needs-review                  # empty: nothing to review again
