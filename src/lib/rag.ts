// Minimal offline RAG (retrieval-augmented generation) over a local notes
// folder. Deliberately simple: no vector database, no embedding model —
// just chunking + keyword/BM25-style overlap scoring. This is enough for
// a personal notes folder (dozens to low hundreds of files) and has zero
// extra moving parts to install.
//
// If your notes grow past a few thousand files and keyword search starts
// missing things, swap `scoreChunk` for real embeddings — llama.cpp's
// server also serves an /embedding endpoint using the same model, so it's
// a drop-in upgrade later rather than a rewrite.

import { readTextFile, readDir } from "@tauri-apps/plugin-fs";
import { askLlm, LlamaMessage } from "./api";

interface Chunk {
  file: string;
  text: string;
}

let indexCache: Chunk[] | null = null;
let indexedDir: string | null = null;

const CHUNK_SIZE = 800; // characters per chunk, rough approximation of ~200 tokens

export async function buildIndex(notesDir: string): Promise<number> {
  const entries = await readDir(notesDir);
  const chunks: Chunk[] = [];

  for (const entry of entries) {
    if (!entry.isFile) continue;
    if (!/\.(md|txt)$/i.test(entry.name ?? "")) continue;

    const path = `${notesDir}/${entry.name}`;
    const content = await readTextFile(path);
    for (let i = 0; i < content.length; i += CHUNK_SIZE) {
      chunks.push({
        file: entry.name ?? path,
        text: content.slice(i, i + CHUNK_SIZE),
      });
    }
  }

  indexCache = chunks;
  indexedDir = notesDir;
  return chunks.length;
}

function scoreChunk(query: string, chunk: string): number {
  const queryWords = new Set(
    query
      .toLowerCase()
      .split(/\W+/)
      .filter((w) => w.length > 2)
  );
  const chunkWords = chunk.toLowerCase().split(/\W+/);
  let score = 0;
  for (const w of chunkWords) {
    if (queryWords.has(w)) score += 1;
  }
  return score;
}

export async function answerFromNotes(
  question: string,
  notesDir: string,
  topK = 4
): Promise<string> {
  if (!indexCache || indexedDir !== notesDir) {
    await buildIndex(notesDir);
  }
  if (!indexCache || indexCache.length === 0) {
    return "I don't have any notes indexed yet — check the notes folder path in settings.";
  }

  const ranked = indexCache
    .map((c) => ({ ...c, score: scoreChunk(question, c.text) }))
    .filter((c) => c.score > 0)
    .sort((a, b) => b.score - a.score)
    .slice(0, topK);

  if (ranked.length === 0) {
    return "I couldn't find anything in your notes about that.";
  }

  const context = ranked
    .map((c) => `[From ${c.file}]\n${c.text}`)
    .join("\n\n---\n\n");

  const history: LlamaMessage[] = [
    {
      role: "system",
      content:
        "You are Hanna, a personal offline assistant. Answer the question using ONLY the context below from the user's own notes. If the answer isn't in the context, say so plainly instead of guessing.\n\nContext:\n" +
        context,
    },
    { role: "user", content: question },
  ];

  return askLlm(history);
}
