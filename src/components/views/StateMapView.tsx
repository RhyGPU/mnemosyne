import type { ReactNode } from "react";
import { Play, RefreshCcw } from "lucide-react";
import type { SessionStateMap } from "../../tauri";
import { KnowledgeGrid } from "./KnowledgeGrid";
import {
  buildStateMapPresentation,
  humanizeStateMapText,
} from "../../features/state-map/model/stateMapPresentation";

export function StateMapView({
  appDialogNode,
  children,
  railNav,
  railShellClass,
}: {
  appDialogNode: ReactNode;
  children: ReactNode;
  railNav: ReactNode;
  railShellClass: string;
}) {
  return (
    <main className={`app-shell statemap-shell${railShellClass}`}>
      {appDialogNode}
      {railNav}
      {children}
    </main>
  );
}

export function StateMapDashboard({
  busy,
  conversationId,
  onBackToPlay,
  onRefresh,
  stateMap,
}: {
  busy: boolean;
  conversationId: string | null;
  onBackToPlay: () => void;
  onRefresh: () => void;
  stateMap: SessionStateMap | null;
}) {
  const featuredMemory =
    stateMap?.memories.find((memory) => memory.is_pinned) ??
    stateMap?.memories.find((memory) => memory.is_active) ??
    stateMap?.memories[0] ??
    null;
  const latestScene = stateMap?.scenes[0] ?? null;
  const presentation = stateMap ? buildStateMapPresentation(stateMap) : null;
  const relKeys: Array<[string, "trust" | "affection" | "intimacy" | "fear" | "desire"]> = [
    ["Trust", "trust"],
    ["Affection", "affection"],
    ["Intimacy", "intimacy"],
    ["Fear", "fear"],
    ["Desire", "desire"],
  ];

  return (
    <>
      <header className="launcher-header">
        <div>
          <span className="eyebrow">State Map / recent sessions</span>
          <h1>What the engine believes is true</h1>
        </div>
        <div className="launcher-actions">
          <button type="button" className="ghost-action" onClick={onRefresh} disabled={busy}>
            <RefreshCcw size={16} aria-hidden="true" />
            <span>Refresh</span>
          </button>
        </div>
      </header>

      {stateMap && stateMap.sessions.length > 0 ? (
        <>
          <section className="statemap-detail-heading">
            <div>
              <span className="eyebrow">
                Compiled from the {stateMap.sessions.length} most recent active{" "}
                {stateMap.sessions.length === 1 ? "session" : "sessions"}
              </span>
            </div>
            <button type="button" className="ghost-action" onClick={onBackToPlay}>
              <Play size={16} aria-hidden="true" />
              <span>Back to Play</span>
            </button>
          </section>

          <div className="statemap-grid statemap-dashboard">
            <section className="statemap-panel statemap-scene-panel">
              <h2 className="statemap-panel-title">
                <span className="statemap-dot warm" /> Scene state{" "}
                <span className="statemap-count">{stateMap.scenes.length} sessions</span>
              </h2>
              {latestScene ? (
                <dl className="statemap-fields">
                  <div><dt>Latest location</dt><dd>{latestScene.location || "Unknown"}</dd></div>
                  <div><dt>Latest time</dt><dd>{latestScene.time_elapsed || "Unknown"}</dd></div>
                  <div><dt>Current focus</dt><dd>{humanizeStateMapText(latestScene.focus || latestScene.current_scene) || "No focus captured yet."}</dd></div>
                  <div><dt>Pressure</dt><dd>{latestScene.pressure_point || "No pressure point captured yet."}</dd></div>
                </dl>
              ) : null}
              {stateMap.scenes.length > 0 ? (
                <ul className="statemap-source-list">
                  {stateMap.scenes.map((scene) => (
                    <li key={scene.session_id}>
                      <strong>{scene.session_title}</strong>
                      <span>{scene.current_scene || scene.focus || scene.location || "No scene state yet."}</span>
                    </li>
                  ))}
                </ul>
              ) : null}
            </section>

            <section className="statemap-panel statemap-characters-panel">
              <h2 className="statemap-panel-title">
                <span className="statemap-dot cool" /> Characters <span className="statemap-count">who knows what</span>
              </h2>
              <div className="statemap-character-list">
                {presentation?.characters.slice(0, 8).map((character, index) => (
                  <div key={character.identity_key} className="statemap-character-row">
                    <span className={`statemap-character-avatar${index > 0 ? " muted" : ""}`}>
                      {(character.name.charAt(0) || "?").toUpperCase()}
                    </span>
                    <span>
                      <strong>{character.name}</strong>
                      <small>
                        {character.role || "Entity"} / {character.session_title}
                        {character.session_count > 1 ? ` + ${character.session_count - 1} sessions` : ""}
                      </small>
                      <em>{character.detail}</em>
                    </span>
                  </div>
                ))}
              </div>
            </section>

            <section className="statemap-panel statemap-relationships-panel">
              <h2 className="statemap-panel-title">
                <span className="statemap-dot green" /> Relationships{" "}
                <span className="statemap-count">{presentation?.relationships.length ?? 0}</span>
              </h2>
              {!presentation?.relationships.length ? (
                <p className="statemap-note">No relationships tracked yet.</p>
              ) : (
                <div className="statemap-rels">
                  {presentation.relationships.map((relationship) => (
                    <div key={relationship.identity_key} className="statemap-rel">
                      <div className="statemap-rel-name">
                        {relationship.soul_name} {"->"} {relationship.target}{" "}
                        <span className="statemap-rel-label">
                          {relationship.session_title}
                          {relationship.session_count > 1 ? ` + ${relationship.session_count - 1} sessions` : ""}
                          {" / "}
                          {relationship.love_type || "Relationship"}
                        </span>
                      </div>
                      <div className="statemap-rel-bars">
                        {relKeys.map(([label, key]) => {
                          const value = relationship[key];
                          return (
                            <div key={label} className="statemap-rel-bar">
                              <span className="statemap-rel-bar-k">{label}</span>
                              <span className="statemap-rel-bar-track">
                                <span
                                  className="statemap-rel-bar-fill"
                                  style={{
                                    left: value >= 0 ? "50%" : `${50 - Math.abs(value) / 2}%`,
                                    width: `${Math.abs(value) / 2}%`,
                                  }}
                                />
                              </span>
                              <span className="statemap-rel-bar-v">{value > 0 ? "+" : ""}{value}</span>
                            </div>
                          );
                        })}
                      </div>
                    </div>
                  ))}
                </div>
              )}
            </section>

            <section className="statemap-panel statemap-objects-panel">
              <h2 className="statemap-panel-title">
                <span className="statemap-dot gold" /> Objects{" "}
                <span className="statemap-count">identity / owner / status</span>
              </h2>
              {presentation?.objects.length ? (
                <ul className="statemap-object-list">
                  {presentation.objects.slice(0, 8).map((object) => (
                    <li key={object.identity_key}>
                      <strong>{object.name}</strong>
                      <span>
                        {object.session_title}
                        {object.session_count > 1 ? ` + ${object.session_count - 1} sessions` : ""}
                        {" / "}
                        {object.owner || "Unassigned"} / {object.status || object.kind}
                      </span>
                    </li>
                  ))}
                </ul>
              ) : (
                <p className="statemap-note">No tracked objects.</p>
              )}
            </section>

            <section className="statemap-panel statemap-timeline-panel">
              <h2 className="statemap-panel-title">
                <span className="statemap-dot violet" /> Timeline <span className="statemap-count">recent events</span>
              </h2>
              {stateMap.timeline.length > 0 ? (
                <ol className="statemap-timeline-list">
                  {stateMap.timeline.slice(-8).map((event, index) => (
                    <li key={`${event.session_id}-${index}`}>
                      <span>{event.turn_counter}</span>
                      <strong>{event.content}</strong>
                      <small>{event.session_title}</small>
                    </li>
                  ))}
                </ol>
              ) : (
                <p className="statemap-note">No recent events.</p>
              )}
            </section>

            <section className="statemap-panel statemap-memory-panel">
              <h2 className="statemap-panel-title">
                <span className="statemap-dot cool" /> Memory Inspector{" "}
                <span className="statemap-count">every memory traces to the turn that created it</span>
              </h2>
              {stateMap.memories.length === 0 ? (
                <p className="statemap-note">No memories yet.</p>
              ) : (
                <div className="statemap-memory-layout">
                  <div className="statemap-memory-stack">
                    {stateMap.memories.slice(0, 8).map((memory, index) => (
                      <article
                        key={`${memory.session_id}-${index}`}
                        className={`statemap-mem-card${memory.is_pinned ? " pinned" : ""}`}
                      >
                        <span className="statemap-mem-kind">{memory.tag || "memory"}</span>
                        <strong>{memory.content}</strong>
                        <small>
                          {memory.session_title} / {memory.source_turn ? `turn ${memory.source_turn}` : memory.source_type} /{" "}
                          {memory.truth_status}
                        </small>
                      </article>
                    ))}
                  </div>
                  <aside className="statemap-provenance">
                    <div className="statemap-subhead">Provenance</div>
                    <strong>{featuredMemory?.content || "No memory selected."}</strong>
                    <dl className="statemap-fields">
                      <div><dt>Session</dt><dd>{featuredMemory?.session_title ?? "unknown"}</dd></div>
                      <div><dt>Source turn</dt><dd>{featuredMemory?.source_turn ?? "unknown"}</dd></div>
                      <div><dt>Confidence</dt><dd>{featuredMemory?.confidence ?? "unknown"}</dd></div>
                      <div><dt>Truth status</dt><dd>{featuredMemory?.truth_status ?? "tracked"}</dd></div>
                      <div><dt>State</dt><dd>{featuredMemory?.is_pinned ? "Pinned" : featuredMemory?.is_active ? "Active" : "Stored"}</dd></div>
                    </dl>
                  </aside>
                </div>
              )}
            </section>

            <section className="statemap-panel statemap-knowledge-panel">
              <h2 className="statemap-panel-title">
                <span className="statemap-dot warm" /> Who Knows What{" "}
                <span className="statemap-count">
                  what has and has not been disclosed
                </span>
              </h2>
              <p className="statemap-note">
                The engine reads most of this from the transcript. Correct it here for
                anything it cannot see: a previous session, or something that happened
                off-screen.
              </p>
              <KnowledgeGrid conversationId={conversationId} busy={busy} />
            </section>

            <section className="statemap-panel statemap-memory-panel">
              <h2 className="statemap-panel-title">
                <span className="statemap-dot green" /> Memory V2 Evidence Map{" "}
                <span className="statemap-count">
                  {stateMap.memory_v2.filter((memory) => memory.validity === "valid").length} valid /{" "}
                  {stateMap.memory_v2.filter((memory) => memory.validity === "stale").length} stale
                </span>
              </h2>
              {stateMap.memory_v2.length === 0 ? (
                <p className="statemap-note">No ledger-backed Memory V2 projections yet.</p>
              ) : (
                <div className="statemap-memory-stack">
                  {stateMap.memory_v2.slice(0, 16).map((memory) => (
                    <article
                      key={`${memory.session_id}-${memory.memory_id}`}
                      className={`statemap-mem-card${memory.validity !== "valid" ? " stale" : ""}`}
                    >
                      <span className="statemap-mem-kind">
                        {memory.layer} / {memory.memory_kind} / {memory.validity}
                      </span>
                      <strong>{memory.content}</strong>
                      <small>
                        {memory.session_title} / confidence {memory.confidence.toFixed(2)} / {memory.truth_status}
                      </small>
                      <small>
                        {memory.layer === "derived"
                          ? `sources ${memory.source_memory_ids.length} / supports ${memory.supporting_evidence_count} / contradicts ${memory.contradicting_evidence_count}`
                          : `turn ${memory.source_turn_id ?? "unknown"} / patch ${memory.source_patch_id ?? "legacy base"}`}
                      </small>
                      {memory.source_quote ? <em>“{memory.source_quote}”</em> : null}
                    </article>
                  ))}
                </div>
              )}
            </section>
          </div>
        </>
      ) : (
        <p className="muted statemap-empty">No active sessions yet. Start one from the Library to populate the State Map.</p>
      )}
    </>
  );
}
