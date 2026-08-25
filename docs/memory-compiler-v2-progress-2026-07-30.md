# Memory Compiler V2 구현 진행 기록

## 완료

- M0: golden corpus와 replay 기준선
- M1: V1 authority/source/evidence 경계 강화
- M2: Rust compiler 계약과 provenance
- M3: strict Perception IR V2, 무커밋 shadow 실행, compiler 진단 저장
- M4: entity binding, semantic validation, effect lowering, transaction simulation,
  V1 `EnginePatch` compatibility adapter
- M5: `evaluator_perception_v2` opt-in production 경로와 Form V1 fallback
- M6: raw/derived Memory V2 계약, SQLite projection, ledger rebuild,
  replay 동등성, derived stale 처리
- M7: topical evidence consolidation, contradiction retention, deterministic belief/schema/
  relationship/self/reflection policies, trigger/run trace
- M8: FTS5 + filters + optional semantic adapter + temporal score + evidence graph recall,
  `MemoryEvidenceBundle` Context Compiler 연결
- M9: State Map evidence inspector, append-only correction event, Hindsight/Cognee ADR

## 현재

- 전체 회귀 및 실앱 최종 검증

## 검증 기준

- V2는 LLM이 만든 state delta를 직접 commit하지 않는다.
- engine-owned source identity가 없으면 production V2 compile을 거부한다.
- simulation이 commit-ready이고 V1 adapter가 모든 effect를 지원할 때만 commit한다.
- V2 transport/parse/semantic/lowering 실패는 이유를 남기고 Form V1으로 fallback한다.
- Memory V2 raw projection은 ledger 활성 경로만으로 재생성한다.
- branch/retcon으로 근거가 사라지면 derived memory는 삭제하지 않고 stale 처리한다.
- unrelated raw memory는 같은 episodic kind라는 이유만으로 합치지 않는다.
- recall graph neighbor에도 동일한 truth/type/character/time filter를 적용한다.
- semantic/vector backend가 없어도 FTS5 + temporal + graph recall이 정상 동작한다.
- correction은 projection 직접 수정 대신 append-only event와 replacement turn으로 남긴다.

## 최근 회귀 결과

- `state_engine`: 340 passed (unit 321 + compiler contract 17 + golden/integration 2)
- Tauri app: 427 passed, 1 ignored (explicit live local-model benchmark)
- frontend: TypeScript + production Vite build passed
- frontend characterization: slash commands + model tests passed
- Memory V2 consolidation/recall/filters/semantic-fallback/token benchmark: passed
- correction event + targeted V2 repair: passed
- 실앱: Home, Settings, Play, State Map 및 Memory V2 Evidence Map 렌더링 확인
- `git diff --check`: passed
- Clippy: 현재 Rust toolchain에 `cargo-clippy` component가 없어 실행하지 못함

## 조건부 유지

- V1 evaluator 제거와 V2 기본값 전환은 계획 자체의 live benchmark/fallback gate가
  충족된 뒤 수행한다. 현재 저장된 narrator profile의 upstream model이 404를 반환하므로
  장시간 live RP 통과를 주장하지 않는다.
- 결정과 비교 범위: `memory-system-v2-decisions-2026-07-30.md`
