// Memory and personality glue.
//
// Two jobs:
//   1. buildMemoryContext() — before answering, pull relevant past facts
//      and recent-message hits into the system prompt so Hanna has
//      continuity across sessions instead of starting blank every time.
//   2. maybeDistill() — every few turns, ask the LLM itself to look at
//      the recent exchange and extract (a) any durable fact worth
//      remembering and (b) any small personality adjustment (tone,
//      running jokes, things Hanna has "decided" about how she talks to
//      you). This is what makes her feel like she develops over time
//      instead of being reset to factory defaults every session — a
//      slow-changing character sheet, not a chat log replay.
//
// Kept deliberately lightweight: one small LLM call every N turns, not
// after every single message, so it doesn't add latency to normal replies.

import {
  askLlm,
  embedText,
  getPersonality,
  recallSimilar,
  saveFact,
  saveMessage,
  updatePersonality,
  LlamaMessage,
} from "./api";

const DISTILL_EVERY_N_TURNS = 4;
let turnCount = 0;

export async function recordTurn(role: "user" | "assistant", content: string) {
  try {
    const embedding = await embedText(content);
    await saveMessage(role, content, embedding);
  } catch (err) {
    console.error("Failed to record turn to memory (continuing anyway):", err);
  }
}

export async function buildMemoryContext(userText: string): Promise<string> {
  let personality: [string, string][] = [];
  let recalled: Awaited<ReturnType<typeof recallSimilar>> = [];

  try {
    const [p, queryEmbedding] = await Promise.all([getPersonality(), embedText(userText)]);
    personality = p;
    recalled = await recallSimilar(queryEmbedding, 5);
  } catch (err) {
    console.error("Memory recall unavailable (answering without it):", err);
    return "";
  }

  const relevant = recalled.filter((r) => r.score > 0.5); // skip weak matches
  let context = "";

  if (personality.length > 0) {
    context += "What you know about your own personality and how you talk to Carlo:\n";
    context += personality.map(([k, v]) => `- ${k}: ${v}`).join("\n");
    context += "\n\n";
  }

  if (relevant.length > 0) {
    context += "Relevant things from past conversations:\n";
    context += relevant.map((r) => `- ${r.content}`).join("\n");
    context += "\n\n";
  }

  return context;
}

/**
 * Every few turns, ask the LLM to reflect on the recent exchange and
 * distill anything worth keeping long-term. Cheap on purpose: a short,
 * structured, low-temperature call — not a full re-read of everything.
 */
export async function maybeDistill(recentHistory: LlamaMessage[]) {
  turnCount += 1;
  if (turnCount % DISTILL_EVERY_N_TURNS !== 0) return;

  const transcript = recentHistory
    .slice(-DISTILL_EVERY_N_TURNS * 2)
    .map((m) => `${m.role}: ${m.content}`)
    .join("\n");

  const distillPrompt: LlamaMessage[] = [
    {
      role: "system",
      content:
        "Read this recent conversation snippet. Respond with strict JSON only, no prose, no markdown fences, in this exact shape:\n" +
        '{"facts": ["short durable fact worth remembering", ...], "personality": {"trait_key": "short value"}}\n' +
        "facts: only include genuinely durable info about Carlo (preferences, ongoing projects, recurring context) — not one-off small talk. Use an empty array if nothing qualifies.\n" +
        "personality: only include a trait if something in this exchange should change how you (Hanna) talk to Carlo going forward (tone, a joke that landed, a topic to avoid, a nickname). Use an empty object if nothing qualifies.",
    },
    { role: "user", content: transcript },
  ];

  try {
    const raw = await askLlm(distillPrompt);
    const parsed = JSON.parse(stripFences(raw)) as {
      facts?: string[];
      personality?: Record<string, string>;
    };

    for (const fact of parsed.facts ?? []) {
      const embedding = await embedText(fact);
      await saveFact(fact, embedding);
    }
    for (const [key, value] of Object.entries(parsed.personality ?? {})) {
      await updatePersonality(key, value);
    }
  } catch {
    // Distillation is best-effort — a malformed JSON response from the
    // small local model shouldn't ever interrupt the actual conversation.
  }
}

function stripFences(text: string): string {
  return text.replace(/```json|```/g, "").trim();
}
