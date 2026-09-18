// Bridges to the three local model processes Hanna depends on.
//
// Design choice: rather than embedding whisper.cpp/llama.cpp/Piper as Rust
// libraries (heavier build, slower to iterate), Hanna shells out to their
// prebuilt CLI binaries / a local HTTP server. Swap for native bindings
// later if startup latency becomes an issue.
//
// Expected layout (see README):
//   bin/whisper-cli        (whisper.cpp)
//   bin/piper               (Piper TTS)
//   models/whisper-base.bin
//   models/hanna-voice.onnx (Piper voice)
//   llama.cpp server running separately on http://127.0.0.1:8080

use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::process::Command;

/// Where Hanna's bundled bin/ and models/ folders actually live: the
/// src-tauri project directory, NOT Tauri's app-data directory (that's a
/// separate, mostly-empty per-user storage folder meant for things the
/// app *writes*, like the memory database — never for bundled binaries).
fn resource_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// Windows' CreateProcess (which is what Rust's Command uses under the
/// hood) does NOT automatically try appending .exe the way a shell does
/// — you have to give it the exact filename, extension included.
fn platform_exe(name: &str) -> String {
    if cfg!(target_os = "windows") {
        format!("{name}.exe")
    } else {
        name.to_string()
    }
}

#[tauri::command]
pub async fn transcribe(wav_path: String, app_dir: String) -> Result<String, String> {
    let _ = app_dir; // kept for API compatibility; binaries resolve via resource_dir() instead
    let base = resource_dir();
    let whisper_bin = base.join("bin").join(platform_exe("whisper-cli"));
    let model = base.join("models/whisper-base.bin");

    let output = Command::new(whisper_bin)
        .arg("-m")
        .arg(model)
        .arg("-f")
        .arg(&wav_path)
        .arg("--no-timestamps")
        .arg("-otxt")
        .output()
        .map_err(|e| format!("failed to run whisper-cli: {e}"))?;

    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).to_string());
    }

    // whisper.cpp with -otxt writes `<wav_path>.txt`
    let txt_path = format!("{wav_path}.txt");
    std::fs::read_to_string(&txt_path)
        .map(|s| s.trim().to_string())
        .map_err(|e| e.to_string())
}

#[derive(Serialize)]
struct LlamaChatRequest {
    messages: Vec<LlamaMessage>,
    temperature: f32,
    max_tokens: u32,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct LlamaMessage {
    pub role: String,
    pub content: String,
}

#[derive(Deserialize)]
struct LlamaChatResponse {
    choices: Vec<LlamaChoice>,
}

#[derive(Deserialize)]
struct LlamaChoice {
    message: LlamaMessage,
}

/// Calls a local llama.cpp `server` (llama-server) instance, which exposes
/// an OpenAI-compatible /v1/chat/completions endpoint on localhost.
/// Start it with: `llama-server -m models/hanna-llm.gguf --port 8080`
#[tauri::command]
pub async fn ask_llm(history: Vec<LlamaMessage>) -> Result<String, String> {
    let client = reqwest::Client::new();
    let body = LlamaChatRequest {
        messages: history,
        temperature: 0.4,
        max_tokens: 512,
    };

    let resp = client
        .post("http://127.0.0.1:8080/v1/chat/completions")
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("llama.cpp server not reachable: {e}"))?;

    let parsed: LlamaChatResponse = resp.json().await.map_err(|e| e.to_string())?;
    parsed
        .choices
        .into_iter()
        .next()
        .map(|c| c.message.content)
        .ok_or_else(|| "empty response from local LLM".to_string())
}

#[derive(Serialize)]
struct EmbeddingRequest {
    input: String,
}

#[derive(Deserialize)]
struct EmbeddingResponse {
    data: Vec<EmbeddingDatum>,
}

#[derive(Deserialize)]
struct EmbeddingDatum {
    embedding: Vec<f32>,
}

/// Calls llama.cpp server's /v1/embeddings — the same server and model
/// already running for chat, so no second model to load. Used by the
/// frontend's memory layer to embed messages/facts for later recall.
#[tauri::command]
pub async fn embed_text(text: String) -> Result<Vec<f32>, String> {
    let client = reqwest::Client::new();
    let resp = client
        .post("http://127.0.0.1:8080/v1/embeddings")
        .json(&EmbeddingRequest { input: text })
        .send()
        .await
        .map_err(|e| format!("llama.cpp server not reachable: {e}"))?;

    let parsed: EmbeddingResponse = resp.json().await.map_err(|e| e.to_string())?;
    parsed
        .data
        .into_iter()
        .next()
        .map(|d| d.embedding)
        .ok_or_else(|| "empty embedding response".to_string())
}

#[tauri::command]
pub async fn speak(text: String, app_dir: String) -> Result<String, String> {
    let base = resource_dir();
    let piper_bin = base.join("bin").join(platform_exe("piper"));
    let voice = base.join("models/hanna-voice.onnx");
    let out_path = PathBuf::from(&app_dir).join("last_reply.wav");

    let mut child = Command::new(piper_bin)
        .arg("--model")
        .arg(&voice)
        .arg("--output_file")
        .arg(&out_path)
        .stdin(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| format!("failed to run piper: {e}"))?;

    use std::io::Write;
    child
        .stdin
        .as_mut()
        .ok_or("no stdin")?
        .write_all(text.as_bytes())
        .map_err(|e| e.to_string())?;

    let status = child.wait().map_err(|e| e.to_string())?;
    if !status.success() {
        return Err("piper exited with an error".into());
    }

    Ok(out_path.to_string_lossy().to_string())
}
