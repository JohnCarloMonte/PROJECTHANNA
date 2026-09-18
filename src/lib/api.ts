// Thin typed layer over Tauri's invoke() so the rest of the app never
// touches raw command names / string typos.

import { invoke } from "@tauri-apps/api/core";
import { appDataDir } from "@tauri-apps/api/path";

export interface LlamaMessage {
  role: "system" | "user" | "assistant";
  content: string;
}

export interface Track {
  title: string;
  path: string;
}

let cachedAppDir: string | null = null;
async function getAppDir(): Promise<string> {
  if (!cachedAppDir) cachedAppDir = await appDataDir();
  return cachedAppDir;
}

export async function startRecording(): Promise<string> {
  const dir = await getAppDir();
  return invoke<string>("start_recording", { appDataDir: dir });
}

export async function stopRecording(): Promise<void> {
  return invoke("stop_recording");
}

export async function transcribe(wavPath: string): Promise<string> {
  const dir = await getAppDir();
  return invoke<string>("transcribe", { wavPath, appDir: dir });
}

export async function askLlm(history: LlamaMessage[]): Promise<string> {
  return invoke<string>("ask_llm", { history });
}

export async function speak(text: string): Promise<string> {
  const dir = await getAppDir();
  return invoke<string>("speak", { text, appDir: dir });
}

export async function openApp(appName: string): Promise<void> {
  return invoke("open_app", { appName });
}

export async function listLocalTracks(musicDir: string): Promise<Track[]> {
  return invoke<Track[]>("list_local_tracks", { musicDir });
}

export async function playTrack(path: string): Promise<void> {
  return invoke("play_track", { path });
}

export interface RecalledItem {
  content: string;
  score: number;
}

export async function initMemory(): Promise<void> {
  const dir = await getAppDir();
  return invoke("init_memory", { appDir: dir });
}

export async function embedText(text: string): Promise<number[]> {
  return invoke<number[]>("embed_text", { text });
}

export async function saveMessage(
  role: "user" | "assistant",
  content: string,
  embedding: number[]
): Promise<void> {
  return invoke("save_message", { role, content, embedding });
}

export async function saveFact(content: string, embedding: number[]): Promise<void> {
  return invoke("save_fact", { content, embedding });
}

export async function recallSimilar(
  queryEmbedding: number[],
  topK = 5
): Promise<RecalledItem[]> {
  return invoke<RecalledItem[]>("recall_similar", { queryEmbedding, topK });
}

export async function getPersonality(): Promise<[string, string][]> {
  return invoke<[string, string][]>("get_personality");
}

export async function updatePersonality(key: string, value: string): Promise<void> {
  return invoke("update_personality", { key, value });
}
