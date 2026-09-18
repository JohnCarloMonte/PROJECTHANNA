// OS-level actions that don't need the LLM at all: opening apps, listing
// and playing local music files, and running whatever else Hanna's intent
// router decides is a "device command" rather than a "question".
//
// Kept intentionally simple and allow-listed. Do not turn this into a
// generic "run arbitrary shell command from LLM output" bridge — that is
// a straightforward path to the LLM being tricked (by a note it read, a
// file it indexed, whatever) into running something you didn't ask for.

use serde::Serialize;
use std::path::Path;
use walkdir::WalkDir;

#[derive(Serialize)]
pub struct Track {
    pub title: String,
    pub path: String,
}

#[tauri::command]
pub async fn open_app(app_name: String) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg("-a")
            .arg(&app_name)
            .spawn()
            .map_err(|e| e.to_string())?;
    }
    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("cmd")
            .args(["/C", "start", "", &app_name])
            .spawn()
            .map_err(|e| e.to_string())?;
    }
    #[cfg(target_os = "linux")]
    {
        std::process::Command::new(&app_name)
            .spawn()
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// Scans a music folder for playable audio files. This is the offline,
/// reliable alternative to trying to remote-control YouTube Music — see
/// README for why. Hanna plays these directly instead.
#[tauri::command]
pub async fn list_local_tracks(music_dir: String) -> Result<Vec<Track>, String> {
    let exts = ["mp3", "flac", "wav", "m4a", "ogg"];
    let mut tracks = Vec::new();

    for entry in WalkDir::new(&music_dir)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let path = entry.path();
        if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
            if exts.contains(&ext.to_lowercase().as_str()) {
                let title = path
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("Unknown")
                    .to_string();
                tracks.push(Track {
                    title,
                    path: path.to_string_lossy().to_string(),
                });
            }
        }
    }
    Ok(tracks)
}

#[tauri::command]
pub async fn play_track(path: String) -> Result<(), String> {
    if !Path::new(&path).exists() {
        return Err(format!("track not found: {path}"));
    }
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("afplay")
            .arg(&path)
            .spawn()
            .map_err(|e| e.to_string())?;
    }
    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("cmd")
            .args(["/C", "start", "", &path])
            .spawn()
            .map_err(|e| e.to_string())?;
    }
    #[cfg(target_os = "linux")]
    {
        std::process::Command::new("aplay")
            .arg(&path)
            .spawn()
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}
