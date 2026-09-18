# Hanna — offline personal assistant (PC first)

A fully offline "Jarvis"-style assistant. Push-to-talk → local speech-to-text
→ intent router → local LLM or RAG over your notes → local text-to-speech.
No cloud calls, no API keys, works on a plane.

## Why this shape

- **Intent router before the LLM** (`src/lib/router.ts`): timers, opening
  apps, and playing local songs are answered instantly by pattern-matching,
  never by asking the LLM. Only genuinely open-ended questions pay the
  model's latency cost. This is the single biggest thing that makes an
  offline assistant feel usable instead of sluggish.
- **Local files instead of YouTube Music control**: there's no reliable
  offline API for driving YT Music's "play this specific track" from
  outside the app. Rather than build something that breaks on every YT
  Music update, Hanna indexes and plays your *downloaded* audio files
  directly (`src-tauri/src/commands/system.rs::list_local_tracks`). Point
  `MUSIC_DIR` in `router.ts` at wherever you keep downloaded songs.
- **Keyword RAG, not a vector DB**: `src/lib/rag.ts` chunks your notes and
  scores by word overlap. No embedding model to install yet. Swap in real
  embeddings later via llama.cpp's `/embedding` endpoint — same server,
  no new moving parts, when your notes folder gets big enough that keyword
  search starts missing things.
- **Shell-out to CLI binaries, not native bindings**: whisper.cpp,
  llama.cpp, and Piper are called as external processes rather than
  compiled into Hanna. Slower to start, much faster to iterate on and
  swap models. Revisit if per-request latency ever needs to drop further.

## Architecture

```
[Mic] --push-to-talk--> WAV (cpal, src-tauri/src/commands/audio.rs)
   -> whisper-cli (STT)                     -> models.rs::transcribe
   -> router.ts: pattern match?
        yes -> handled instantly (timer / open app / play local song)
        "about my notes" -> rag.ts -> llama-server (context-stuffed)
        otherwise         -> llama-server directly (chat)
   -> piper (TTS)                            -> models.rs::speak
   -> playback (OS default player via shell) -> App.tsx
```

## Setup

1. **Install Rust + Node** (Rust via rustup, Node 18+).
2. **Install Tauri CLI deps** for your OS — see
   https://v2.tauri.app/start/prerequisites/ (this varies by OS and isn't
   worth duplicating here since it changes over time).
3. `npm install`
4. `bash scripts/setup-models.sh` — builds whisper.cpp and llama.cpp,
   downloads the whisper model. Read the script first; it clones two
   repos and builds them, which takes a while. You still need to:
   - Download a GGUF chat model (e.g. Qwen2.5-4B-Instruct-Q4_K_M, ~2.5GB)
     from Hugging Face into `src-tauri/models/hanna-llm.gguf`
   - Build/install Piper and a voice model — see piper's own README
5. Start the LLM server in one terminal:
   ```
   ./src-tauri/bin/llama-server -m src-tauri/models/hanna-llm.gguf --port 8080
   ```
6. In another terminal: `npm run tauri dev`

## Long-term memory & personality

Hanna keeps a local SQLite database (`hanna_memory.sqlite3`, written next
to your app data — point it at your 1TB drive by changing the app data
dir in your OS settings, or symlink it there) with three tables:

- **messages** — every turn, embedded, for recall of recent context
- **facts** — durable facts *distilled* out of conversations (not raw
  transcript — see below)
- **personality** — a small key/value character sheet (tone, running
  jokes, things Hanna has picked up about how to talk to you) that drifts
  slowly instead of resetting every session

This is memory and a persistent character, not consciousness — worth
being clear-eyed about, since it's easy to read more into "she remembers
things and has a personality now" than is actually there. What's real:
she carries context and preferences forward across restarts, and that
context shapes her tone and answers. What's not: there's no inner
experience, no model of self-awareness — it's a database and a system
prompt, doing exactly what a database and a system prompt do.

**How it works** (`src/lib/personality.ts`):
1. Every message (yours and hers) gets embedded via llama.cpp server's
   `/v1/embeddings` and saved.
2. Before each LLM answer, `buildMemoryContext()` embeds your question,
   pulls the most similar past facts/messages via cosine similarity, and
   injects them plus the current personality sheet into that turn's
   system prompt.
3. Every 4 turns, `maybeDistill()` makes one small LLM call asking it to
   extract any durable fact or personality adjustment from the recent
   exchange, as strict JSON, and writes it to the database. This is what
   makes her develop over time instead of just replaying a chat log —
   deliberately cheap (one short call every few turns, not every turn) so
   it doesn't add latency to normal replies.

**To run it**: start `llama-server` with `--embeddings` enabled so the
same server handles both chat and embedding requests:
```
./src-tauri/bin/llama-server -m src-tauri/models/hanna-llm.gguf --port 8080 --embeddings
```

**Note on scale**: recall is brute-force cosine similarity in Rust, no
vector index. Completely fine for a personal assistant's memory (low
thousands of rows). If it ever gets slow, that's `memory.rs::recall_similar`
— swap in `sqlite-vec` there, schema doesn't need to change.

## Known gaps (intentionally left for you to close)

- **Mic resampling**: `audio.rs::start_recording` records at your mic's
  native sample rate but writes the WAV header as 16kHz — whisper.cpp
  wants real 16kHz audio. Add a resampler (the `rubato` crate is the
  standard choice) before this will transcribe correctly on most mics.
  Flagged clearly in the code comment where it needs to go.
- **Playback wiring**: `speak()` produces a WAV file; hooking it to
  auto-play needs a couple lines of `tauri-plugin-shell` calling
  `afplay`/`aplay`/`start` depending on OS. Stubbed in `App.tsx` where the
  path comes back.
- **Wake word**: not implemented. This is push-to-talk only right now.
  Add Picovoice Porcupine (train a custom "Hanna" keyword) once the core
  loop feels solid — don't add it first, it's the least useful part to
  debug against a shaky pipeline.
- **"Control existing apps"**: `open_app` launches apps by name. Deeper
  control (clicking things inside other apps) needs OS automation
  (AppleScript on macOS, UI Automation on Windows, `xdotool` on Linux) —
  add allow-listed commands per app as you actually need them. Resist the
  urge to let the LLM generate arbitrary automation commands; keep it to
  a fixed menu of things you've explicitly wired up.

## Roadmap to Android

Once this feels good on PC:
- STT: `whisper.rn` (React Native binding of whisper.cpp)
- LLM: `llama.rn`, same GGUF model, quantized smaller if needed for phone RAM
- TTS: Android's native `TextToSpeech` with offline voice pack (simplest)
  or llama.rn's experimental on-device TTS
- Wake word: Porcupine has a React Native SDK
- "Open app" / "play song": Android Intents for anything that exposes
  them; anything else needs Accessibility Service — a real project on its
  own, don't scope it into v1
