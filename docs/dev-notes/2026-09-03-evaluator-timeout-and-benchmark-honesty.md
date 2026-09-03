# Dev Note — 2026-09-03 — Why a ten-turn benchmark ran one turn

## What the run actually did

A 10-turn AI-vs-AI benchmark (Aurora vs the rewritten Echo-0) stopped after one
turn. The per-turn record read:

```
turn 0  stage=completed  fallback=[structured_none, evaluator_form_v1, noop_after_all_fallbacks]  memories_after=4
turn 1  stage=evaluator_failed  ERROR: evaluator_failed: running
```

`memories_after` never moved off its starting value. Turn 0 was reported as a
completed turn while extracting nothing at all.

The evaluator job rows say the rest:

```
partial_success  elapsed 120446ms  timeout_ms 60000  patch_applied 0  error_message NULL
partial_success  elapsed 120243ms  timeout_ms 60000  patch_applied 0  error_message NULL
completed         elapsed  69195ms  timeout_ms 60000  patch_applied 1
completed         elapsed  49589ms  timeout_ms 60000  patch_applied 1
```

Four separate defects, one visible symptom.

## 1. The timeout in Settings was never the timeout that applied

`timeout_ms 60000` against profiles that all store `evaluator_timeout_ms =
25000`. `effective_evaluator_timeout_ms` reads three fields in order —
diagnostic, structured, configured — and the front end filled all three on every
call with hardcoded constants (60s, 90s, 25s). The diagnostic default won every
time. The number in the Settings panel had never reached a provider.

The front end now sends only what a profile actually stores; the structured and
diagnostic leashes stay `null` unless something asks for them. `120446ms` against
a 60s ceiling is two attempts, each cut off: the structured pass, then the
form_v1 fallback, then `noop_after_all_fallbacks`. `structured_none` in the
fallback path is not a separate cause — it is what a timed-out call leaves
behind when no enforcement level was ever negotiated.

glm-5.3-flash measured 49–70s per extraction here, because a reasoning model
spends most of its budget before the first visible token. Both 25s and 60s cut it
off mid-thought, and a slow-but-correct model read as a broken one. The default
is now 120s, and a migration raises stored `25000` values — nobody was running at
25s, so nobody chose it. A profile with any other number keeps it.

## 2. A failed extraction reported as a partial success with nothing to say

`noop_after_all_fallbacks` sets `partial_success` and a reason, but the job row
only carried an error message when form rows had been rejected. Every fallback
timing out produced `partial_success`, no error, empty patch — a row
indistinguishable from a turn where nothing happened to change. The reason is now
written through.

## 3. The benchmark counted that no-op as a finished turn

`partial_success` was in the accepted-status set unconditionally. It covers two
different endings, and only one of them read the turn: `partial_success` with no
patch applied is now a failure. `stale_skipped` and an empty-but-clean
`completed` still pass — skipping a superseded turn and finding nothing to change
are both correct outcomes.

## 4. `evaluator_failed: running`, and a lost run

The wait resolved on *this* job's terminal event but then re-read *the latest*
job for the conversation. By the time a 120-second job finished, the next turn's
job existed, and the benchmark read the newcomer's `running` as this turn's
verdict. It now waits on the job id it started with (`get_evaluator_job`, new).

And the failure ended the whole run. The narrator had already written turn 1 and
it was saved; only the extraction fell over. One slow provider call threw away
the nine turns still to come. An evaluator failure now costs its own turn and
continues; three in a row still stops, because a benchmark of an engine that
never commits state measures nothing worth paying for.

## Not a defect

The scorecard was honest throughout: `visible_turns_completed ==
visible_turns_requested` is a scored check, so 1-of-10 could not pass. The false
success was one level down, in the per-turn record.
