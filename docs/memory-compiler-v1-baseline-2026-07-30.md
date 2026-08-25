# Memory Compiler V1 Offline Baseline

- Date: 2026-07-30
- Corpus version: 1
- Structured schema version: 1
- Structured compiler version: 1
- Scope: deterministic Rust parse/validate/lower boundary

## Corpus

11 cases:

- 7 successful outputs, including a pure OOC no-op
- 4 total semantic rejections
- 1 partial-accept case
- world event, scene, object, relationship, direct memory, hearsay memory
- fabricated evidence, missing scene evidence, two engine-truth escalation attempts
- evaluator-controlled source message spoof

## Baseline Result

| Check | Result |
|---|---|
| Fixture JSON/schema parse | pass |
| Expected accepted/rejected counts | pass |
| Expected effect-family counts | pass |
| Engine-only truth rejection | pass |
| Source message spoof discarded during lowering | pass |
| Scene evidence required and grounded | pass |
| Repeated compilation patch equality | pass |
| Repeated compilation serialized-byte equality | pass |

## Regression Verification

- `cargo test --manifest-path src-tauri/state_engine/Cargo.toml`: pass
- `cargo test --manifest-path src-tauri/Cargo.toml --lib --jobs 2`: 416 passed,
  1 ignored

## Measurement Boundary

This is the offline compiler baseline, not a claim about any one provider/model's
extraction quality. Live latency, token cost, candidate recall, and provider
variance require model calls. They will be captured with the existing structured
diagnostic runner when V2 shadow execution is introduced, so V1 and V2 see the
same turns under the same provider conditions.

## Next Expansion

M2/M3 extends this corpus with explicit Perception IR expectations and metamorphic
pairs for:

- paraphrase
- negation
- actor swap
- perceiver swap
- direct observation versus hearsay
- temporal shift
- retcon/correction
