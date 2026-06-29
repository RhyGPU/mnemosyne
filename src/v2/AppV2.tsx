// UX v2 — runnable prototype of the new flow (mount with ?v2).
// Book/editorial paper theme; nav rail; composable Purpose; mode-driven redaction;
// living memory; mode lens; Soul biography. Mock-data driven. See docs/UX-plan-v2.md.
import { useMemo, useState } from "react";
import type { CSSProperties, ReactNode } from "react";
import type { NarrativeMode } from "../uiTypes";
import {
  PURPOSE_BUNDLES, PURPOSE_BLURB, purposeFrom, effectiveMode, isGodMode,
} from "./sessionPurpose";
import type { PurposeName, SessionPurpose, PurposeToggles, PanelKey } from "./sessionPurpose";
import { fieldVisibility } from "./redaction";
import type { Ownership, Visibility } from "./redaction";
import { c, serif, sans, mono, label, eyebrow, card } from "./theme";
import {
  scene, characters, relationships, objects, bodyRegions, timeline, memories,
  transcript, sessions, auroraBio, worlds, charRows,
} from "./mockData";

type View = "home" | "play" | "state" | "library" | "settings" | "bio";
const MODES: NarrativeMode[] = ["Realistic", "Reader", "Active Director", "GM Simulation"];

export default function AppV2() {
  const [view, setView] = useState<View>("home");
  const [purpose, setPurpose] = useState<SessionPurpose>(purposeFrom("Immersive"));
  const [selectedMode, setSelectedMode] = useState<NarrativeMode>(purpose.toggles.defaultMode);
  const [isGM, setIsGM] = useState(false);
  const [revealed, setRevealed] = useState<Set<string>>(new Set());
  const [selMem, setSelMem] = useState<string>("m1");
  const [composerOpen, setComposerOpen] = useState(false);

  const mode = effectiveMode(selectedMode, purpose.toggles, isGM);

  function startPurpose(name: PurposeName) {
    const p = purposeFrom(name);
    setPurpose(p);
    setSelectedMode(p.toggles.defaultMode);
    setIsGM(false);
    setRevealed(new Set());
    setView("play");
  }
  function reveal(key: string) {
    setRevealed((s) => new Set(s).add(key));
  }

  return (
    <div style={{ display: "flex", height: "100vh", background: c.paper, color: c.ink, fontFamily: sans, overflow: "hidden" }}>
      <NavRail view={view} setView={setView} immersive={view === "play"} />
      <main style={{ flex: 1, overflowY: "auto" }}>
        {view === "home" && <Home onStart={() => setComposerOpen(true)} onResume={() => setView("play")} />}
        {view === "play" && (
          <Play purpose={purpose} mode={mode} selectedMode={selectedMode} setSelectedMode={setSelectedMode}
            isGM={isGM} setIsGM={setIsGM} goState={() => setView("state")} openComposer={() => setComposerOpen(true)} />
        )}
        {view === "state" && (
          <StateMap purpose={purpose} mode={mode} selectedMode={selectedMode} setSelectedMode={setSelectedMode}
            isGM={isGM} setIsGM={setIsGM} revealed={revealed} reveal={reveal}
            selMem={selMem} setSelMem={setSelMem} openComposer={() => setComposerOpen(true)}
            goBio={() => setView("bio")} />
        )}
        {view === "bio" && <Biography onBack={() => setView("state")} />}
        {view === "library" && <Library onBio={() => setView("bio")} onResume={() => setView("play")} />}
        {view === "settings" && <Stub title="Settings" sub="Providers · Chat · Data · About" />}
      </main>
      {composerOpen && (
        <PurposeComposer purpose={purpose} setPurpose={(p) => { setPurpose(p); setSelectedMode(p.toggles.defaultMode); }}
          onClose={() => setComposerOpen(false)} onStart={startPurpose} />
      )}
    </div>
  );
}

/* ---------------- Nav rail ---------------- */
const NAV: { key: View; glyph: string; name: string; scope: string }[] = [
  { key: "home", glyph: "▣", name: "Home", scope: "global" },
  { key: "play", glyph: "▷", name: "Play", scope: "session" },
  { key: "state", glyph: "◈", name: "State Map", scope: "session" },
  { key: "library", glyph: "◰", name: "Library", scope: "global" },
  { key: "settings", glyph: "⚙", name: "Settings", scope: "overlay" },
];

function NavRail(props: { view: View; setView: (v: View) => void; immersive: boolean }) {
  const w = props.immersive ? 60 : 210;
  return (
    <aside style={{ width: w, flex: "none", background: c.panel, borderRight: `1px solid ${c.panelEdge}`, display: "flex", flexDirection: "column", padding: "20px 12px", transition: "width .15s" }}>
      <div style={{ padding: "0 6px 20px" }}>
        <div style={{ fontFamily: serif, fontSize: props.immersive ? 16 : 20, fontWeight: 600, color: c.ink }}>{props.immersive ? "M" : "Mnemosyne"}</div>
        {!props.immersive && <div style={{ ...label, fontSize: 9.5, marginTop: 4 }}>the narrator writes · the soul remembers</div>}
      </div>
      <nav style={{ display: "flex", flexDirection: "column", gap: 2 }}>
        {NAV.map((n) => {
          const active = props.view === n.key;
          return (
            <button key={n.key} onClick={() => props.setView(n.key)} title={n.name}
              style={{ display: "flex", alignItems: "center", gap: 11, padding: "9px 10px", border: "none", borderRadius: 3, cursor: "pointer", textAlign: "left", fontFamily: sans, fontSize: 13.5,
                background: active ? c.paper : "transparent", color: active ? c.ink : c.inkSoft,
                boxShadow: active ? `inset 2px 0 0 ${c.accent}` : "none" }}>
              <span style={{ fontSize: 15, width: 16, textAlign: "center" }}>{n.glyph}</span>
              {!props.immersive && <span>{n.name}</span>}
            </button>
          );
        })}
      </nav>
      {!props.immersive && (
        <div style={{ marginTop: "auto", borderTop: `1px solid ${c.ruleSoft}`, paddingTop: 12, ...label, fontSize: 9.5, lineHeight: 1.7 }}>
          rail recedes in Play &amp; Dev
        </div>
      )}
    </aside>
  );
}

/* ---------------- Home ---------------- */
function Home(props: { onStart: () => void; onResume: () => void }) {
  return (
    <div style={{ padding: "34px 44px 60px", maxWidth: 1100, margin: "0 auto" }}>
      <div style={{ display: "flex", justifyContent: "space-between", alignItems: "flex-end", marginBottom: 26 }}>
        <div>
          <div style={eyebrow}>Campaigns</div>
          <h1 style={{ fontFamily: serif, fontSize: 30, fontWeight: 600, margin: "6px 0 0" }}>Your living worlds</h1>
        </div>
        <button onClick={props.onStart} style={primaryBtn}>+ New session</button>
      </div>
      <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr", gap: 16 }}>
        {sessions.map((s) => (
          <button key={s.id} onClick={props.onResume} style={card({ textAlign: "left", cursor: "pointer", display: "flex", flexDirection: "column", gap: 10 })}>
            <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center" }}>
              <span style={{ fontFamily: serif, fontSize: 17, fontWeight: 600 }}>{s.title}</span>
              <PurposeBadge name={s.purpose} />
            </div>
            <div style={{ fontFamily: serif, fontSize: 13.5, lineHeight: 1.5, color: c.inkSoft, minHeight: 42 }}>{s.scene}</div>
            <div style={{ ...label, fontSize: 10, borderTop: `1px solid ${c.ruleSoft}`, paddingTop: 9 }}>{s.turns} turns · {s.last}</div>
          </button>
        ))}
      </div>
      <p style={{ ...label, fontSize: 10.5, marginTop: 22 }}>new session begins by composing a Purpose — it configures every surface</p>
    </div>
  );
}

function PurposeBadge(props: { name: PurposeName }) {
  return (
    <span style={{ fontFamily: mono, fontSize: 10, letterSpacing: ".08em", textTransform: "uppercase", color: c.accent, border: `1px solid ${c.panelEdge}`, background: c.paper, borderRadius: 20, padding: "3px 9px" }}>{props.name}</span>
  );
}

/* ---------------- Play (book register) ---------------- */
function Play(props: {
  purpose: SessionPurpose; mode: NarrativeMode; selectedMode: NarrativeMode;
  setSelectedMode: (m: NarrativeMode) => void; isGM: boolean; setIsGM: (b: boolean) => void;
  goState: () => void; openComposer: () => void;
}) {
  const t = props.purpose.toggles;
  return (
    <div style={{ display: "flex", flexDirection: "column", height: "100vh", background: c.book }}>
      <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center", padding: "16px 34px", borderBottom: `1px solid ${c.rule}` }}>
        <div>
          <div style={{ fontFamily: serif, fontSize: 17, fontWeight: 600 }}>The Ashgate Conspiracy</div>
          <div style={{ ...label, fontSize: 10, marginTop: 2 }}>Ashgate · turn 45 · <PurposeName2 name={props.purpose.base} /></div>
        </div>
        <div style={{ display: "flex", gap: 10, alignItems: "center" }}>
          <ModeLens purpose={props.purpose} selectedMode={props.selectedMode} setSelectedMode={props.setSelectedMode} isGM={props.isGM} setIsGM={props.setIsGM} />
          <button onClick={props.goState} style={ghostBtn}>◈ State Map</button>
          <button onClick={props.openComposer} style={ghostBtn}>Purpose…</button>
        </div>
      </div>
      <div style={{ flex: 1, overflowY: "auto", padding: "34px 44px" }}>
        <div style={{ maxWidth: 660, margin: "0 auto" }}>
          {transcript.map((m, i) => (
            <div key={i} style={{ marginBottom: 28 }}>
              <div style={{ ...label, fontSize: 10, marginBottom: 8, color: m.role === "narrator" ? c.accent : c.inkFaint }}>
                {m.role === "narrator" ? "Narrator" : "You · Aurora"} · turn {m.turn}
              </div>
              <div style={{ fontFamily: serif, fontSize: 17.5, lineHeight: 1.72, color: c.ink, fontStyle: m.role === "user" ? "italic" : "normal", paddingLeft: m.role === "user" ? 16 : 0, borderLeft: m.role === "user" ? `2px solid ${c.rule}` : "none" }}>
                {m.text}
              </div>
              {t.sensoryCallbacks && m.sensory && (
                <div style={{ marginTop: 10, fontFamily: mono, fontSize: 11, color: c.accentSoft, display: "flex", gap: 8, alignItems: "center" }}>
                  <span>❦</span><span><i>{m.sensory.cue}</i> stirs the memory of {m.sensory.memory}</span>
                </div>
              )}
            </div>
          ))}
        </div>
      </div>
      <div style={{ padding: "16px 44px 22px", borderTop: `1px solid ${c.rule}` }}>
        <div style={{ maxWidth: 660, margin: "0 auto", display: "flex", gap: 10 }}>
          <input placeholder="Write Aurora's next move…" style={{ flex: 1, padding: "12px 14px", borderRadius: 4, border: `1px solid ${c.rule}`, background: c.panel, color: c.ink, fontFamily: serif, fontSize: 16, outline: "none" }} />
          <button style={primaryBtn}>Send</button>
        </div>
      </div>
    </div>
  );
}

function PurposeName2(props: { name: PurposeName }) {
  return <span style={{ color: c.accent }}>{props.name} purpose</span>;
}

/* ---------------- Mode lens (segmented, ceiling-aware) ---------------- */
function ModeLens(props: {
  purpose: SessionPurpose; selectedMode: NarrativeMode; setSelectedMode: (m: NarrativeMode) => void;
  isGM: boolean; setIsGM: (b: boolean) => void;
}) {
  const t = props.purpose.toggles;
  return (
    <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
      <div style={{ display: "flex", border: `1px solid ${c.rule}`, borderRadius: 4, overflow: "hidden" }}>
        {MODES.map((m) => {
          const capped = t.asymmetricVisibility && !props.isGM && MODES.indexOf(m) > MODES.indexOf(t.modeCeiling);
          const active = props.selectedMode === m;
          return (
            <button key={m} disabled={capped} onClick={() => props.setSelectedMode(m)} title={capped ? "Above this purpose's player ceiling — switch to GM" : m}
              style={{ padding: "6px 9px", border: "none", borderRight: `1px solid ${c.ruleSoft}`, cursor: capped ? "not-allowed" : "pointer", fontFamily: mono, fontSize: 10.5,
                background: active ? c.accent : "transparent", color: active ? c.panel : capped ? c.inkFaint : c.inkSoft, opacity: capped ? 0.5 : 1 }}>
              {m === "Active Director" ? "Director" : m === "GM Simulation" ? "GM" : m}{isGodMode(m) ? " ☉" : ""}
            </button>
          );
        })}
      </div>
      {t.asymmetricVisibility && (
        <button onClick={() => props.setIsGM(!props.isGM)} title="Whoever holds god-mode is the GM"
          style={{ ...ghostBtn, borderColor: props.isGM ? c.accent : c.rule, color: props.isGM ? c.accent : c.inkSoft }}>
          {props.isGM ? "GM ✓" : "GM"}
        </button>
      )}
    </div>
  );
}

/* ---------------- State Map (editorial register) ---------------- */
function StateMap(props: {
  purpose: SessionPurpose; mode: NarrativeMode; selectedMode: NarrativeMode;
  setSelectedMode: (m: NarrativeMode) => void; isGM: boolean; setIsGM: (b: boolean) => void;
  revealed: Set<string>; reveal: (k: string) => void; selMem: string; setSelMem: (id: string) => void;
  openComposer: () => void; goBio: () => void;
}) {
  const t = props.purpose.toggles;
  const order = useMemo(() => panelOrder(t.emphasize), [t.emphasize]);
  return (
    <div style={{ padding: "28px 40px 60px", maxWidth: 1180, margin: "0 auto" }}>
      <div style={{ display: "flex", justifyContent: "space-between", alignItems: "flex-end", marginBottom: 20 }}>
        <div>
          <div style={eyebrow}>State Map · {props.purpose.base} purpose</div>
          <h1 style={{ fontFamily: serif, fontSize: 26, fontWeight: 600, margin: "6px 0 0" }}>What the engine believes is true</h1>
          <div style={{ ...label, fontSize: 10, marginTop: 6 }}>backend stores all · this view hides per mode</div>
        </div>
        <div style={{ display: "flex", gap: 10, alignItems: "center" }}>
          <ModeLens purpose={props.purpose} selectedMode={props.selectedMode} setSelectedMode={props.setSelectedMode} isGM={props.isGM} setIsGM={props.setIsGM} />
          {t.biography && <button onClick={props.goBio} style={ghostBtn}>Soul biography →</button>}
          <button onClick={props.openComposer} style={ghostBtn}>Purpose…</button>
        </div>
      </div>
      <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr", gap: 16, alignItems: "start" }}>
        {order.map((k) => (
          <Panel key={k} k={k} purpose={props.purpose} mode={props.mode} revealed={props.revealed} reveal={props.reveal} selMem={props.selMem} setSelMem={props.setSelMem} />
        ))}
      </div>
    </div>
  );
}

function panelOrder(emphasize: PanelKey[]): PanelKey[] {
  const all: PanelKey[] = ["scene", "characters", "relationships", "objects", "body", "timeline", "memory"];
  const rest = all.filter((k) => !emphasize.includes(k));
  return [...emphasize, ...rest];
}

function Panel(props: {
  k: PanelKey; purpose: SessionPurpose; mode: NarrativeMode;
  revealed: Set<string>; reveal: (key: string) => void; selMem: string; setSelMem: (id: string) => void;
}) {
  const { k } = props;
  if (k === "body" && !props.purpose.toggles.bodyGhost) return null;
  if (k === "memory") return <MemoryPanel purpose={props.purpose} selMem={props.selMem} setSelMem={props.setSelMem} />;
  return (
    <div style={card()}>
      <PanelHead title={titleOf(k)} />
      {k === "scene" && <SceneBody />}
      {k === "characters" && <CharactersBody mode={props.mode} toggles={props.purpose.toggles} revealed={props.revealed} reveal={props.reveal} />}
      {k === "relationships" && <RelBody />}
      {k === "objects" && <ObjectsBody />}
      {k === "body" && <BodyBody />}
      {k === "timeline" && <TimelineBody />}
    </div>
  );
}

const titleOf = (k: PanelKey): string =>
  ({ scene: "Scene State", characters: "Characters", relationships: "Relationships", objects: "Objects", body: "Body", timeline: "Timeline", memory: "Memory" }[k]);

function PanelHead(props: { title: string; right?: string }) {
  return (
    <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center", marginBottom: 12 }}>
      <span style={{ fontFamily: sans, fontWeight: 600, fontSize: 14, color: c.ink }}>{props.title}</span>
      {props.right && <span style={{ ...label, fontSize: 9.5 }}>{props.right}</span>}
    </div>
  );
}

function Field(props: { k: string; v: string }) {
  return (
    <div style={{ marginBottom: 10 }}>
      <div style={{ ...label, fontSize: 9, marginBottom: 3 }}>{props.k}</div>
      <div style={{ fontFamily: serif, fontSize: 13.5, color: c.ink }}>{props.v}</div>
    </div>
  );
}

function SceneBody() {
  return (
    <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr", gap: 4 }}>
      <Field k="Location" v={scene.location} />
      <Field k="Positions" v={scene.positions} />
      <Field k="Room state" v={scene.room} />
      <Field k="Active object" v={scene.object} />
    </div>
  );
}

/* Redaction-aware character secrets (the heart of the demo). */
function CharactersBody(props: { mode: NarrativeMode; toggles: PurposeToggles; revealed: Set<string>; reveal: (k: string) => void }) {
  return (
    <div style={{ display: "flex", flexDirection: "column", gap: 14 }}>
      {characters.map((ch) => (
        <div key={ch.id} style={{ display: "flex", gap: 12 }}>
          <div style={{ width: 30, height: 30, flex: "none", borderRadius: 4, display: "flex", alignItems: "center", justifyContent: "center", fontFamily: serif, fontWeight: 600, color: c.panel, background: ch.ownership === "self" ? c.accent : c.inkSoft }}>{ch.initial}</div>
          <div style={{ minWidth: 0 }}>
            <div style={{ fontFamily: serif, fontWeight: 600, fontSize: 14 }}>{ch.name}{ch.ownership === "self" && <span style={{ ...label, fontSize: 9, marginLeft: 8 }}>you</span>}</div>
            <div style={{ ...label, fontSize: 9.5, marginBottom: 5 }}>{ch.role}</div>
            <SecretRow tone={c.good} word="knows" ownership={ch.ownership} text={ch.knows} mode={props.mode} toggles={props.toggles} revealed={props.revealed} reveal={props.reveal} rk={ch.id + ":k"} />
            <SecretRow tone={c.warn} word="misbelieves" ownership={ch.ownership} text={ch.misbelieves} mode={props.mode} toggles={props.toggles} revealed={props.revealed} reveal={props.reveal} rk={ch.id + ":m"} />
          </div>
        </div>
      ))}
    </div>
  );
}

function SecretRow(props: { tone: string; word: string; ownership: Ownership; text: string; mode: NarrativeMode; toggles: PurposeToggles; revealed: Set<string>; reveal: (k: string) => void; rk: string }) {
  const vis = fieldVisibility({ sensitivity: "secret", ownership: props.ownership, mode: props.mode, toggles: props.toggles });
  return <SecretRowInner tone={props.tone} word={props.word} text={props.text} vis={vis} revealed={props.revealed} reveal={props.reveal} rk={props.rk} />;
}

function SecretRowInner(props: { tone: string; word: string; text: string; vis: Visibility; revealed: Set<string>; reveal: (k: string) => void; rk: string }) {
  if (props.vis === "omit") return null;
  const open = props.vis === "show" || props.revealed.has(props.rk);
  return (
    <div style={{ fontFamily: serif, fontSize: 13, lineHeight: 1.5, color: c.inkSoft, marginBottom: 2 }}>
      <span style={{ color: props.tone, fontFamily: mono, fontSize: 10.5, textTransform: "uppercase", letterSpacing: ".06em" }}>{props.word}</span>{" — "}
      {open ? <span>{props.text}</span> : (
        <button onClick={() => props.reveal(props.rk)} title="redacted — click to reveal"
          style={{ display: "inline-block", verticalAlign: "middle", background: c.redact, color: c.redact, border: "none", borderRadius: 2, height: 13, width: 132, cursor: "pointer" }} />
      )}
    </div>
  );
}

function RelBody() {
  return (
    <div style={{ display: "flex", flexDirection: "column", gap: 14 }}>
      {relationships.map((r) => (
        <div key={r.target}>
          <div style={{ fontFamily: serif, fontWeight: 600, fontSize: 13.5, marginBottom: 8 }}>{r.target}</div>
          {r.dims.map((d) => (
            <div key={d.k} style={{ display: "flex", alignItems: "center", gap: 10, marginBottom: 6 }}>
              <span style={{ width: 60, ...label, fontSize: 9.5 }}>{d.k}</span>
              <div style={{ flex: 1, height: 5, background: c.ruleSoft, borderRadius: 3, position: "relative" }}>
                <div style={{ position: "absolute", top: 0, bottom: 0, left: "50%", width: 1, background: c.rule }} />
                <div style={{ position: "absolute", top: 0, bottom: 0, borderRadius: 3, background: d.v >= 0 ? c.good : c.accent, left: d.v >= 0 ? "50%" : `${50 - Math.abs(d.v) / 2}%`, width: `${Math.abs(d.v) / 2}%` }} />
              </div>
              <span style={{ width: 30, textAlign: "right", fontFamily: mono, fontSize: 10.5, color: c.inkSoft }}>{d.v > 0 ? "+" : ""}{d.v}</span>
            </div>
          ))}
          <div style={{ ...label, fontSize: 9.5, marginTop: 4 }}>{r.event}</div>
        </div>
      ))}
    </div>
  );
}

function ObjectsBody() {
  return (
    <div style={{ display: "flex", flexDirection: "column", gap: 10 }}>
      {objects.map((o) => (
        <div key={o.name} style={{ display: "flex", justifyContent: "space-between", alignItems: "center", borderBottom: `1px solid ${c.ruleSoft}`, paddingBottom: 9 }}>
          <div><div style={{ fontFamily: serif, fontWeight: 600, fontSize: 13.5 }}>{o.name}</div><div style={{ ...label, fontSize: 9.5 }}>{o.owner} · {o.loc}</div></div>
          <span style={{ fontFamily: mono, fontSize: 10, color: c.warn, background: c.paper, border: `1px solid ${c.panelEdge}`, borderRadius: 4, padding: "3px 8px" }}>{o.status}</span>
        </div>
      ))}
    </div>
  );
}

function BodyBody() {
  return (
    <div style={{ display: "flex", flexDirection: "column", gap: 8 }}>
      <div style={{ ...label, fontSize: 9.5 }}>only narratively-active regions shown</div>
      {bodyRegions.map((b) => (
        <div key={b.label} style={{ display: "flex", justifyContent: "space-between", fontFamily: serif, fontSize: 13 }}>
          <span>{b.label}</span>
          <span style={{ fontFamily: mono, fontSize: 10.5, color: c.inkSoft }}>injury {b.injury} · fatigue {b.fatigue} · pain {b.pain}</span>
        </div>
      ))}
    </div>
  );
}

function TimelineBody() {
  return (
    <div style={{ display: "flex", flexDirection: "column" }}>
      {timeline.map((tl, i) => (
        <div key={i} style={{ display: "flex", gap: 12, paddingBottom: 11 }}>
          <div style={{ width: 30, textAlign: "right", ...label, fontSize: 9.5 }}>{tl.turn}</div>
          <div style={{ fontFamily: serif, fontSize: 13, borderLeft: `1px solid ${c.rule}`, paddingLeft: 12 }}>{tl.label}</div>
        </div>
      ))}
    </div>
  );
}

/* ---------------- Living memory ---------------- */
function MemoryPanel(props: { purpose: SessionPurpose; selMem: string; setSelMem: (id: string) => void }) {
  const lm = props.purpose.toggles.livingMemory;
  const sel = memories.find((m) => m.id === props.selMem) ?? memories[0];
  return (
    <div style={card({ gridColumn: "1 / 3" })}>
      <PanelHead title="Memory" right={lm === "off" ? "surfacing off for this purpose" : "✦ consolidation turn 50 · 2 memories → schema"} />
      {lm === "off" ? (
        <div style={{ fontFamily: serif, fontSize: 13, color: c.inkFaint }}>This purpose keeps memory in the background. Switch to a purpose with living memory to inspect it.</div>
      ) : (
        <div style={{ display: "grid", gridTemplateColumns: "1fr 300px", gap: 18, alignItems: "start" }}>
          <div style={{ display: "flex", flexDirection: "column", gap: 9 }}>
            {memories.map((m) => {
              const dead = m.truth === "invalidated";
              const weight = m.cls === "core" ? 1 : m.cls === "recent" ? 0.7 : 0.4;
              return (
                <button key={m.id} onClick={() => props.setSelMem(m.id)} style={{ textAlign: "left", cursor: "pointer", border: `1px solid ${m.id === props.selMem ? c.accentSoft : c.panelEdge}`, background: m.id === props.selMem ? c.paper : c.panel, borderRadius: 4, padding: "11px 13px", opacity: dead ? 0.5 : 1 }}>
                  <div style={{ display: "flex", justifyContent: "space-between", marginBottom: 5 }}>
                    <span style={{ ...label, fontSize: 9, color: c.accent }}>{m.cls} · {m.tag}</span>
                    {lm === "full" && <span style={{ fontFamily: mono, fontSize: 9.5, color: c.inkFaint }}>sal {m.salience.toFixed(2)}</span>}
                  </div>
                  <div style={{ fontFamily: serif, fontSize: 13 + weight * 2.5, color: dead ? c.inkFaint : c.ink, lineHeight: 1.5, textDecoration: dead ? "line-through" : "none" }}>{m.text}</div>
                  <div style={{ ...label, fontSize: 9, marginTop: 5 }}>turn {m.turn} · {m.conf} confidence</div>
                  {lm === "full" && (
                    <div style={{ height: 3, marginTop: 7, background: c.ruleSoft, borderRadius: 2 }}>
                      <div style={{ height: 3, width: `${m.salience * 100}%`, background: dead ? c.inkFaint : c.accentSoft, borderRadius: 2 }} />
                    </div>
                  )}
                </button>
              );
            })}
          </div>
          <div style={{ ...card({ background: c.paper }), position: "sticky", top: 0 }}>
            <div style={{ ...label, fontSize: 9.5, marginBottom: 10 }}>Provenance</div>
            <div style={{ fontFamily: serif, fontSize: 14, lineHeight: 1.55, marginBottom: 12 }}>{sel.text}</div>
            <div style={{ ...label, fontSize: 9, marginBottom: 5 }}>Source turn {sel.turn} · {sel.conf} confidence · {sel.truth}</div>
            <div style={{ ...label, fontSize: 9, marginBottom: 5, marginTop: 10 }}>Evidence quote</div>
            <div style={{ fontFamily: serif, fontStyle: "italic", fontSize: 13, color: c.inkSoft, borderLeft: `2px solid ${c.rule}`, paddingLeft: 11 }}>{sel.evidence}</div>
          </div>
        </div>
      )}
    </div>
  );
}

/* ---------------- Purpose composer (toggles) ---------------- */
function PurposeComposer(props: { purpose: SessionPurpose; setPurpose: (p: SessionPurpose) => void; onClose: () => void; onStart: (n: PurposeName) => void }) {
  const [draft, setDraft] = useState<SessionPurpose>(props.purpose);
  const t = draft.toggles;
  function set<K extends keyof PurposeToggles>(k: K, v: PurposeToggles[K]) {
    setDraft({ ...draft, toggles: { ...draft.toggles, [k]: v } });
  }
  const names = Object.keys(PURPOSE_BUNDLES) as PurposeName[];
  return (
    <div style={{ position: "fixed", inset: 0, background: "rgba(20,16,12,.35)", display: "flex", justifyContent: "flex-end", zIndex: 100 }}>
      <aside style={{ width: 420, background: c.paper, borderLeft: `1px solid ${c.panelEdge}`, padding: 24, overflowY: "auto" }}>
        <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center", marginBottom: 6 }}>
          <h1 style={{ fontFamily: serif, fontSize: 21, fontWeight: 600, margin: 0 }}>Compose a Purpose</h1>
          <button onClick={props.onClose} style={{ ...ghostBtn, padding: "4px 9px" }}>×</button>
        </div>
        <div style={{ ...label, fontSize: 10, marginBottom: 14 }}>presets are just starting toggles — recompose freely</div>
        <div style={{ display: "flex", flexWrap: "wrap", gap: 7, marginBottom: 18 }}>
          {names.map((n) => (
            <button key={n} onClick={() => setDraft(purposeFrom(n))} style={{ ...chip(draft.base === n), display: "flex", flexDirection: "column", alignItems: "flex-start", gap: 2 }}>
              <span>{n}</span>
            </button>
          ))}
        </div>
        <div style={{ ...label, fontSize: 10, marginBottom: 10 }}>{PURPOSE_BLURB[draft.base]}</div>

        <Row label="Default mode">
          <select value={t.defaultMode} onChange={(e) => set("defaultMode", e.target.value as NarrativeMode)} style={selectStyle}>
            {MODES.map((m) => <option key={m} value={m}>{m}</option>)}
          </select>
        </Row>
        <Row label="Player ceiling">
          <select value={t.modeCeiling} onChange={(e) => set("modeCeiling", e.target.value as NarrativeMode)} style={selectStyle}>
            {MODES.map((m) => <option key={m} value={m}>{m}</option>)}
          </select>
        </Row>
        <Toggle label="Dramatic irony (NPC secrets earlier)" v={t.dramaticIrony} on={(b) => set("dramaticIrony", b)} />
        <Toggle label="Asymmetric visibility (GM sees more)" v={t.asymmetricVisibility} on={(b) => set("asymmetricVisibility", b)} />
        <Toggle label="Sensory callbacks in Play" v={t.sensoryCallbacks} on={(b) => set("sensoryCallbacks", b)} />
        <Toggle label="Body ghost on State Map" v={t.bodyGhost} on={(b) => set("bodyGhost", b)} />
        <Toggle label="Soul biography" v={t.biography} on={(b) => set("biography", b)} />
        <Row label="Living memory">
          <select value={t.livingMemory} onChange={(e) => set("livingMemory", e.target.value as PurposeToggles["livingMemory"])} style={selectStyle}>
            <option value="off">off</option><option value="ambient">ambient</option><option value="full">full</option>
          </select>
        </Row>

        <div style={{ display: "flex", gap: 10, marginTop: 22 }}>
          <button onClick={() => { props.setPurpose(draft); props.onClose(); }} style={ghostBtn}>Apply to session</button>
          <button onClick={() => { props.setPurpose(draft); props.onStart(draft.base); props.onClose(); }} style={primaryBtn}>Start session →</button>
        </div>
      </aside>
    </div>
  );
}

function Row(props: { label: string; children: ReactNode }) {
  return (
    <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center", padding: "9px 0", borderBottom: `1px solid ${c.ruleSoft}` }}>
      <span style={{ fontFamily: sans, fontSize: 12.5, color: c.ink }}>{props.label}</span>
      {props.children}
    </div>
  );
}
function Toggle(props: { label: string; v: boolean; on: (b: boolean) => void }) {
  return (
    <Row label={props.label}>
      <button onClick={() => props.on(!props.v)} style={{ ...chip(props.v), minWidth: 52 }}>{props.v ? "on" : "off"}</button>
    </Row>
  );
}

/* ---------------- Soul biography (innovation #4) ---------------- */
function Biography(props: { onBack: () => void }) {
  const b = auroraBio;
  const W = 560, H = 130, pad = 8;
  const maxS = b.totalSessions;
  const pts = b.trust
    .map((p) => `${pad + (p.session / maxS) * (W - pad * 2)},${H - pad - (p.trust / 100) * (H - pad * 2)}`)
    .join(" ");
  const lastTrust = b.trust[b.trust.length - 1];
  const lastPhase = b.traumaPhase[b.traumaPhase.length - 1];
  return (
    <div style={{ padding: "28px 44px 60px", maxWidth: 900, margin: "0 auto" }}>
      <button onClick={props.onBack} style={{ ...ghostBtn, marginBottom: 16 }}>← Back to State Map</button>
      <div style={eyebrow}>Soul biography · {b.name}</div>
      <h1 style={{ fontFamily: serif, fontSize: 27, fontWeight: 600, margin: "6px 0 4px" }}>How she has changed over {b.totalSessions} sessions</h1>
      <div style={{ ...label, fontSize: 10, marginBottom: 22 }}>the long arc your engine makes possible — proof she remembers</div>

      <div style={card({ marginBottom: 16 })}>
        <PanelHead title="Trust toward you" right={`now ${lastTrust.trust}`} />
        <svg viewBox={`0 0 ${W} ${H}`} width="100%" height={H} role="img" aria-label="Trust over sessions">
          <line x1={pad} y1={H - pad} x2={W - pad} y2={H - pad} stroke={c.ruleSoft} strokeWidth={1} />
          <line x1={pad} y1={pad} x2={pad} y2={H - pad} stroke={c.ruleSoft} strokeWidth={1} />
          <polyline points={pts} fill="none" stroke={c.accent} strokeWidth={2} />
          {b.trust.map((p) => (
            <circle key={p.session} cx={pad + (p.session / maxS) * (W - pad * 2)} cy={H - pad - (p.trust / 100) * (H - pad * 2)} r={2.5} fill={c.accent} />
          ))}
        </svg>
        <div style={{ ...label, fontSize: 9, marginTop: 4 }}>session 1 → {b.totalSessions} · trust 0–100, speed-gated</div>
      </div>

      <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr", gap: 16, marginBottom: 16 }}>
        <div style={card()}>
          <PanelHead title="Trauma phase" right="slow change" />
          <div style={{ fontFamily: serif, fontSize: 14, color: c.ink }}>Phase {lastPhase.phase} / 4</div>
          <div style={{ ...label, fontSize: 9.5, marginTop: 6 }}>stepped up at session {b.traumaPhase[1].session}</div>
        </div>
        <div style={card()}>
          <PanelHead title="Identity drift" right="very slow" />
          <div style={{ height: 6, background: c.ruleSoft, borderRadius: 3, marginTop: 4 }}>
            <div style={{ height: 6, width: `${b.identityDrift * 100}%`, background: c.accentSoft, borderRadius: 3 }} />
          </div>
          <div style={{ ...label, fontSize: 9.5, marginTop: 6 }}>{Math.round(b.identityDrift * 100)}% from her starting self — core identity holds</div>
        </div>
      </div>

      <div style={card()}>
        <PanelHead title="Milestones" right="this timeline is your chapter outline" />
        <div style={{ display: "flex", flexDirection: "column" }}>
          {b.milestones.map((m) => (
            <div key={m.session} style={{ display: "flex", gap: 12, paddingBottom: 11 }}>
              <div style={{ width: 60, ...label, fontSize: 9.5 }}>session {m.session}</div>
              <div style={{ fontFamily: serif, fontSize: 13.5, borderLeft: `1px solid ${c.rule}`, paddingLeft: 12 }}>{m.label}</div>
            </div>
          ))}
        </div>
      </div>
    </div>
  );
}

/* ---------------- Library ---------------- */
function Library(props: { onBio: () => void; onResume: () => void }) {
  return (
    <div style={{ padding: "34px 44px 60px", maxWidth: 1100, margin: "0 auto" }}>
      <div style={eyebrow}>Library</div>
      <h1 style={{ fontFamily: serif, fontSize: 28, fontWeight: 600, margin: "6px 0 22px" }}>Characters &amp; Worlds</h1>

      <div style={{ ...label, fontSize: 10, marginBottom: 12 }}>Worlds</div>
      <div style={{ display: "grid", gridTemplateColumns: "repeat(3,1fr)", gap: 14, marginBottom: 26 }}>
        {worlds.map((w) => (
          <div key={w.id} style={card({ display: "flex", flexDirection: "column", gap: 8 })}>
            <div style={{ fontFamily: serif, fontSize: 17, fontWeight: 600 }}>{w.name}</div>
            <div style={{ ...label, fontSize: 9.5 }}>{w.genre}</div>
            <div style={{ fontFamily: serif, fontSize: 13, color: c.inkSoft, lineHeight: 1.5, flex: 1 }}>{w.desc}</div>
            <div style={{ ...label, fontSize: 9.5, borderTop: `1px solid ${c.ruleSoft}`, paddingTop: 9 }}>{w.sessions} sessions · {w.chars} characters</div>
          </div>
        ))}
      </div>

      <div style={{ ...label, fontSize: 10, marginBottom: 12 }}>Characters</div>
      <div style={{ display: "grid", gridTemplateColumns: "repeat(3,1fr)", gap: 14 }}>
        {charRows.map((ch) => (
          <div key={ch.id} style={card({ display: "flex", flexDirection: "column", gap: 10 })}>
            <div style={{ display: "flex", gap: 12, alignItems: "center" }}>
              <div style={{ width: 40, height: 40, flex: "none", borderRadius: 4, display: "flex", alignItems: "center", justifyContent: "center", fontFamily: serif, fontWeight: 600, fontSize: 18, color: c.panel, background: ch.self ? c.accent : c.inkSoft }}>{ch.initial}</div>
              <div>
                <div style={{ fontFamily: serif, fontSize: 15, fontWeight: 600 }}>{ch.name}</div>
                <div style={{ ...label, fontSize: 9.5 }}>{ch.role}</div>
              </div>
            </div>
            <div style={{ display: "flex", gap: 7, flexWrap: "wrap" }}>
              <span style={chip(false)}>{ch.world}</span>
              <span style={chip(false)}>{ch.preset}</span>
            </div>
            {ch.self && <button onClick={props.onBio} style={{ ...ghostBtn, alignSelf: "flex-start" }}>Soul biography →</button>}
          </div>
        ))}
      </div>
    </div>
  );
}

/* ---------------- misc ---------------- */
function Stub(props: { title: string; sub: string }) {
  return (
    <div style={{ padding: "34px 44px" }}>
      <div style={eyebrow}>{props.title}</div>
      <h1 style={{ fontFamily: serif, fontSize: 28, fontWeight: 600, margin: "6px 0 8px" }}>{props.title}</h1>
      <div style={{ ...label, fontSize: 11 }}>{props.sub} — wired in the real app; stubbed in this prototype.</div>
    </div>
  );
}

const primaryBtn: CSSProperties = { padding: "10px 16px", borderRadius: 4, border: "none", background: c.accent, color: c.paper, fontFamily: sans, fontWeight: 600, fontSize: 13, cursor: "pointer" };
const ghostBtn: CSSProperties = { padding: "8px 12px", borderRadius: 4, border: `1px solid ${c.rule}`, background: "transparent", color: c.inkSoft, fontFamily: sans, fontWeight: 500, fontSize: 12.5, cursor: "pointer" };
const selectStyle: CSSProperties = { fontFamily: mono, fontSize: 11.5, padding: "5px 8px", border: `1px solid ${c.rule}`, borderRadius: 4, background: c.panel, color: c.ink };
function chip(active: boolean): CSSProperties {
  return { padding: "6px 11px", borderRadius: 20, fontFamily: mono, fontSize: 11, cursor: "pointer", border: `1px solid ${active ? c.accent : c.rule}`, background: active ? c.paper : "transparent", color: active ? c.accent : c.inkSoft };
}
