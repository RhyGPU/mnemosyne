import { useCallback, useEffect, useMemo, useState } from "react";

import {
  applyRelationshipStage,
  listCharacterKnowledge,
  markCharactersMet,
  RELATIONSHIP_STAGES,
  setCharacterKnowledge,
  type KnowledgeCell,
} from "../../tauri";

/** The statuses a person can put on a fact by hand. */
const STATUSES = ["knows", "suspects", "believes_false", "unaware", "hiding"] as const;

const STATUS_LABEL: Record<string, string> = {
  knows: "Knows",
  suspects: "Suspects",
  believes_false: "Believes wrongly",
  unaware: "Does not know",
  hiding: "Hiding it",
};

type Props = {
  conversationId: string | null;
  busy?: boolean;
};

/**
 * Who knows what about whom, as a grid.
 *
 * The engine works out most of this on its own — a name counts as known once
 * the transcript says it — but two things it cannot see: what happened before
 * this session, and what happened off-screen. This is where a person supplies
 * those, so an automatic answer that is wrong stays correctable instead of
 * becoming something to argue with the narrator about.
 */
export function KnowledgeGrid({ conversationId, busy = false }: Props) {
  const [cells, setCells] = useState<KnowledgeCell[]>([]);
  const [stage, setStage] = useState("strangers");
  const [status, setStatus] = useState<string | null>(null);
  const [working, setWorking] = useState(false);

  const reload = useCallback(async () => {
    if (!conversationId) {
      setCells([]);
      return;
    }
    try {
      setCells(await listCharacterKnowledge(conversationId));
    } catch (error) {
      setStatus(error instanceof Error ? error.message : String(error));
    }
  }, [conversationId]);

  useEffect(() => {
    void reload();
  }, [reload]);

  /** One row per fact, one column per observer. */
  const { observers, rows } = useMemo(() => {
    const observers = Array.from(new Set(cells.map((cell) => cell.holder_entity_id))).sort();
    const byProposition = new Map<string, Map<string, KnowledgeCell>>();
    for (const cell of cells) {
      let row = byProposition.get(cell.proposition);
      if (!row) {
        row = new Map();
        byProposition.set(cell.proposition, row);
      }
      row.set(cell.holder_entity_id, cell);
    }
    const rows = Array.from(byProposition.entries())
      .map(([proposition, byObserver]) => ({ proposition, byObserver }))
      .sort((a, b) => a.proposition.localeCompare(b.proposition));
    return { observers, rows };
  }, [cells]);

  const run = async (label: string, action: () => Promise<unknown>) => {
    if (!conversationId || working) return;
    setWorking(true);
    setStatus(null);
    try {
      await action();
      await reload();
      setStatus(label);
    } catch (error) {
      setStatus(error instanceof Error ? error.message : String(error));
    } finally {
      setWorking(false);
    }
  };

  const disabled = busy || working || !conversationId;

  if (!conversationId) {
    return (
      <div className="knowledge-grid">
        <p className="muted">Open a session to see who knows what.</p>
      </div>
    );
  }

  return (
    <div className="knowledge-grid">
      <div className="knowledge-grid-controls">
        <label className="field">
          <span>Starting relationship</span>
          <select value={stage} onChange={(event) => setStage(event.target.value)} disabled={disabled}>
            {RELATIONSHIP_STAGES.map((item) => (
              <option key={item.value} value={item.value}>
                {item.label}
              </option>
            ))}
          </select>
          <span className="hint">
            {RELATIONSHIP_STAGES.find((item) => item.value === stage)?.hint}
          </span>
        </label>
        <button
          type="button"
          disabled={disabled}
          onClick={() =>
            run("Relationship applied.", () => applyRelationshipStage(conversationId, stage))
          }
        >
          Apply to both sides
        </button>
      </div>

      {rows.length === 0 ? (
        <p className="muted">
          Nothing recorded yet. Apply a starting relationship, or run Seed Observable Knowledge in
          Dev Mode to catalogue what is visible.
        </p>
      ) : (
        <div className="scroller">
          <table className="knowledge-table">
            <thead>
              <tr>
                <th>Fact</th>
                {observers.map((observer) => (
                  <th key={observer}>{observer}</th>
                ))}
              </tr>
            </thead>
            <tbody>
              {rows.map((row) => (
                <tr key={row.proposition}>
                  <td className="knowledge-fact">{row.proposition}</td>
                  {observers.map((observer) => {
                    const cell = row.byObserver.get(observer);
                    return (
                      <td key={observer}>
                        <select
                          value={cell?.status ?? "unaware"}
                          disabled={disabled}
                          aria-label={`${observer}: ${row.proposition}`}
                          onChange={(event) =>
                            run("Updated.", () =>
                              setCharacterKnowledge(
                                conversationId,
                                observer,
                                row.proposition,
                                event.target.value,
                              ),
                            )
                          }
                        >
                          {STATUSES.map((value) => (
                            <option key={value} value={value}>
                              {STATUS_LABEL[value]}
                            </option>
                          ))}
                        </select>
                      </td>
                    );
                  })}
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}

      {observers.length > 0 ? (
        <div className="knowledge-grid-controls">
          <span className="hint">
            Meeting someone opens what you learn by looking, and nothing else.
          </span>
          {observers.map((observer) => (
            <button
              key={observer}
              type="button"
              disabled={disabled}
              onClick={() => {
                // The subject is whatever the other rows are about, which is the
                // fact prefix: "<subject>'s name".
                const subject = rows
                  .map((row) => row.proposition.split("'s ")[0])
                  .find((candidate) => candidate.length > 0);
                if (!subject) return;
                void run("Meeting recorded.", () =>
                  markCharactersMet(conversationId, observer, subject),
                );
              }}
            >
              {observer} has now met them
            </button>
          ))}
        </div>
      ) : null}

      {status ? <p className="muted">{status}</p> : null}
    </div>
  );
}
