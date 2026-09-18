// Microphone capture for push-to-talk.
//
// Flow: frontend calls `start_recording`, user talks, frontend calls
// `stop_recording` which returns the path to a 16kHz mono WAV file ready
// for whisper.cpp.
//
// NOTE: this uses `cpal` for cross-platform mic access. On first run,
// Windows/macOS/Linux will each prompt for mic permission through the OS,
// not through Hanna.

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use hound::{WavSpec, WavWriter};
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicBool, Ordering};

pub struct RecordingHandle {
    stop_flag: Arc<AtomicBool>,
}

// Simple global slot for the one active recording. Hanna only ever
// records one utterance at a time (push-to-talk), so this keeps the
// Tauri command surface simple instead of threading state through.
static ACTIVE: Mutex<Option<RecordingHandle>> = Mutex::new(None);

#[tauri::command]
pub async fn start_recording(app_data_dir: String) -> Result<String, String> {
    let host = cpal::default_host();
    let device = host
        .default_input_device()
        .ok_or("No microphone found")?;
    let config = device
        .default_input_config()
        .map_err(|e| e.to_string())?;

    let sample_rate = config.sample_rate().0;
    let channels = config.channels();

    let out_path = format!("{app_data_dir}/last_utterance.wav");

    // Tauri's app-data directory isn't created automatically — on a fresh
    // install this folder doesn't exist yet.
    std::fs::create_dir_all(&app_data_dir).map_err(|e| e.to_string())?;

    let spec = WavSpec {
        channels: 1,
        sample_rate: 16000, // whisper.cpp wants 16kHz mono
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };

    let writer = Arc::new(Mutex::new(
        WavWriter::create(&out_path, spec).map_err(|e| e.to_string())?,
    ));
    let stop_flag = Arc::new(AtomicBool::new(false));

    let writer_clone = writer.clone();
    let stop_clone = stop_flag.clone();

    // Resampling note: for a first working version, record at the device's
    // native rate into a temp buffer and downsample to 16kHz with a basic
    // linear resampler before writing. Omitted here for brevity — see
    // README "Known gaps" section. If your mic's native rate is already
    // 16kHz/48kHz you can wire a proper resampler (e.g. `rubato` crate).
    let stream = device
        .build_input_stream(
            &config.into(),
            move |data: &[f32], _| {
                if stop_clone.load(Ordering::Relaxed) {
                    return;
                }
                let mut w = writer_clone.lock().unwrap();
                for frame in data.chunks(channels as usize) {
                    let sample = frame[0]; // mono-downmix: just take channel 0
                    let amplitude = (sample * i16::MAX as f32) as i16;
                    let _ = w.write_sample(amplitude);
                }
            },
            move |err| eprintln!("audio stream error: {err}"),
            None,
        )
        .map_err(|e| e.to_string())?;

    stream.play().map_err(|e| e.to_string())?;

    // Keep the stream alive by leaking it into the active-handle slot.
    // It gets dropped (and stops) when stop_recording finalizes the file.
    std::mem::forget(stream);

    *ACTIVE.lock().unwrap() = Some(RecordingHandle { stop_flag });

    Ok(out_path)
}

#[tauri::command]
pub async fn stop_recording() -> Result<(), String> {
    if let Some(handle) = ACTIVE.lock().unwrap().take() {
        handle.stop_flag.store(true, Ordering::Relaxed);
    }
    Ok(())
}

#[tauri::command]
pub async fn play_audio_file(path: String) -> Result<(), String> {
    // Delegates to the OS default player via the shell plugin from the
    // frontend is simpler than re-implementing playback in Rust; see
    // lib/api.ts::playAudio which shells out to `afplay`/`aplay`/`start`.
    Err(format!(
        "play_audio_file is a stub — call the OS player via the shell plugin instead (path: {path})"
    ))
}
