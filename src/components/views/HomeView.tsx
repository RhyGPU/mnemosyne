import type { ReactNode } from "react";
import { Play } from "lucide-react";
import type { ConversationSummary, SoulSummary } from "../../tauri";

export function HomeView({
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
    <main className={`app-shell home-shell${railShellClass}`}>
      {appDialogNode}
      {railNav}
      {children}
    </main>
  );
}

export function HomeDashboard({
  busy,
  conversations,
  onOpenLibrary,
  onSelectConversation,
  onSelectSoul,
  souls,
}: {
  busy: boolean;
  conversations: ConversationSummary[];
  onOpenLibrary: () => void;
  onSelectConversation: (conversation: ConversationSummary) => void;
  onSelectSoul: (soulId: string) => void;
  souls: SoulSummary[];
}) {
  const playedSoulIds = new Set(conversations.map((conversation) => conversation.soul_id));
  const recent = [...conversations]
    .filter((conversation) => !conversation.archived_at)
    .sort((a, b) => b.updated_at - a.updated_at)
    .slice(0, 8);
  const hero = recent[0] ?? null;
  const heroSoul = hero ? souls.find((soul) => soul.character_id === hero.soul_id) ?? null : null;
  const recentStrip = recent.slice(hero ? 1 : 0, hero ? 4 : 3);
  const soulName = (id: string) => souls.find((soul) => soul.character_id === id)?.character_name ?? "Unknown soul";
  const initialOf = (id: string) => (soulName(id).charAt(0) || "?").toUpperCase();
  const recommended = souls.filter((soul) => !soul.archived_at && !playedSoulIds.has(soul.character_id)).slice(0, 6);

  return (
    <>
      <header className="home-header">
        <div>
          <span className="eyebrow">Home</span>
          <h1>Your living worlds</h1>
        </div>
      </header>

      <div className="home-layout">
        <div className="home-main">
          {hero ? (
            <div className="home-hero">
              <div className="home-hero-head">
                <div className="home-hero-avatar">{initialOf(hero.soul_id)}</div>
                <div>
                  <div className="home-hero-kicker">Continue - {formatRelativeTime(hero.updated_at)}</div>
                  <h2 className="home-hero-title">{hero.title || "Untitled"}</h2>
                </div>
              </div>
              <p className="home-hero-preview">
                {hero.last_message_preview ?? `${hero.message_count} messages in this session`}
              </p>
              <div className="home-hero-foot">
                <button type="button" className="ghost-action primary-cta" onClick={() => onSelectConversation(hero)} disabled={busy}>
                  <Play size={15} aria-hidden="true" />
                  <span>Resume - {hero.message_count} messages</span>
                </button>
                <div className="home-hero-stats">
                  <div className="home-hero-stat"><b>{(heroSoul?.core_count ?? 0) + (heroSoul?.recent_count ?? 0)}</b><span>memories</span></div>
                  <div className="home-hero-stat"><b>{heroSoul?.core_count ?? 0}</b><span>core</span></div>
                  <div className="home-hero-stat"><b>{heroSoul?.recent_count ?? 0}</b><span>recent</span></div>
                </div>
              </div>
            </div>
          ) : null}

          <div className="home-feature-row">
            <section className="home-feature-panel">
              <div className="home-section-label">Recommended <span className="home-side-note">never played</span></div>
              {recommended.length === 0 ? (
                <p className="home-side-empty">Every soul already has a session.</p>
              ) : (
                <div className="home-feature-list">
                  {recommended.slice(0, 3).map((soul) => (
                    <button
                      key={soul.character_id}
                      type="button"
                      className="home-feature-item"
                      onClick={() => {
                        onSelectSoul(soul.character_id);
                        onOpenLibrary();
                      }}
                      disabled={busy}
                    >
                      <span className="home-feature-avatar">{(soul.character_name.charAt(0) || "?").toUpperCase()}</span>
                      <span>
                        <strong>{soul.character_name}</strong>
                        <small>{soul.core_count} core / {soul.recent_count} recent</small>
                      </span>
                    </button>
                  ))}
                </div>
              )}
            </section>
            <section className="home-feature-panel">
              <div className="home-section-label">Best rated</div>
              <div className="home-rating-placeholder">
                <strong>Ratings will surface here.</strong>
                <span>Once the engine tracks ratings, this becomes the fast path back to your strongest worlds.</span>
              </div>
            </section>
          </div>

          <section className="home-recent-strip" aria-label="Most recent sessions">
            <div className="home-section-label">Most recent</div>
            <div className="home-recent-list">
              {recentStrip.length === 0 ? (
                <p className="muted">No sessions yet - start one from the Library.</p>
              ) : (
                recentStrip.map((conversation) => (
                  <button
                    key={conversation.conversation_id}
                    type="button"
                    className="home-recent-item"
                    onClick={() => onSelectConversation(conversation)}
                    disabled={busy}
                  >
                    <span className="home-recent-avatar">{initialOf(conversation.soul_id)}</span>
                    <span className="home-recent-copy">
                      <strong>{conversation.title || "Untitled"}</strong>
                      <small>{soulName(conversation.soul_id)} / {formatRelativeTime(conversation.updated_at)}</small>
                    </span>
                  </button>
                ))
              )}
            </div>
          </section>
        </div>

        <aside className="home-side">
          <div className="home-side-panel home-turn-panel">
            <div className="home-side-title">How a turn works</div>
            <div className="home-turn">
              <div><span className="home-turn-n">01</span><span><b>Narrator</b> writes the visible prose.</span></div>
              <div><span className="home-turn-n">02</span><span><b>Evaluator</b> extracts schema-checked state patches.</span></div>
              <div><span className="home-turn-n">03</span><span>The <b>state map</b> updates - nothing re-sent next turn.</span></div>
            </div>
          </div>
          <div className="home-side-panel">
            <div className="home-side-title">Waiting on you</div>
            <p className="home-side-empty">
              Souls whose feelings toward you run high - needs per-soul relationship stats in the soul list.
            </p>
          </div>
        </aside>
      </div>
    </>
  );
}

function formatRelativeTime(timestamp: number): string {
  const ms = timestamp > 1_000_000_000_000 ? timestamp : timestamp * 1000;
  const diff = Date.now() - ms;
  if (diff < 60_000) return "just now";
  if (diff < 3_600_000) return `${Math.floor(diff / 60_000)}m ago`;
  if (diff < 86_400_000) return `${Math.floor(diff / 3_600_000)}h ago`;
  if (diff < 604_800_000) return `${Math.floor(diff / 86_400_000)}d ago`;
  return new Date(ms).toLocaleDateString([], { month: "short", day: "numeric" });
}
