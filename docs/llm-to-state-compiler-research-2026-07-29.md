# LLM → State Compiler 발전 연구

작성일: 2026-07-29  
범위: Mnemosyne의 자연어 대화가 검증 가능한 상태·기억 변경으로 변환되는 경로  
비범위: Neural Map UI, 그래프 시각화, 특정 그래프 데이터베이스 선정

## 1. 결론

Mnemosyne의 현재 Evaluator는 단순한 JSON 생성기가 아니다. 이미 다음 형태의 작은 컴파일러다.

```text
최근 사용자 입력 + Narrator 서술
  → LLM이 제한된 EvaluatorOp AST 생성
  → Rust strict parse
  → 증거 인용, 엔티티, soul 소유권 검증
  → EnginePatch로 lowering
  → 상태 패치 원장에 commit
  → branch를 따라 replay하여 Soul + SessionWorld 재구축
```

이 토대는 좋다. 특히 범용 메모리 프레임워크보다 Mnemosyne의 목적에 더 가까운 요소가 이미 존재한다.

- LLM 출력과 실행 가능한 상태 패치가 분리되어 있다.
- LLM이 만든 각 op를 독립적으로 검증하고 부분 수용할 수 있다.
- source quote와 entity identity를 검증한다.
- 패치는 append-only 원장에 저장되며 branch/variant/rewind 후 재생 가능하다.
- 관계 변화는 표면 감정 수치를 직접 쓰는 대신 관계 사건의 축과 modifier를 계산기로 전달한다.

하지만 다음 세대로 확장할 때 현재 AST에 기억 종류와 그래프 op를 계속 추가하는 방식은 한계가 명확하다.

**차세대의 핵심은 LLM이 상태 변경 명령을 직접 생성하지 못하게 하고, 증거에 묶인 관찰·주장·해석 후보만 생성하게 하는 것이다.** Rust 컴파일러가 이 후보를 바인딩하고 의미 분석한 뒤, 허용된 상태 효과로 낮춰야 한다.

권장 구조는 일반 컴파일러와 유사하다.

```text
Source Envelope
  → Perception IR
  → Bound Claim IR
  → Validated Event IR
  → State Effects
  → Transaction Simulation
  → Ledger Commit
  → Memory / Graph / Vector Projections
```

Cognee는 파이프라인과 projection 구조의 좋은 참고 자료다. 그러나 Mnemosyne의 차세대 컴파일러에 가장 직접적인 발전 재료는 다음 조합이다.

- Mnemosyne 현재 구현: 정본, branch, evidence, deterministic lowering
- Cognee: task pipeline, typed DataPoint, provenance, graph/vector projection
- Hindsight: world/experience/observation 분리와 evidence-backed consolidation
- Graphiti/Zep: episode provenance와 bi-temporal fact validity
- HippoRAG 2: 그래프를 write authority가 아닌 associative retrieval index로 사용
- constrained decoding 연구: 문법적 유효성과 의미적 안전성을 별도 측정

## 2. Mnemosyne의 목적에 대한 해석

Mnemosyne은 범용 대화 검색기가 아니다. 목적은 다음과 같다.

1. Narrator가 자유로운 산문을 만든다.
2. 별도 Evaluator가 산문과 사용자 행동에서 지속되어야 할 변화를 읽는다.
3. 엔진은 객관적 세계, Soul의 주관적 경험, 관계 변화, 기억을 재현 가능한 상태로 만든다.
4. 다음 Narrator 호출에는 전체 로그가 아니라 현재 정신과 상황에 필요한 상태만 컴파일한다.
5. regenerate, rewind, branch 전환 후에도 선택된 서사에 맞춰 상태가 결정론적으로 복구된다.

따라서 성공 기준은 “대화를 잘 요약했는가”가 아니다.

- 실제로 일어난 사건과 단순 발언을 구분했는가
- 누가 행동했고 누가 보았는가
- 세계의 사실과 Soul의 믿음을 구분했는가
- 관계 변화가 근거와 비례하는가
- 분기가 바뀌면 파생 기억도 함께 사라지거나 재구축되는가
- 같은 원장과 컴파일러 버전에서 같은 상태가 나오는가
- 잘못된 LLM 출력이 정본 권한을 얻지 못하는가

이 때문에 Cognee식 범용 knowledge extraction을 그대로 중심에 둘 수 없다.

## 3. 현재 LLM → 코드 경로

### 3.1 LLM이 만드는 AST

`src-tauri/state_engine/src/evaluator_structured.rs`의 `EvaluatorStructuredOutputV1`은 다음 op만 허용한다.

- `add_memory`
- `relationship_event`
- `update_object_state`
- `update_scene_state`
- `add_world_event`
- `no_op`

Serde의 `deny_unknown_fields`와 provider JSON Schema enforcement를 사용하므로 임의 필드나 임의 코드 실행은 허용되지 않는다.

### 3.2 Rust semantic lowering

`compile_evaluator_ops_to_engine_patch`는 op별로 다음을 수행한다.

- active Soul 및 entity alias 해석
- Soul 전용 필드에 player alias가 들어간 경우 교정 또는 거부
- entity 존재 여부 검증
- `evidence_quote`가 최신 사용자/Narrator 원문에 실제로 존재하는지 검증
- 수치 clamp
- 안정 ID 생성
- `EnginePatch`의 memory, relationship, world operation으로 변환
- 잘못된 op만 거부하고 다른 op는 부분 수용

모든 op가 거부되면 전체 structured 경로를 실패로 처리하여 form fallback 또는 repair 경로를 사용할 수 있다.

### 3.3 패치 적용과 원장

`EnginePatch::apply_to_session`은 `Soul`과 `SessionWorld`에 패치를 적용한다. DB에서는 baseline/enrichment patch가 `state_patches`에 저장되고, active commit path를 따라 순서대로 replay된다.

이 구조의 중요한 성질은 materialized Soul JSON이 정본이 아니라는 점이다. 정본은 base state와 활성 patch 계보다.

### 3.4 Context Compiler

현재 Context Compiler는 활성 memory를 대상으로:

- truth/source/entity/owner 필터
- salience와 retrieval strength
- 최근 대화 어휘와의 일치
- plot 및 world term 일치
- 여섯 개 memory slot별 affinity
- slot당 개수 제한
- 전체 토큰 budget

을 적용한다.

즉, state compiler와 prompt compiler가 이미 분리돼 있다. 차세대 memory retrieval도 이 경계를 유지해야 한다.

## 4. 현재 구현의 강점

### 4.1 Patch가 LLM 원문과 분리되어 있다

LLM 응답을 바로 상태로 deserialize하지 않고 `EvaluatorOp → EnginePatch` 변환을 거친다. 이는 compiler IR/lowering 패턴이며 계속 유지해야 한다.

### 4.2 Evidence-addressed operation

대부분의 durable op가 `evidence_quote`를 요구하고 원문 포함 여부를 검증한다. 범용 메모리 시스템에서 자주 빠지는 중요한 안전장치다.

### 4.3 Partial acceptance

한 후보의 오류 때문에 전체 턴의 올바른 기억과 사건까지 버리지 않는다. 동시에 rejected candidate와 사유가 trace에 남는다.

### 4.4 Branch-aware event sourcing

regenerate/rewind에서 파생 상태를 직접 수선하지 않고 활성 patch를 다시 재생한다. 시간 기억과 그래프 projection의 정본으로 쓰기에 매우 유리하다.

### 4.5 관계를 사건에서 계산하려는 방향

LLM이 최종 `trust += 12`를 직접 쓰기보다 행동 특성과 modifier를 출력하고 Rust calculator가 관계 표면을 계산한다. 차세대 전체 상태 변경도 이 패턴으로 확장해야 한다.

## 5. 현재 구현의 구조적 한계와 위험

### 5.1 Perception과 Effect가 같은 AST에 섞여 있다

`add_world_event`, `update_object_state`, `add_memory`는 이미 실행 의도를 가진 imperative op다. LLM이 다음 두 판단을 동시에 수행한다.

1. 무엇을 관찰했는가
2. 그 관찰이 어떤 저장소와 상태를 변경해야 하는가

이 결합은 op 종류가 늘수록 prompt와 semantic validator를 폭발시킨다.

### 5.2 LLM이 engine-only truth label을 출력할 수 있다

`TruthStatusOp`에는 `verified_engine`과 `actual_system_event`가 포함되고 현재 lowering은 이를 `TruthStatus`로 직접 변환한다. `architecture_verified`가 false로 저장되더라도 truth label 자체에 엔진 권위가 부여될 수 있다.

원칙적으로 다음 필드는 LLM schema에 존재해서는 안 된다.

- engine verified
- actual system event
- architecture verified
- branch identity
- source message address
- compiler version

이들은 코드가 source envelope과 실행 결과를 바탕으로 주입해야 한다.

### 5.3 Source address 일부를 모델이 제출한다

`source_message_id`를 모델이 출력하게 하면 quote가 맞더라도 잘못된 메시지 주소를 붙일 수 있다. 모델은 quote 또는 span 선택만 해야 하고 message/turn/branch ID는 호출 컨텍스트에서 compiler가 주입해야 한다.

### 5.4 Scene update의 evidence contract가 약하다

현재 structured `UpdateSceneStateOp`은 `evidence_quote`가 없다. 장면 focus, participant, pressure point, continuity note가 durable state에 영향을 주면서도 다른 op와 같은 provenance 검증을 받지 않는다.

### 5.5 단일 confidence가 여러 의미를 겸한다

추출 확신, 출처 신뢰도, 캐릭터의 믿음 강도, 객관적 참일 가능성은 서로 다르다. 현재 memory confidence 하나로는 다음을 표현하기 어렵다.

- Evaluator가 문장을 올바르게 읽었을 확률
- Soul이 그 주장을 믿는 정도
- 그 주장이 실제 세계에서 참인 정도
- 기억 자체가 선명한 정도

### 5.6 Fixed slot이 기억 존재론과 prompt 표현을 겸한다

`relationship_memory`, `current_plot_memory` 등은 검색 렌즈로는 유용하지만 기억의 본질적 종류가 아니다. 예를 들어 같은 경험 기억이 관계, 감정, 미해결 긴장 슬롯 모두에 관련될 수 있다.

저장 ontology와 prompt slot은 분리해야 한다.

### 5.7 Patch만으로는 향후 재컴파일이 어렵다

원장에 최종 patch만 남기면 compiler 규칙이나 ontology가 개선되었을 때 원문에서 새 IR을 재생성하거나 과거 후보가 왜 거부되었는지 비교하기 어렵다.

다음 산출물을 함께 보존해야 한다.

- source envelope
- raw model output
- parsed perception candidates
- bound/validated IR
- compiler diagnostics
- lowered patch
- compiler/model/schema/ontology version

### 5.8 JSON Schema 성공이 의미 성공으로 오인될 수 있다

JSONSchemaBench는 constrained decoding의 schema coverage, 효율, 출력 품질을 별도 차원으로 평가해야 함을 보여준다. 더 최근의 transaction-compiler 연구도 schema-valid JSON이 의미적으로 안전하거나 충실하다는 보장은 없음을 지적한다.

Mnemosyne는 parse success가 아니라 다음을 측정해야 한다.

- unsupported state mutation acceptance rate
- evidence mismatch rate
- speaker/perceiver inversion rate
- world fact / belief confusion rate
- branch replay divergence
- durable fact omission rate

## 6. Cognee GitHub 분석

분석 기준: `topoteretes/cognee` main commit `88aa09b4e3289e3dbf12c0c090080920816e2fb7`

### 6.1 실제 목표

Cognee의 핵심 목표는 자연어와 문서를 graph/vector/relational representation으로 변환하는 범용 memory ETL이다.

기본 cognify task는 다음과 같다.

```text
classify_documents
  → extract_chunks_from_documents
  → extract_graph_and_summarize
  → add_data_points
  → extract_dlt_fk_edges
```

Temporal cognify는:

```text
classify
  → chunk
  → extract_events_and_timestamps
  → extract_knowledge_graph_from_events
  → add_data_points
```

### 6.2 LLM → code 방식

- Pydantic `graph_model`을 response model로 사용한다.
- Instructor, native structured output, BAML 등의 provider adapter를 통해 typed output을 얻는다.
- chunk별 LLM graph를 ontology resolver와 기존 graph에 통합한다.
- DataPoint model에서 graph node/edge를 재귀적으로 추출한다.
- `metadata.index_fields`에 지정된 필드만 vector index한다.
- deterministic identity field와 UUID5 기반 ID를 지원한다.
- graph/vector write 전에 node와 edge를 deduplicate한다.
- dataset/data/pipeline provenance를 graph 또는 relational ledger에 기록한다.
- pipeline rollback/recovery 경로가 있다.

### 6.3 배울 점

- 작은 Task를 연결하는 pipeline 구조
- typed intermediate DataPoint
- embeddable field를 명시적으로 선택하는 방식
- deterministic identity
- provenance를 파이프라인 전체에 자동 stamping
- graph/vector/relational store를 하나의 projection pipeline으로 다루는 방식
- background execution, batch, cache, rollback
- ontology를 extraction 뒤 canonicalization 단계에 적용하는 구조

### 6.4 그대로 따르지 말아야 할 점

Cognee graph extraction의 주된 질문은 “텍스트에서 어떤 entity와 relation을 뽑을까”다. Mnemosyne의 주된 질문은 “이 서사에서 어떤 상태 전이가 정당한가”다.

Cognee는 범용 문서에서 추출된 관계를 graph fact로 저장하기 위해 설계됐다. Mnemosyne에 그대로 적용하면 다음 구분이 약해진다.

- Narrator가 실제로 서술한 사건
- 사용자가 주장한 것
- Soul이 직접 지각한 것
- Soul이 추론한 것
- 엔진이 보장하는 사실

따라서 Cognee는 compiler core가 아니라 memory projection/indexing architecture의 참고 자료로 사용해야 한다.

## 7. 더 적합한 최신 발전 레퍼런스

### 7.1 Hindsight: epistemic network 분리

Hindsight는 장기 기억을 world fact, agent experience, synthesized observation, evolving belief/opinion 계층으로 구분한다. 최신 오픈소스 retain 코드는 실제 저장 fact type을 world/experience로 나누고, observation을 source memories에 근거한 파생 지식으로 비동기 공고화한다.

주요 교훈:

- raw fact와 consolidated observation을 분리
- observation이 source memory ID와 proof count를 보유
- source 삭제 시 파생 observation을 무효화하고 나머지 source를 재공고화
- 새 사실이 반박하면 raw history를 보존하면서 observation을 갱신
- stale observation은 raw fact로 재검증
- recall은 검색이고 reflect는 별도 추론 작업

이는 Mnemosyne의 객관 사건 / 경험 / 믿음 / 스키마 분리에 Cognee보다 직접적으로 맞는다.

단, Hindsight도 LLM fact extraction 자체는 상당히 넓은 prompt와 Pydantic schema에 의존한다. 저장 시스템으로서의 교훈은 크지만 Mnemosyne의 정본 compiler보다 write authority가 넓다.

### 7.2 Graphiti/Zep: bi-temporal episode/fact

Graphiti는 raw episode를 provenance 원천으로 보존하고, 파생 fact edge에 두 종류의 시간을 둔다.

- transaction time: 시스템이 언제 알게 되었는가
- valid time: 세계에서 언제부터 언제까지 유효했는가

Mnemosyne에는 이를 다음처럼 번역하는 것이 적합하다.

- recorded_at_turn / recorded_at_patch
- valid_from_turn / valid_to_turn
- source_episode_id
- source_branch_id
- invalidated_by_patch_id

현재 branch ledger가 이미 있으므로 별도 temporal graph를 정본으로 만들 필요가 없다. ledger에서 temporal memory projection을 생성하면 된다.

### 7.3 Generative Agents: reflection threshold

Generative Agents는 경험의 전체 stream을 보존하고, 중요도가 누적되어 threshold를 넘을 때 고차 reflection을 만든다. observation, planning, reflection 각각이 believable behavior에 기여했다.

Mnemosyne에서는 매 턴 모든 경험을 schema로 승격하지 않고:

- 새 경험의 salience 누적
- 관계/주제별 unresolved evidence 누적
- consolidation threshold 도달
- background reflection
- source-backed schema/belief 생성

으로 구현할 수 있다.

### 7.4 A-MEM: atomic notes와 dynamic linking

A-MEM은 새 기억을 atomic note로 만들고 기존 기억과 동적으로 연결하며, 새 정보가 과거 memory representation을 발전시키도록 한다.

Mnemosyne에서는 raw memory를 직접 고쳐서는 안 된다. 대신:

- immutable episode memory
- mutable derived interpretation
- supersedes/supports/contradicts edge

를 분리하여 A-MEM의 evolution 장점을 취해야 한다.

### 7.5 HippoRAG 2: graph는 recall index

HippoRAG 2는 passage와 KG를 함께 보존하고 query에서 seed node를 찾아 Personalized PageRank로 연관 기억을 확장한다. 중요한 교훈은 graph를 truth writer가 아니라 associative retriever로 사용하는 것이다.

Mnemosyne도:

- ledger/episode가 정본
- memory graph는 projection
- vector/BM25가 seed 후보 생성
- graph propagation이 연상 후보 확장
- Context Compiler가 최종 budget과 truth filter 적용

이어야 한다.

## 8. 권장 차세대 Compiler IR

### 8.1 Stage 0: SourceEnvelope

LLM에 맡기지 않고 코드가 만든다.

```rust
struct SourceEnvelope {
    conversation_id: String,
    branch_id: String,
    turn_id: String,
    user_message_id: i64,
    assistant_message_id: i64,
    assistant_variant_id: Option<i64>,
    active_soul_ids: Vec<String>,
    player_entity_id: String,
    user_text: String,
    narrator_text: String,
    parent_state_hash: String,
    recorded_at: i64,
}
```

### 8.2 Stage 1: PerceptionIR

LLM이 생성할 수 있는 유일한 계층이다. 아직 상태 변경 명령이 아니다.

```rust
struct PerceptionCandidate {
    candidate_id: String,
    kind: PerceptionKind,
    subject: EntityMention,
    predicate: PerceptionPredicate,
    object: Option<TypedValue>,
    actor: Option<EntityMention>,
    perceiver: Option<EntityMention>,
    targets: Vec<EntityMention>,
    evidence: EvidenceSpan,
    epistemic_mode: EpistemicMode,
    extraction_confidence: f32,
    temporal_expression: Option<String>,
    durability_hint: DurabilityHint,
}

enum PerceptionKind {
    Event,
    Utterance,
    ObjectObservation,
    AffectCue,
    RelationshipEvidence,
    Intention,
    BeliefExpression,
    Correction,
}

enum EpistemicMode {
    DirectlyObserved,
    StatedBy,
    NarratorDescribed,
    Inferred,
    RememberedBy,
}
```

`EvidenceSpan`은 quote와 함께 role/start/end를 받되, message ID는 SourceEnvelope에서 주입한다. 가능하면 모델이 quote 문자열을 복사하는 대신 허용된 line/span ID를 선택하도록 한다.

### 8.3 Stage 2: Binding

순수 Rust 단계:

- alias → stable entity ID
- pronoun/coreference resolution 검증
- speaker, actor, perceiver 권한 확인
- temporal expression → turn/time range
- quote/span 원문 일치
- duplicate candidate canonicalization

결과는 `BoundPerception`.

### 8.4 Stage 3: Semantic analysis

도메인 규칙으로 epistemic status를 결정한다.

예:

```text
사용자: "나는 문을 잠갔어"
  → UserTestimony(actor=player, proposition=door locked)
  → world fact가 아니라 user_claimed candidate

Narrator: "그가 문을 잠그고 열쇠를 주머니에 넣었다."
  → SceneEvent(actor=player)
  → ObjectObservation(door=locked)

Narrator: "Aurora는 그가 거짓말한다고 확신했다."
  → SoulBelief(owner=Aurora, proposition=player lied)
  → 실제 거짓말 여부는 미확정

Evaluator 추론: "Aurora가 불안해 보인다."
  → AffectHypothesis(confidence=...)
  → VerifiedEngine으로 승격 불가
```

이 단계만 다음 권한을 가진다.

- truth status 부여
- source type 부여
- objective event와 subjective memory 연결
- durable/ephemeral 결정
- contradiction 및 supersession 후보 생성

### 8.5 Stage 4: Effect lowering

검증된 event를 실제 state effect로 낮춘다.

```rust
enum StateEffect {
    AppendWorldEvent(...),
    RecordObjectObservation(...),
    FormEpisodicMemory(...),
    FormBeliefMemory(...),
    RecordIntention(...),
    ApplyRelationshipEvidence(...),
    UpdateSceneProjection(...),
}
```

LLM은 `StateEffect`를 직접 생성하지 못한다.

### 8.6 Stage 5: Transaction simulation

commit 전 cloned state에 patch를 적용한다.

- parent state hash가 현재 branch head와 일치하는가
- referenced entity/object/event가 존재하는가
- invariant가 유지되는가
- 동일 candidate를 재실행해도 idempotent한가
- impossible transition이 없는가
- patch를 serialize/replay했을 때 같은 결과가 나오는가

실패 시 전체 turn을 버리지 않고 effect별 diagnostic을 생성한다. 의미가 불명확한 후보만 targeted repair LLM에 전달한다.

### 8.7 Stage 6: Commit과 projection

동기 commit:

- source envelope
- accepted/rejected candidate IR
- compiler diagnostics
- lowered EnginePatch
- compiler/model/schema/ontology version

비동기 projection:

- episodic memory index
- temporal fact projection
- entity/causal graph
- embedding
- consolidation queue

projection은 언제든 ledger에서 재구축 가능해야 한다.

## 9. 관계 시스템에 대한 구체적 변경

현재 numeric relationship event는 좋은 방향이지만 LLM의 출력 부담이 크다. 12개 axis와 여러 modifier를 한 번에 정확히 매기는 것은 작은 모델에서 조용히 실패하기 쉽다.

권장 입력:

```rust
RelationshipEvidenceCandidate {
    actor,
    target,
    perceived_by,
    behavior: BoundaryRespected | BoundaryViolated | HonestDisclosure |
              DeceptionSignal | ReliableHelp | Abandonment | RepairAttempt | ...,
    valence: -2..=2,
    directness: 0..=3,
    stakes: 0..=3,
    costliness: 0..=3,
    repetition_hint: New | Repeated,
    evidence_span,
    extraction_confidence,
}
```

Rust의 관계 calculator가 behavior를 내부 axis vector로 펼치고 현재 관계, attachment, trauma, first impression, saturation을 반영해 최종 delta를 계산한다.

장점:

- 모델에게 심리 수학을 맡기지 않는다.
- 모델별 numeric calibration 차이가 줄어든다.
- 관계 이론을 코드와 테스트로 버전 관리할 수 있다.
- 같은 사건을 새 calculator 버전으로 shadow replay할 수 있다.

## 10. 기억 모델 V2

저장 종류와 retrieval lens를 분리한다.

### 10.1 원본 기억

- `Episode`: 실제 turn/scene에 묶인 경험
- `Testimony`: 누군가 말한 주장
- `Perception`: Soul이 직접 지각한 것
- `Affect`: 감정 및 신체 반응
- `Intention`: 약속, 계획, 미완료 목표

### 10.2 파생 기억

- `Belief`: Soul이 참이라고 여기는 proposition
- `Schema`: 반복 경험에서 공고화된 패턴
- `RelationshipModel`: 특정 인물에 대한 누적 해석
- `SelfModel`: 자신에 대한 지속적 이해
- `Reflection`: 여러 기억을 종합한 고차 해석

파생 기억은 반드시:

- source memory IDs
- supporting/contradicting evidence
- compiler/consolidator version
- confidence
- valid interval
- stale status

를 가진다.

## 11. LLM 호출 전략

### 동기 hot path

한 번의 structured extraction:

```text
turn text → PerceptionIR candidates
```

그 뒤는 Rust binding/semantic analysis/lowering/simulation이다.

### 조건부 repair

전체 응답을 재생성하지 않는다. 거부된 candidate와 정확한 validator error, 허용된 entity/span 목록만 보내 해당 후보만 고친다.

### 비동기 consolidation

매 턴이 아니라 다음 조건 중 하나에서 실행한다.

- `turns_since_consolidation` threshold
- topic/scene 종료
- 특정 관계 evidence 누적
- salience 누적 threshold
- app idle
- 수동 savepoint

### retrieval

기본 회상에는 LLM을 사용하지 않는다.

```text
exact identity/BM25 + vector seeds
  → temporal/truth/branch filter
  → graph propagation/PPR
  → deterministic rerank
  → Context Compiler
```

복잡한 “왜 그런가”, “어떤 패턴인가” 질문이나 Narrator의 고차 자기성찰에서만 reflect LLM을 호출한다.

## 12. 검증과 벤치마크

### 12.1 Golden turn corpus

다음 유형을 포함한 고정 turn 세트를 만든다.

- 행동과 발언의 구분
- 거짓말과 믿음
- 간접 목격
- 대명사와 동명이인
- 부정문
- 조건문과 가정
- 회상 장면
- retcon
- object 이동 및 소유권
- 관계 repair/betrayal
- 사용자의 메타 명령

각 turn에 expected PerceptionIR, accepted effects, rejected effects를 기록한다.

### 12.2 Metamorphic tests

의미는 같고 표현만 바꾼 paraphrase에서 같은 effect가 나와야 한다.

의미를 바꾸는 최소 변형에서는 effect가 바뀌어야 한다.

- actor swap
- negation insertion
- tense change
- “말했다” ↔ “행동했다”
- perceiver change
- direct experience ↔ hearsay

### 12.3 Replay tests

- 동일 ledger + 동일 compiler version → 동일 state hash
- projection 삭제 후 rebuild → 동일 graph/index contents
- branch switch → inactive branch derived memory 제외
- evaluator enrichment 재시도 → 중복 effect 없음

### 12.4 품질 지표

- candidate precision/recall
- durable event omission rate
- unsafe effect acceptance rate
- evidence grounding accuracy
- entity binding accuracy
- epistemic classification accuracy
- relationship delta calibration
- state replay divergence
- repair recovery rate
- p50/p95 latency
- turn당 token/cost

JSON parse rate는 보조 지표일 뿐 최종 품질 지표가 아니다.

## 13. 단계별 이행안

### Phase A — 현재 compiler 권한 정리

1. LLM schema에서 engine-only truth status 제거
2. source message/turn/branch ID를 code injection으로 변경
3. 모든 durable op에 evidence span contract 적용
4. compiler/model/schema version을 ledger trace에 저장
5. unsafe acceptance benchmark 추가

### Phase B — PerceptionIR 도입

1. `evaluator_structured_v2`를 effect op가 아닌 perception candidate schema로 생성
2. v1과 v2를 shadow mode로 동시에 실행
3. v2 lowering 결과를 commit하지 않고 v1 patch와 비교
4. golden corpus에서 precision/recall과 divergence 측정

### Phase C — Semantic lowering 분리

1. entity binder
2. evidence span validator
3. epistemic classifier rules
4. temporal anchor resolver
5. state effect planner
6. transaction simulator

### Phase D — Memory V2 projection

1. immutable episode/testimony/perception 저장
2. source-backed belief/schema projection
3. invalidation/rebuild
4. embedding 및 entity/causal graph

### Phase E — Hybrid recall

1. BM25/exact + vector seed
2. temporal/truth/branch filter
3. graph expansion 또는 PPR
4. Context Compiler용 `MemoryEvidenceBundle`
5. retrieval trace와 benchmark

### Phase F — Consolidation

1. threshold 기반 background reflection
2. source proof와 contradiction tracking
3. stale derived memory 재검증
4. schema/relationship/self model 공고화

## 14. 최종 선택

Mnemosyne가 취해야 할 것은 “LLM이 코드를 생성하여 실행하는 시스템”이 아니다.

정확한 표현은:

> LLM은 자연어를 증거 기반의 제한된 지각 IR로 파싱한다.  
> 결정론적 Rust compiler가 그 IR을 검증 가능한 상태 전이로 변환한다.  
> 원장이 정본을 보존하고, 기억·그래프·벡터는 재구축 가능한 projection이 된다.

이 구조라면 LLM이 강한 부분인 언어 이해와 미묘한 인간 행동 해석을 활용하면서, 약한 부분인 권한 판단, 수치 일관성, identity, temporal validity, branch consistency를 코드가 담당한다.

## 15. 주요 자료

### 분석한 코드

- Mnemosyne `src-tauri/state_engine/src/evaluator_structured.rs`
- Mnemosyne `src-tauri/state_engine/src/evaluator_form/*`
- Mnemosyne `src-tauri/state_engine/src/patch.rs`
- Mnemosyne `src-tauri/state_engine/src/context_compiler.rs`
- Mnemosyne `src-tauri/src/commands.rs`
- Mnemosyne `src-tauri/src/db/mod.rs`
- [Cognee GitHub](https://github.com/topoteretes/cognee)
- [Hindsight GitHub](https://github.com/vectorize-io/hindsight)
- [Graphiti GitHub](https://github.com/getzep/graphiti)

### 논문 및 공식 문서

- [Cognee Architecture](https://docs.cognee.ai/core-concepts/architecture)
- [Cognee Custom Data Models](https://docs.cognee.ai/guides/custom-data-models)
- [Hindsight: Structured Agent Memory that Retains, Recalls, and Reflects](https://aclanthology.org/2026.acl-demo.27/)
- [Hindsight is 20/20](https://arxiv.org/abs/2512.12818)
- [Zep: A Temporal Knowledge Graph Architecture for Agent Memory](https://arxiv.org/abs/2501.13956)
- [Generative Agents: Interactive Simulacra of Human Behavior](https://arxiv.org/abs/2304.03442)
- [A-MEM: Agentic Memory for LLM Agents](https://arxiv.org/abs/2502.12110)
- [From RAG to Memory: HippoRAG 2](https://arxiv.org/abs/2502.14802)
- [Mem0: Production-Ready Long-Term Memory](https://arxiv.org/abs/2504.19413)
- [JSONSchemaBench](https://arxiv.org/abs/2501.10868)

