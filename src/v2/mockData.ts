// UX v2 — mock data so the prototype runs without the backend.
import type { PurposeName } from "./sessionPurpose";
import type { Ownership } from "./redaction";

export interface MemoryItem {
  id: string;
  text: string;
  cls: "core" | "recent" | "schema";
  salience: number; // 0..1
  turn: number;
  conf: "low" | "med" | "high";
  truth: "active" | "pinned" | "invalidated";
  tag: string;
  evidence: string;
}

export interface CharState {
  id: string;
  name: string;
  initial: string;
  role: string;
  ownership: Ownership;
  knows: string;
  misbelieves: string;
}

export interface RelDim { k: string; v: number }
export interface Relationship { target: string; dims: RelDim[]; event: string }

export interface BodyRegion { label: string; group: string; side: string; injury: number; fatigue: number; pain: number }

export interface TimelineEvent { turn: string; label: string }

export interface TranscriptMsg {
  role: "narrator" | "user";
  turn: number;
  text: string;
  sensory?: { cue: string; memory: string };
}

export interface SessionRow {
  id: string;
  title: string;
  purpose: PurposeName;
  turns: number;
  last: string;
  scene: string;
}

export const scene = {
  location: "Ashgate Customs House — night, rain on the roof",
  time: "Turn 45 · late, after the gate-bell",
  positions: "Aurora at the doorway; Crow seated at the ledger desk",
  room: "Front door ajar; the strongroom behind Crow is locked",
  object: "The altered customs ledger, open on the desk",
  last: "Crow demanded Aurora explain the matching dates",
};

export const characters: CharState[] = [
  {
    id: "aurora", name: "Aurora Schwarz", initial: "A", role: "You · informant, forger",
    ownership: "self",
    knows: "Sereth ordered the barge; the real cargo was people, not silk",
    misbelieves: "Believes Crow can still be bought off",
  },
  {
    id: "crow", name: "Inspector Valen Crow", initial: "C", role: "Customs investigator",
    ownership: "npc",
    knows: "The forged seals and the harbor book disagree",
    misbelieves: "Thinks the barge carried untaxed silk",
  },
  {
    id: "sereth", name: "Lady Sereth", initial: "S", role: "Merchant House head · offscreen",
    ownership: "npc",
    knows: "Aurora can place her at the gate",
    misbelieves: "Believes Aurora has been paid into silence",
  },
];

export const relationships: Relationship[] = [
  { target: "Inspector Crow", dims: [{ k: "Trust", v: -15 }, { k: "Fear", v: 35 }, { k: "Affection", v: -5 }], event: "Crow tied the seals to the Sereth barge · turn 45" },
  { target: "Lady Sereth", dims: [{ k: "Trust", v: -40 }, { k: "Fear", v: 20 }, { k: "Affection", v: -30 }], event: "Learned the cargo was people · turn 43" },
];

export const objects = [
  { name: "Altered customs ledger", owner: "Customs House", loc: "Open on Crow's desk", status: "Evidence" },
  { name: "Forged seal set", owner: "Aurora", loc: "Hidden in the lamp base", status: "Concealed" },
  { name: "Sereth barge manifest", owner: "Lady Sereth", loc: "Unknown", status: "Missing" },
];

export const bodyRegions: BodyRegion[] = [
  { label: "Right hand", group: "hand", side: "right", injury: 12, fatigue: 30, pain: 18 },
  { label: "Chest", group: "chest", side: "center", injury: 0, fatigue: 40, pain: 6 },
];

export const timeline: TimelineEvent[] = [
  { turn: "43", label: "Aurora learned the barge cargo was people" },
  { turn: "44", label: "Crow opened the ledger under lamplight" },
  { turn: "45", label: "Crow linked the forged seals to the Sereth barge" },
  { turn: "now", label: "Crow is one honest answer away from the truth" },
];

export const memories: MemoryItem[] = [
  { id: "m1", text: "Aurora hides a second forged seal set in the hollow base of the desk lamp.", cls: "core", salience: 0.92, turn: 46, conf: "high", truth: "pinned", tag: "object", evidence: "spare seals tucked into the hollow of the lamp foot" },
  { id: "m2", text: "Inspector Crow suspects the customs ledger was altered but cannot yet prove it.", cls: "recent", salience: 0.61, turn: 45, conf: "med", truth: "active", tag: "belief", evidence: "these dates do not agree with the harbor master book" },
  { id: "m3", text: "A Sereth merchant barge cleared the gate the same night, uninspected.", cls: "recent", salience: 0.55, turn: 45, conf: "high", truth: "active", tag: "event", evidence: "a Sereth barge cleared the gate without inspection" },
  { id: "m4", text: "The barge's real cargo was people, not the silk on the manifest.", cls: "core", salience: 0.88, turn: 43, conf: "high", truth: "active", tag: "secret", evidence: "she counted the breathing in the hold and stopped at thirty" },
  { id: "m5", text: "Aurora is loyal to Lady Sereth.", cls: "schema", salience: 0.18, turn: 12, conf: "low", truth: "invalidated", tag: "relationship", evidence: "Superseded turn 43 — the payment bought silence, not loyalty" },
];

export const transcript: TranscriptMsg[] = [
  { role: "narrator", turn: 44, text: "The customs house smells of wet rope and lamp oil. Inspector Crow turns the ledger toward the lamplight, one finger tracing the column of seals you forged three nights ago. “These dates,” he says, not looking up. “They don't agree with the harbor master's book.”", sensory: { cue: "wet rope and lamp oil", memory: "the night on the Sereth barge" } },
  { role: "user", turn: 45, text: "I lean against the doorframe and let the silence stretch a beat too long. “Harbor masters drink,” I say. “Books wander.”" },
  { role: "narrator", turn: 45, text: "Crow's mouth thins. He sets the pen down with the care of a man choosing not to reach for something else. “And yet yours wandered the same night a Sereth barge cleared the gate without inspection.” He finally looks at you. “Convince me that's coincidence.”" },
];

// ---- Soul biography (innovation #4: the long arc) ----
export interface BioPoint { session: number; trust: number }
export interface BioMilestone { session: number; label: string }
export interface SoulBio {
  name: string;
  totalSessions: number;
  trust: BioPoint[];
  traumaPhase: { session: number; phase: number }[];
  identityDrift: number; // 0..1, very slow
  milestones: BioMilestone[];
}

export const auroraBio: SoulBio = {
  name: "Aurora Schwarz",
  totalSessions: 47,
  trust: [
    { session: 1, trust: 0 }, { session: 6, trust: 12 }, { session: 12, trust: 28 },
    { session: 19, trust: 22 }, { session: 27, trust: 41 }, { session: 35, trust: 38 },
    { session: 43, trust: 52 }, { session: 47, trust: 55 },
  ],
  traumaPhase: [
    { session: 1, phase: 2 }, { session: 20, phase: 3 }, { session: 41, phase: 3 },
  ],
  identityDrift: 0.14,
  milestones: [
    { session: 6, label: "First trusted the player with a real name" },
    { session: 19, label: "Betrayal at the Sereth docks — trust dipped" },
    { session: 27, label: "Chose the player over Lady Sereth's coin" },
    { session: 43, label: "Learned the barge cargo was people" },
  ],
};

// ---- Library rows ----
export interface WorldRow { id: string; name: string; genre: string; desc: string; sessions: number; chars: number }
export interface CharRow { id: string; name: string; role: string; world: string; preset: string; initial: string; self?: boolean }

export const worlds: WorldRow[] = [
  { id: "ashgate", name: "Ashgate", genre: "Political Intrigue", desc: "A drowned harbor city of merchant houses, forgeries, and old debts.", sessions: 3, chars: 5 },
  { id: "pale", name: "The Pale", genre: "Survival · Party", desc: "A frozen frontier outpost. Firewood politics and tracks that circle back.", sessions: 2, chars: 7 },
  { id: "embergard", name: "Embergard", genre: "Alt-History War", desc: "A contested front, late campaign. Two divisions out of contact; one bridge left.", sessions: 4, chars: 9 },
];

export const charRows: CharRow[] = [
  { id: "aurora", name: "Aurora Schwarz", role: "Informant · forger", world: "Ashgate", preset: "Trusting Friend", initial: "A", self: true },
  { id: "crow", name: "Inspector Valen Crow", role: "Customs investigator", world: "Ashgate", preset: "Hostile Rival", initial: "C" },
  { id: "reyes", name: "Cmdr. Reyes", role: "Field commander", world: "Embergard", preset: "Stranger", initial: "R" },
];

export const sessions: SessionRow[] = [
  { id: "ashgate", title: "The Ashgate Conspiracy", purpose: "Immersive", turns: 47, last: "12 min ago", scene: "Rain on the customs-house roof. Aurora waits for Crow to look up from the forged ledger." },
  { id: "wolves", title: "Wolves of the Pale", purpose: "Tactical", turns: 112, last: "yesterday", scene: "Firewood is down to a day. Someone swears they saw tracks circle back toward camp." },
  { id: "ember", title: "Embergard Falls", purpose: "Director", turns: 78, last: "last week", scene: "Two divisions out of contact; the only bridge across the Ember is mined." },
  { id: "salon", title: "The Verdant Salon", purpose: "Ensemble", turns: 33, last: "3 days ago", scene: "Four houses, one ballroom, and a secret that only two of them share." },
];
