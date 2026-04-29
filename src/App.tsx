import {
  Brain,
  Database,
  FileDown,
  FileUp,
  MessageSquareText,
  Play,
  RefreshCcw,
  Save,
  Sparkles,
} from "lucide-react";
import { FormEvent, useEffect, useMemo, useRef, useState } from "react";
import {
  ChatMessage,
  ContextPreview,
  Soul,
  SoulSummary,
  compileContext,
  createDefaultSoul,
  getSoul,
  listSouls,
  runConsolidation,
  saveSoulFile,
  sendMockTurn,
  upsertSoul,
} from "./tauri";

const DEFAULT_CONVERSATION_ID = "local-mock";

export function App() {
  const [souls, setSouls] = useState<SoulSummary[]>([]);
  const [soul, setSoul] = useState<Soul | null>(null);
  const [messages, setMessages] = useState<ChatMessage[]>([]);
  const [context, setContext] = useState<ContextPreview | null>(null);
  const [draft, setDraft] = useState("");
  const [characterName, setCharacterName] = useState("Aurora Schwarz");
  const [mode, setMode] = useState("Reader");
  const [status, setStatus] = useState("Ready");
  const [busy, setBusy] = useState(false);
  const didBootstrap = useRef(false);

  useEffect(() => {
    if (didBootstrap.current) return;
    didBootstrap.current = true;
    void bootstrap();
  }, []);

  useEffect(() => {
    if (!soul) return;
    void refreshContext(soul.character_id);
  }, [soul?.character_id, messages.length]);

  async function bootstrap() {
    const existing = await listSouls();
    setSouls(existing);

    if (existing.length > 0) {
      setStatus("Loaded local Soul index");
      return;
    }

    const nextSoul = await createDefaultSoul(characterName);
    await upsertSoul(nextSoul);
    setSoul(nextSoul);
    setSouls(await listSouls());
    setStatus("Created starter Soul");
  }

  async function refreshContext(soulId: string) {
    const preview = await compileContext(soulId, DEFAULT_CONVERSATION_ID);
    setContext(preview);
  }

  async function handleCreateSoul() {
    setBusy(true);
    try {
      const nextSoul = await createDefaultSoul(characterName || "Unnamed Character");
      await upsertSoul(nextSoul);
      setSoul(nextSoul);
      setMessages([]);
      setSouls(await listSouls());
      setStatus("New Soul created");
    } catch (error) {
      setStatus(error instanceof Error ? error.message : String(error));
    } finally {
      setBusy(false);
    }
  }

  async function handleSelectSoul(characterId: string) {
    const selected = souls.find((item) => item.character_id === characterId);
    if (!selected) return;

    setBusy(true);
    try {
      const nextSoul = await getSoul(selected.character_id);
      setStatus(`Selected ${nextSoul.character_name}`);
      setSoul(nextSoul);
      setMessages([]);
    } catch (error) {
      setStatus(error instanceof Error ? error.message : String(error));
    } finally {
      setBusy(false);
    }
  }

  async function handleSubmit(event: FormEvent) {
    event.preventDefault();
    const text = draft.trim();
    if (!text || busy) return;

    setBusy(true);
    setDraft("");
    setStatus("Mock provider thinking");

    try {
      const result = await sendMockTurn(DEFAULT_CONVERSATION_ID, text);
      setSoul(result.soul);
      setMessages(result.messages);
      setContext(result.context_preview);
      setSouls(await listSouls());
      setStatus(result.consolidation_ran ? "Turn saved; consolidation ran" : "Turn saved");
    } catch (error) {
      setStatus(error instanceof Error ? error.message : String(error));
    } finally {
      setBusy(false);
    }
  }

  async function handleConsolidate() {
    if (!soul) return;
    setBusy(true);
    try {
      const nextSoul = await runConsolidation(soul.character_id);
      setSoul(nextSoul);
      setSouls(await listSouls());
      setStatus("Memory consolidated");
    } catch (error) {
      setStatus(error instanceof Error ? error.message : String(error));
    } finally {
      setBusy(false);
    }
  }

  async function handleSaveSoul() {
    if (!soul) return;
    setBusy(true);
    try {
      await saveSoulFile(`${soul.character_name.replace(/\s+/g, "_")}.soul.json`, soul);
      setStatus("Soul exported beside the app working directory");
    } catch (error) {
      setStatus(error instanceof Error ? error.message : String(error));
    } finally {
      setBusy(false);
    }
  }

  const relationship = soul?.relationships.user;
  const activeMessages = useMemo(
    () => messages.filter((message) => message.role !== "system"),
    [messages],
  );

  return (
    <main className="app-shell">
      <aside className="character-panel">
        <header className="panel-header">
          <div>
            <span className="eyebrow">Soul</span>
            <h1>{soul?.character_name ?? "Mnemosyne"}</h1>
          </div>
          <Brain aria-hidden="true" />
        </header>

        <div className="avatar" aria-hidden="true">
          {soul?.character_name.slice(0, 1) ?? "M"}
        </div>

        <label className="field">
          <span>Character</span>
          <input
            value={characterName}
            onChange={(event) => setCharacterName(event.target.value)}
            placeholder="Character name"
          />
        </label>

        <button className="wide-button" onClick={handleCreateSoul} disabled={busy}>
          <Sparkles size={18} />
          New Soul
        </button>

        <section className="compact-list" aria-label="Saved souls">
          <h2>Local Souls</h2>
          {souls.length === 0 ? (
            <p className="muted">No saved Souls yet.</p>
          ) : (
            souls.map((item) => (
              <button
                key={item.character_id}
                className="soul-row"
                onClick={() => handleSelectSoul(item.character_id)}
              >
                <span>{item.character_name}</span>
                <small>
                  {item.core_count} core / {item.recent_count} recent
                </small>
              </button>
            ))
          )}
        </section>

        <section className="stat-grid" aria-label="Relationship stats">
          <Stat label="Trust" value={relationship?.trust ?? 0} />
          <Stat label="Affection" value={relationship?.affection ?? 0} />
          <Stat label="Fear" value={relationship?.fear ?? 0} />
          <Stat label="Resolve" value={soul?.global.resolve ?? 0} />
        </section>

        <section className="memory-section">
          <h2>Core Memories</h2>
          {(soul?.memory.core ?? []).slice(0, 4).map((memory) => (
            <p key={memory}>{memory}</p>
          ))}
        </section>

        <section className="memory-section">
          <h2>Schemas</h2>
          {(soul?.memory.schemas ?? []).map((schema) => (
            <p key={schema.schema_type}>
              <strong>{schema.schema_type}</strong>: {schema.summary}
            </p>
          ))}
        </section>
      </aside>

      <section className="chat-panel">
        <div className="chat-header">
          <div>
            <span className="eyebrow">Provider: Mock</span>
            <h2>Chat Window</h2>
          </div>
          <div className="token-pill">
            {context?.estimated_tokens ?? 0}
            <span>tok</span>
          </div>
        </div>

        <div className="message-list">
          {activeMessages.length === 0 ? (
            <div className="empty-state">
              <MessageSquareText size={34} />
              <p>Start a local mock turn to exercise memory, hidden state, and context compilation.</p>
            </div>
          ) : (
            activeMessages.map((message) => (
              <article className={`message ${message.role}`} key={message.id}>
                <span>{message.role === "user" ? "User" : "Narrator"}</span>
                <p>{message.content}</p>
              </article>
            ))
          )}
        </div>

        <form className="composer" onSubmit={handleSubmit}>
          <input
            value={draft}
            onChange={(event) => setDraft(event.target.value)}
            placeholder="Type message..."
            disabled={busy}
          />
          <button aria-label="Send message" disabled={busy || !draft.trim()}>
            <Play size={18} />
          </button>
        </form>
      </section>

      <aside className="controls-panel">
        <header className="panel-header">
          <div>
            <span className="eyebrow">Controls</span>
            <h2>Session</h2>
          </div>
          <Database aria-hidden="true" />
        </header>

        <label className="field">
          <span>Mode</span>
          <select value={mode} onChange={(event) => setMode(event.target.value)}>
            <option>Reader</option>
            <option>Realistic</option>
            <option>God</option>
          </select>
        </label>

        <div className="button-grid">
          <button title="Import Soul placeholder" disabled>
            <FileUp size={18} />
          </button>
          <button title="Export Soul" onClick={handleSaveSoul} disabled={!soul || busy}>
            <FileDown size={18} />
          </button>
          <button
            title="Persist current Soul"
            onClick={async () => {
              if (!soul) return;
              await upsertSoul(soul);
              setSouls(await listSouls());
              setStatus("Soul persisted");
            }}
            disabled={!soul || busy}
          >
            <Save size={18} />
          </button>
          <button title="Run consolidation" onClick={handleConsolidate} disabled={!soul || busy}>
            <RefreshCcw size={18} />
          </button>
        </div>

        <section className="context-preview">
          <h2>Context</h2>
          <pre>{context?.text ?? "No context compiled yet."}</pre>
        </section>

        <section className="memory-section">
          <h2>Recent</h2>
          {(soul?.memory.recent ?? []).map((memory) => (
            <p key={memory.id}>
              <strong>{memory.tag}</strong> / {memory.salience}: {memory.content}
            </p>
          ))}
        </section>

        <footer className="status-line">{status}</footer>
      </aside>
    </main>
  );
}

function Stat({ label, value }: { label: string; value: number }) {
  return (
    <div className="stat">
      <span>{label}</span>
      <strong>{value}</strong>
    </div>
  );
}
