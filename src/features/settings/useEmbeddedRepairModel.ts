import { useEffect, useRef, useState } from "react";

import {
  embeddedRepairModelStatus,
  startEmbeddedRepairModel,
  stopEmbeddedRepairModel,
  type EmbeddedModelStatus,
} from "../../tauri";
import { EMBEDDED_MODEL_PATH_STORAGE_KEY } from "../../app/preferencesStorage";

type ProviderAlert = {
  title: string;
  detail?: string;
  hint?: string;
};

type EmbeddedRepairModelOptions = {
  onStatus: (message: string) => void;
  onAlert: (alert: ProviderAlert) => void;
};

const STOPPED_MODEL: EmbeddedModelStatus = {
  running: false,
  ready: false,
  url: null,
  model: null,
};

export function useEmbeddedRepairModel({
  onStatus,
  onAlert,
}: EmbeddedRepairModelOptions) {
  const [path, setPath] = useState(
    () => localStorage.getItem(EMBEDDED_MODEL_PATH_STORAGE_KEY) ?? "",
  );
  const [model, setModel] = useState<EmbeddedModelStatus>(STOPPED_MODEL);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const pathRef = useRef(path);
  const readyPromiseRef = useRef<Promise<boolean> | null>(null);

  useEffect(() => {
    pathRef.current = path;
  }, [path]);

  useEffect(() => {
    let active = true;
    let timer: number | undefined;
    const tick = async () => {
      try {
        const status = await embeddedRepairModelStatus();
        if (!active) return;
        setModel(status);
        if (status.running && !status.ready) {
          timer = window.setTimeout(() => void tick(), 2_000);
        }
      } catch {
        // Keep the last known status while the local process is unavailable.
      }
    };
    void tick();
    return () => {
      active = false;
      if (timer !== undefined) window.clearTimeout(timer);
    };
  }, [model.running]);

  async function start() {
    if (!path.trim()) {
      setError("Set the path to your llamafile first.");
      return;
    }
    setBusy(true);
    setError(null);
    try {
      const status = await startEmbeddedRepairModel(path.trim(), 8080, null);
      setModel(status);
      onStatus("Embedded repair model starting...");
    } catch (caught) {
      const detail = caught instanceof Error ? caught.message : String(caught);
      setError(detail);
      onAlert({
        title: "Local repair model failed to start",
        detail,
        hint: "Check the llamafile path in Settings, or that the file is runnable.",
      });
    } finally {
      setBusy(false);
    }
  }

  async function stop() {
    setBusy(true);
    try {
      await stopEmbeddedRepairModel();
      setModel(STOPPED_MODEL);
    } catch (caught) {
      setError(caught instanceof Error ? caught.message : String(caught));
    } finally {
      setBusy(false);
    }
  }

  function ensureReady(timeoutMs = 90_000): Promise<boolean> {
    if (!readyPromiseRef.current) {
      readyPromiseRef.current = (async () => {
        let status: EmbeddedModelStatus | null = null;
        try {
          status = await embeddedRepairModelStatus();
        } catch {
          status = null;
        }
        if (status?.ready) return true;

        if (!status?.running) {
          const configuredPath = pathRef.current.trim();
          if (!configuredPath) return false;
          try {
            await startEmbeddedRepairModel(configuredPath, 8080, null);
            onStatus("Starting local repair model...");
          } catch (caught) {
            setError(caught instanceof Error ? caught.message : String(caught));
            return false;
          }
        }

        const deadline = Date.now() + timeoutMs;
        while (Date.now() < deadline) {
          await new Promise((resolve) => setTimeout(resolve, 2_000));
          try {
            const next = await embeddedRepairModelStatus();
            setModel(next);
            if (next.ready) return true;
            if (!next.running) return false;
          } catch {
            // Keep polling until the caller's deadline.
          }
        }
        return false;
      })().finally(() => {
        readyPromiseRef.current = null;
      });
    }
    return readyPromiseRef.current;
  }

  return {
    path,
    setPath,
    model,
    busy,
    error,
    setError,
    start,
    stop,
    ensureReady,
  };
}
