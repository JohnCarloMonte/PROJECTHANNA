// Intent router: decides whether an utterance can be handled instantly
// with a deterministic rule (timer, open app, play song) or needs to go
// to the local LLM. This is the single most important file for making
// Hanna *feel* responsive — every request that skips the LLM answers in
// well under a second instead of several.
//
// Deliberately dumb pattern matching rather than an LLM-based classifier:
// classifying "what should I do with this sentence" via another model
// call would defeat the point (latency) and add another failure mode.
// Expand these patterns as you find gaps; don't reach for an LLM router
// until simple patterns genuinely can't keep up.

import { listLocalTracks, openApp, playTrack } from "./api";

export type RouterResult =
  | { kind: "handled"; reply: string }
  | { kind: "llm"; prompt: string }
  | { kind: "rag"; prompt: string };

const OPEN_APP_RE = /^(?:open|launch|start)\s+(.+)$/i;
const PLAY_SONG_RE = /^(?:play)\s+(.+)$/i;
const TIMER_RE = /^(?:set (?:a )?)?timer(?: for)?\s+(\d+)\s*(seconds?|minutes?|mins?|hours?|hrs?)$/i;
const NOTES_RE = /\b(my notes?|my files?|what did i (write|save)|according to my)\b/i;

// Configure this to wherever your downloaded music actually lives.
const MUSIC_DIR = "/home/carlo/Music";

export async function routeUtterance(text: string): Promise<RouterResult> {
  const trimmed = text.trim();

  const timerMatch = trimmed.match(TIMER_RE);
  if (timerMatch) {
    const [, amountStr, unit] = timerMatch;
    const amount = parseInt(amountStr, 10);
    const ms = toMilliseconds(amount, unit);
    scheduleTimer(ms, `${amount} ${unit}`);
    return { kind: "handled", reply: `Timer set for ${amount} ${unit}.` };
  }

  const openMatch = trimmed.match(OPEN_APP_RE);
  if (openMatch) {
    const appName = openMatch[1].trim();
    await openApp(appName);
    return { kind: "handled", reply: `Opening ${appName}.` };
  }

  const playMatch = trimmed.match(PLAY_SONG_RE);
  if (playMatch) {
    const query = playMatch[1].trim().toLowerCase();
    const tracks = await listLocalTracks(MUSIC_DIR);
    const found = tracks.find((t) => t.title.toLowerCase().includes(query));
    if (found) {
      await playTrack(found.path);
      return { kind: "handled", reply: `Playing ${found.title}.` };
    }
    return {
      kind: "handled",
      reply: `I couldn't find "${query}" in your downloaded music.`,
    };
  }

  if (NOTES_RE.test(trimmed)) {
    return { kind: "rag", prompt: trimmed };
  }

  return { kind: "llm", prompt: trimmed };
}

function toMilliseconds(amount: number, unit: string): number {
  if (/^h/i.test(unit)) return amount * 3600_000;
  if (/^m/i.test(unit)) return amount * 60_000;
  return amount * 1000;
}

function scheduleTimer(ms: number, label: string) {
  setTimeout(() => {
    new Notification("Hanna", { body: `Timer done: ${label}` });
  }, ms);
}
