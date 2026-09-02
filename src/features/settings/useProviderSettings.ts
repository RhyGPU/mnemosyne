import { useMemo, useState } from "react";

import type { ApiProviderSettings } from "../../tauri";
import {
  EVALUATOR_EXECUTION_MODE_STORAGE_KEY,
  NARRATOR_PROVIDER_PROFILE_STORAGE_KEY,
  REPAIR_PROVIDER_PROFILE_STORAGE_KEY,
  STRUCTURED_EVALUATOR_TRANSPORT_STORAGE_KEY,
  UPDATER_PROVIDER_PROFILE_STORAGE_KEY,
  USE_NARRATOR_FOR_UPDATER_STORAGE_KEY,
  loadStoredCustomNarratorPrompt,
  loadStoredGenerationPreferences,
  loadStoredReadingPreferences,
} from "../../app/preferencesStorage";
import type {
  GenerationPreferences,
  ReadingPreferences,
} from "../../settings/preferences";

function defaultProviderSettings(systemPrompt = ""): ApiProviderSettings {
  return {
    base_url: "https://api.openai.com/v1",
    api_key: "",
    model: "",
    system_prompt: systemPrompt,
    narrator_timeout_ms: null,
    evaluator_timeout_ms: 25_000,
    structured_evaluator_timeout_ms: 90_000,
    diagnostic_evaluator_timeout_ms: 60_000,
    evaluator_timeout_mode: "finite",
    evaluator_mode: "evaluator_form_v1",
    structured_evaluator_policy: "prefer",
    structured_evaluator_max_retries: 1,
    wait_for_evaluator_before_next_turn: true,
    allow_send_with_stale_state: false,
    evaluator_background_enabled: false,
    anti_replay_forced_retry_enabled: false,
  };
}

export function useProviderSettings() {
  const [narratorProviderProfileName, setNarratorProviderProfileName] =
    useState("Narrator API");
  const [updaterProviderProfileName, setUpdaterProviderProfileName] =
    useState("Updater API");
  const [selectedProviderProfileId, setSelectedProviderProfileId] = useState(
    () => localStorage.getItem(NARRATOR_PROVIDER_PROFILE_STORAGE_KEY) ?? "",
  );
  const [selectedStateUpdaterProfileId, setSelectedStateUpdaterProfileId] = useState(
    () => localStorage.getItem(UPDATER_PROVIDER_PROFILE_STORAGE_KEY) ?? "",
  );
  const [selectedRepairProfileId, setSelectedRepairProfileId] = useState(
    () => localStorage.getItem(REPAIR_PROVIDER_PROFILE_STORAGE_KEY) ?? "",
  );
  const [useNarratorProviderForUpdater, setUseNarratorProviderForUpdater] = useState(
    () => localStorage.getItem(USE_NARRATOR_FOR_UPDATER_STORAGE_KEY) !== "false",
  );
  const [devOverrideActive, setDevOverrideActive] = useState(false);
  const [apiSettings, setApiSettings] = useState<ApiProviderSettings>(() =>
    defaultProviderSettings(loadStoredCustomNarratorPrompt()),
  );
  const [stateUpdaterSettings, setStateUpdaterSettings] =
    useState<ApiProviderSettings>(() => defaultProviderSettings());
  const [generationPreferences, setGenerationPreferences] =
    useState<GenerationPreferences>(loadStoredGenerationPreferences);
  const [readingPreferences, setReadingPreferences] =
    useState<ReadingPreferences>(loadStoredReadingPreferences);
  const effectiveNarratorSettings = useMemo<ApiProviderSettings>(
    () => ({
      ...apiSettings,
      narrator_temperature: generationPreferences.temperature,
      narrator_max_tokens: generationPreferences.maxTokens,
      context_max_tokens: generationPreferences.contextMaxTokens,
      narrator_top_p: generationPreferences.topP,
      narrator_frequency_penalty: generationPreferences.frequencyPenalty,
      narrator_presence_penalty: generationPreferences.presencePenalty,
    }),
    [apiSettings, generationPreferences],
  );
  const [evaluatorExecutionMode, setEvaluatorExecutionMode] = useState(
    () => localStorage.getItem(EVALUATOR_EXECUTION_MODE_STORAGE_KEY) ?? "balanced",
  );
  const [structuredEvaluatorTransport, setStructuredEvaluatorTransport] = useState(
    () => localStorage.getItem(STRUCTURED_EVALUATOR_TRANSPORT_STORAGE_KEY) ?? "auto",
  );

  function updateEvaluatorExecutionMode(mode: string) {
    setEvaluatorExecutionMode(mode);
    localStorage.setItem(EVALUATOR_EXECUTION_MODE_STORAGE_KEY, mode);
  }

  function updateStructuredEvaluatorTransport(transport: string) {
    setStructuredEvaluatorTransport(transport);
    localStorage.setItem(STRUCTURED_EVALUATOR_TRANSPORT_STORAGE_KEY, transport);
  }

  return {
    narratorProviderProfileName,
    setNarratorProviderProfileName,
    updaterProviderProfileName,
    setUpdaterProviderProfileName,
    selectedProviderProfileId,
    setSelectedProviderProfileId,
    selectedStateUpdaterProfileId,
    setSelectedStateUpdaterProfileId,
    selectedRepairProfileId,
    setSelectedRepairProfileId,
    useNarratorProviderForUpdater,
    setUseNarratorProviderForUpdater,
    devOverrideActive,
    setDevOverrideActive,
    apiSettings,
    setApiSettings,
    stateUpdaterSettings,
    setStateUpdaterSettings,
    generationPreferences,
    setGenerationPreferences,
    readingPreferences,
    setReadingPreferences,
    effectiveNarratorSettings,
    evaluatorExecutionMode,
    updateEvaluatorExecutionMode,
    structuredEvaluatorTransport,
    updateStructuredEvaluatorTransport,
  };
}
