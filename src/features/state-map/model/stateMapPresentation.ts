import type {
  SessionStateMap,
  StateMapCharacterItem,
  StateMapObjectItem,
  StateMapRelationshipItem,
} from "../../../tauri";

export type PresentedStateMapItem<T> = T & {
  identity_key: string;
  session_count: number;
};

export type StateMapPresentation = {
  characters: Array<PresentedStateMapItem<StateMapCharacterItem>>;
  relationships: Array<PresentedStateMapItem<StateMapRelationshipItem>>;
  objects: Array<PresentedStateMapItem<StateMapObjectItem>>;
};

export function buildStateMapPresentation(stateMap: SessionStateMap): StateMapPresentation {
  return {
    characters: mergeLatest(stateMap.characters, characterIdentity, presentCharacter),
    relationships: mergeLatest(
      stateMap.relationships,
      relationshipIdentity,
      presentRelationship,
    ),
    objects: mergeLatest(stateMap.objects, objectIdentity, presentObject),
  };
}

export function humanizeStateMapText(value: string): string {
  if (!value.trim()) return "";
  return value
    .replace(/\b(?:preset_male|preset_female|default_player|user)\b/gi, "You")
    .replace(/\bsession_clone\b/gi, "Soul")
    .replace(/\brelationship\b/gi, "Player")
    .replace(/\bunknown\b/gi, "Unassigned")
    .replace(/_/g, " ");
}

function mergeLatest<T>(
  items: T[],
  identity: (item: T) => string,
  present: (item: T) => T,
): Array<PresentedStateMapItem<T>> {
  const merged = new Map<string, PresentedStateMapItem<T>>();
  for (const item of items) {
    const key = identity(item);
    const existing = merged.get(key);
    if (existing) {
      existing.session_count += 1;
      continue;
    }
    merged.set(key, {
      ...present(item),
      identity_key: key,
      session_count: 1,
    });
  }
  return [...merged.values()];
}

function characterIdentity(item: StateMapCharacterItem) {
  return normalizeIdentity(humanizeStateMapText(item.name));
}

function relationshipIdentity(item: StateMapRelationshipItem) {
  return `${normalizeIdentity(item.soul_name)}->${normalizeIdentity(
    humanizeStateMapText(item.target),
  )}`;
}

function objectIdentity(item: StateMapObjectItem) {
  return normalizeIdentity(item.name);
}

function presentCharacter(item: StateMapCharacterItem): StateMapCharacterItem {
  return {
    ...item,
    name: humanizeStateMapText(item.name),
    role: humanizeStateMapText(item.role),
  };
}

function presentRelationship(item: StateMapRelationshipItem): StateMapRelationshipItem {
  return {
    ...item,
    soul_name: humanizeStateMapText(item.soul_name),
    target: humanizeStateMapText(item.target),
    love_type: humanizeStateMapText(item.love_type),
  };
}

function presentObject(item: StateMapObjectItem): StateMapObjectItem {
  return {
    ...item,
    kind: humanizeStateMapText(item.kind),
    owner: humanizeStateMapText(item.owner),
    status: humanizeStateMapText(item.status),
  };
}

function normalizeIdentity(value: string) {
  return value.trim().toLocaleLowerCase().replace(/\s+/g, " ");
}
