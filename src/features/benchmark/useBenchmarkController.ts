import { useRef, useState } from "react";

import type {
  ApiProviderSettings,
  BenchmarkSettings,
  BenchmarkSummary,
  BenchmarkTarget,
  BenchmarkTurnSummary,
  BenchmarkType,
  StructuredEvaluatorDiagnosticSummary,
} from "../../tauri";

export type BenchmarkTurnPhase =
  | "player_generation"
  | "execute_turn"
  | "evaluator_wait"
  | "turn_summary"
  | "completed";

export type BenchmarkLiveContext = {
  benchmarkId: string;
  conversationId: string;
  soulId: string;
  startedAt: number;
  playerProfileId: string;
  playerGoal: string;
  traditionalOpponent: boolean;
  settings: BenchmarkSettings;
  narratorSettings: ApiProviderSettings;
  updaterSettings: ApiProviderSettings;
  initialMemoryCount: number;
  initialObjectCount: number;
  initialRelationshipCount: number;
  relationshipTargetChecked: string;
  initialActivePlayerRelationship: Record<string, unknown> | null;
  perTurn: BenchmarkTurnSummary[];
  narratorFailures: number;
  completedTurns: number;
  nextTurnIndex: number;
  lastPlayerText: string;
};

export function useBenchmarkController() {
  const liveBenchmarkJobIdRef = useRef<string | null>(null);
  const [structuredDiagnosticRunning, setStructuredDiagnosticRunning] = useState(false);
  const [structuredDiagnosticResult, setStructuredDiagnosticResult] =
    useState<StructuredEvaluatorDiagnosticSummary | null>(null);
  const [structuredDiagnosticError, setStructuredDiagnosticError] = useState<string | null>(
    null,
  );
  const [benchmarkType, setBenchmarkType] = useState<BenchmarkType>("visible_ai_chat");
  const [benchmarkTarget, setBenchmarkTarget] =
    useState<BenchmarkTarget>("current_session");
  const [benchmarkTurnCount, setBenchmarkTurnCount] = useState(5);
  const [benchmarkPlayerProfileId, setBenchmarkPlayerProfileId] = useState("");
  const [benchmarkPlayerGoal, setBenchmarkPlayerGoal] = useState(
    "Build cautious trust with the active Soul while respecting boundaries.",
  );
  const [benchmarkStrictToolEvaluator, setBenchmarkStrictToolEvaluator] = useState(false);
  const [benchmarkTransport, setBenchmarkTransport] =
    useState<ApiProviderSettings["structured_evaluator_transport"]>("tool_call");
  const [benchmarkWaitForEvaluator, setBenchmarkWaitForEvaluator] = useState(true);
  const [benchmarkTraditionalOpponent, setBenchmarkTraditionalOpponent] = useState(false);
  const [benchmarkRunning, setBenchmarkRunning] = useState(false);
  const [benchmarkResult, setBenchmarkResult] = useState<BenchmarkSummary | null>(null);
  const [benchmarkError, setBenchmarkError] = useState<string | null>(null);
  const [benchmarkLiveActive, setBenchmarkLiveActive] = useState(false);
  const [benchmarkTurnsRemaining, setBenchmarkTurnsRemaining] = useState(0);
  const [benchmarkLivePhase, setBenchmarkLivePhase] = useState<
    BenchmarkTurnPhase | "idle" | "preparing" | "finalizing" | "stopping" | "failed"
  >("idle");
  const benchmarkCtxRef = useRef<BenchmarkLiveContext | null>(null);
  const benchmarkTurnInFlightRef = useRef(false);
  const benchmarkStopRef = useRef(false);

  return {
    liveBenchmarkJobIdRef,
    structuredDiagnosticRunning,
    setStructuredDiagnosticRunning,
    structuredDiagnosticResult,
    setStructuredDiagnosticResult,
    structuredDiagnosticError,
    setStructuredDiagnosticError,
    benchmarkType,
    setBenchmarkType,
    benchmarkTarget,
    setBenchmarkTarget,
    benchmarkTurnCount,
    setBenchmarkTurnCount,
    benchmarkPlayerProfileId,
    setBenchmarkPlayerProfileId,
    benchmarkPlayerGoal,
    setBenchmarkPlayerGoal,
    benchmarkStrictToolEvaluator,
    setBenchmarkStrictToolEvaluator,
    benchmarkTransport,
    setBenchmarkTransport,
    benchmarkWaitForEvaluator,
    setBenchmarkWaitForEvaluator,
    benchmarkTraditionalOpponent,
    setBenchmarkTraditionalOpponent,
    benchmarkRunning,
    setBenchmarkRunning,
    benchmarkResult,
    setBenchmarkResult,
    benchmarkError,
    setBenchmarkError,
    benchmarkLiveActive,
    setBenchmarkLiveActive,
    benchmarkTurnsRemaining,
    setBenchmarkTurnsRemaining,
    benchmarkLivePhase,
    setBenchmarkLivePhase,
    benchmarkCtxRef,
    benchmarkTurnInFlightRef,
    benchmarkStopRef,
  };
}
