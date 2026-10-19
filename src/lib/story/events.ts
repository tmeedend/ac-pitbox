// The Events screen's logic, pure (SPEC-story-mode, Events, retries and
// failure): where each event is used, its success criteria as a form, and
// the edits that keep an event readable. Whether the event is right is the
// engine's validator, run after every save.
import type { Chapter, ContentRef, EditedFile, EventType, Session, StoryEvent } from "./studio";

/** In the order the type control offers them: the spec's list. */
export const EVENT_TYPES: EventType[] = ["race", "hot_lap", "time_attack", "test_drive", "track_day"];

/** The session keys only a race has: the validator refuses them on another
 * type, so changing the type drops them. */
const RACE_ONLY = ["laps", "grid", "ai_level", "start"] as const;

/** The file of a new event: named after its id, like the engine's sample. */
export function eventFile(id: string): string {
  return `events/${id}.yaml`;
}

/** The ids of the events that read. */
export function eventIds(events: EditedFile<StoryEvent>[]): string[] {
  return events.flatMap((e) => (e.value ? [e.value.id] : []));
}

/** What the screens show for an event: its display name, or its id. */
export function eventLabels(events: EditedFile<StoryEvent>[]): Record<string, string> {
  const out: Record<string, string> = {};
  for (const e of events) if (e.value) out[e.value.id] = e.value.name ?? e.value.id;
  return out;
}

export function newEvent(id: string, name: string): StoryEvent {
  return {
    id,
    name,
    type: "race",
    session: {},
    success: [],
    on_success: { effects: [], advance: null },
    on_failure: { effects: [], pool: null, pick: 1 },
    entry_fee: null,
  };
}

/** `session` fit for `type`: a race keeps everything, another type loses
 * what only a race has. */
export function sessionFor(type: EventType, session: Session): Session {
  if (type === "race") return session;
  const out = { ...session };
  for (const key of RACE_ONLY) delete out[key];
  return out;
}

/** Where the events are used, in the order the story plays them: by chapter,
 * the events its Event nodes launch, then those Side events nodes open, then
 * those nothing uses yet. An event appears once, at its first use. */
export interface EventPlaces {
  chapters: { index: number; title: string; ids: string[] }[];
  side: string[];
  unused: string[];
}

export function eventPlaces(chapters: (Chapter | null)[], ids: string[]): EventPlaces {
  const known = new Set(ids);
  const placed = new Set<string>();
  const take = (id: string | undefined) => {
    if (!id || !known.has(id) || placed.has(id)) return false;
    placed.add(id);
    return true;
  };
  const out: EventPlaces = { chapters: [], side: [], unused: [] };
  chapters.forEach((c, index) => {
    const used = (c?.nodes ?? []).map((n) => n.event).filter(take) as string[];
    if (used.length) out.chapters.push({ index, title: c?.title ?? c?.id ?? "", ids: used });
  });
  for (const c of chapters) for (const n of c?.nodes ?? []) out.side.push(...(n.side_events?.open ?? []).filter(take));
  out.unused = ids.filter(take);
  return out;
}

/** What success takes, as the form shows it. A sentence the form has no
 * control for (`ahead_of`, until the story has drivers) is kept as it is. */
export interface Criteria {
  position: number | null;
  finished: boolean;
  /** `1:18.200`. */
  lapUnder: string | null;
  others: string[];
}

export function criteriaOf(success: string[]): Criteria {
  const out: Criteria = { position: null, finished: false, lapUnder: null, others: [] };
  for (const line of success) {
    const position = /^position\s*<=\s*(\d+)$/.exec(line);
    const lap = /^best_lap\s*<\s*(\S+)$/.exec(line);
    if (position) out.position = Number(position[1]);
    else if (lap) out.lapUnder = lap[1];
    else if (line === "finished") out.finished = true;
    else out.others.push(line);
  }
  return out;
}

/** The sentences of `c`, in the order the form shows them. */
export function successOf(c: Criteria): string[] {
  return [
    ...(c.position !== null ? [`position <= ${c.position}`] : []),
    ...(c.finished ? ["finished"] : []),
    ...(c.lapUnder ? [`best_lap < ${c.lapUnder}`] : []),
    ...c.others,
  ];
}

/** The failure pools the story's content declares, by name. */
export function poolIds(content: ContentRef[]): string[] {
  return [...new Set(content.flatMap((c) => (c.pool ? [c.pool] : [])))].sort();
}
