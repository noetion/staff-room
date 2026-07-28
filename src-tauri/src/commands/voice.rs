use crate::*;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{SampleFormat, StreamConfig};
use std::io::Write as _;
use std::process::Stdio;

const MAX_CAPTURE_SECONDS: u64 = 30;
const MAX_TRANSCRIPTION_SECONDS: u64 = 120;
const TARGET_SAMPLE_RATE: u32 = 16_000;

/// Test seam for the fake-provider suite. Honoured in debug builds only: a release build
/// must not let an inherited environment variable choose which executable gets run.
fn debug_path_override(key: &str) -> Option<Option<PathBuf>> {
    if !cfg!(debug_assertions) {
        return None;
    }
    let path = std::env::var_os(key).map(PathBuf::from)?;
    Some(path.is_file().then_some(path))
}

fn installed_voice_model(app: &AppHandle) -> Result<Option<PathBuf>, String> {
    if let Some(resolved) = debug_path_override("AGENT_ROOM_WHISPER_MODEL") {
        return Ok(resolved);
    }
    let path = app
        .path()
        .app_local_data_dir()
        .map_err(|error| error.to_string())?
        .join("voice")
        .join("ggml-model.bin");
    Ok(path.is_file().then_some(path))
}

fn installed_voice_engine(app: &AppHandle) -> Result<Option<PathBuf>, String> {
    if let Some(resolved) = debug_path_override("AGENT_ROOM_WHISPER_CLI") {
        return Ok(resolved);
    }
    let local = app
        .path()
        .app_local_data_dir()
        .map_err(|error| error.to_string())?
        .join("voice")
        .join(if cfg!(windows) {
            "whisper-cli.exe"
        } else {
            "whisper-cli"
        });
    if local.is_file() {
        return Ok(Some(local));
    }
    if cfg!(debug_assertions) {
        return Ok(which::which("whisper-cli").ok());
    }
    Ok(None)
}

/// Single source of truth for readiness. Every command that returns a `VoiceStatus`
/// goes through here so a status can never disagree with itself.
fn voice_snapshot(app: &AppHandle, recording: bool) -> Result<VoiceStatus, String> {
    let model = installed_voice_model(app)?;
    let engine = installed_voice_engine(app)?;
    let microphone = cpal::default_host().default_input_device().is_some();
    let (available, detail) = match (&model, &engine, microphone) {
        (Some(_), Some(_), true) => (
            true,
            "Local push-to-talk is ready. Audio remains on this device.".to_owned(),
        ),
        (None, _, _) => (
            false,
            "Choose a local whisper.cpp ggml model in Settings to enable dictation.".to_owned(),
        ),
        (_, None, _) => (
            false,
            "Choose a local whisper.cpp CLI executable in Settings to enable dictation.".to_owned(),
        ),
        (_, _, false) => (
            false,
            "No default microphone is available to Agent Room.".to_owned(),
        ),
    };
    Ok(VoiceStatus {
        available,
        recording,
        model_path: model.map(|path| path.to_string_lossy().into_owned()),
        detail,
        max_seconds: MAX_CAPTURE_SECONDS,
    })
}

/// Turns a backend audio error into something the person can act on. Windows silently
/// denies microphone access to desktop apps until the privacy toggle is set, and the
/// raw HRESULT tells nobody that.
fn microphone_error(context: &str, error: impl std::fmt::Display) -> String {
    let text = error.to_string();
    let lowered = text.to_lowercase();
    if lowered.contains("0x80070005")
        || lowered.contains("access is denied")
        || lowered.contains("denied")
        || lowered.contains("permission")
    {
        if cfg!(windows) {
            return "Windows is blocking microphone access for Agent Room. Open Settings > Privacy & security > Microphone, turn on \"Let desktop apps access your microphone\", then try again.".to_owned();
        }
        return "The system denied microphone access to Agent Room. Grant microphone permission in your system settings, then try again.".to_owned();
    }
    format!("{context}: {text}")
}

#[tauri::command]
pub(crate) async fn voice_status(
    app: AppHandle,
    runtime: State<'_, RuntimeState>,
) -> Result<VoiceStatus, String> {
    let recording = runtime
        .voice_capture
        .lock()
        .await
        .as_ref()
        .is_some_and(|session| !session.finished.load(std::sync::atomic::Ordering::SeqCst));
    voice_snapshot(&app, recording)
}

#[tauri::command]
pub(crate) async fn voice_pick_engine(app: AppHandle) -> Result<VoiceStatus, String> {
    let Some(selected) = app.dialog().file().blocking_pick_file() else {
        return Err("No local transcription engine was selected.".to_owned());
    };
    let source = PathBuf::from(selected.to_string());
    if !source.is_file() {
        return Err("Choose the local whisper.cpp `whisper-cli` executable.".to_owned());
    }
    let directory = app
        .path()
        .app_local_data_dir()
        .map_err(|error| error.to_string())?
        .join("voice");
    std::fs::create_dir_all(&directory)
        .map_err(|error| format!("Cannot create the local voice directory: {error}"))?;
    let destination = directory.join(if cfg!(windows) {
        "whisper-cli.exe"
    } else {
        "whisper-cli"
    });
    let temporary = directory.join(format!(".engine-{}.tmp", Uuid::new_v4()));
    std::fs::copy(&source, &temporary)
        .map_err(|error| format!("Cannot copy the local transcription engine: {error}"))?;
    if destination.exists() {
        std::fs::remove_file(&destination)
            .map_err(|error| format!("Cannot replace the local transcription engine: {error}"))?;
    }
    std::fs::rename(&temporary, &destination)
        .map_err(|error| format!("Cannot install the local transcription engine: {error}"))?;
    voice_snapshot(&app, false)
}

#[tauri::command]
pub(crate) async fn voice_pick_model(app: AppHandle) -> Result<VoiceStatus, String> {
    let Some(selected) = app
        .dialog()
        .file()
        .add_filter("whisper.cpp model", &["bin"])
        .blocking_pick_file()
    else {
        return Err("No voice model was selected.".to_owned());
    };
    let source = PathBuf::from(selected.to_string());
    let metadata = std::fs::metadata(&source)
        .map_err(|error| format!("Cannot read the selected voice model: {error}"))?;
    if !metadata.is_file() || metadata.len() < 1_000_000 || metadata.len() > 2_500_000_000 {
        return Err("Choose a whisper.cpp ggml `.bin` model between 1 MB and 2.5 GB.".to_owned());
    }
    let directory = app
        .path()
        .app_local_data_dir()
        .map_err(|error| error.to_string())?
        .join("voice");
    std::fs::create_dir_all(&directory)
        .map_err(|error| format!("Cannot create the local voice directory: {error}"))?;
    let destination = directory.join("ggml-model.bin");
    let temporary = directory.join(format!(".model-{}.tmp", Uuid::new_v4()));
    std::fs::copy(&source, &temporary)
        .map_err(|error| format!("Cannot copy the local voice model: {error}"))?;
    if destination.exists() {
        std::fs::remove_file(&destination)
            .map_err(|error| format!("Cannot replace the local voice model: {error}"))?;
    }
    std::fs::rename(&temporary, &destination)
        .map_err(|error| format!("Cannot install the local voice model: {error}"))?;
    voice_snapshot(&app, false)
}

#[tauri::command]
pub(crate) async fn voice_start(
    app: AppHandle,
    runtime: State<'_, RuntimeState>,
) -> Result<(), String> {
    if installed_voice_model(&app)?.is_none() || installed_voice_engine(&app)?.is_none() {
        return Err(
            "Choose a local whisper.cpp CLI and ggml model in Settings before dictating."
                .to_owned(),
        );
    }
    let mut capture = runtime.voice_capture.lock().await;
    // A renderer reload while recording leaves a session nobody will ever stop. Reclaim
    // any session whose capture thread has exited, or that has outlived the hard cap,
    // instead of locking dictation out until the app restarts.
    let active = capture.as_ref().is_some_and(|existing| {
        !existing.finished.load(std::sync::atomic::Ordering::SeqCst)
            && existing.started.elapsed() < Duration::from_secs(MAX_CAPTURE_SECONDS + 2)
    });
    if active {
        return Err("Voice capture is already active.".to_owned());
    }
    if let Some(stale) = capture.take() {
        let _ = stale.stop.send(());
    }
    let (stop_sender, stop_receiver) = std::sync::mpsc::channel();
    let (result_sender, result_receiver) = tokio::sync::oneshot::channel();
    let (ready_sender, ready_receiver) = tokio::sync::oneshot::channel::<Result<(), String>>();
    let finished = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let thread_finished = finished.clone();
    let level_app = app.clone();
    std::thread::spawn(move || {
        let mut ready = Some(ready_sender);
        let result = capture_microphone(&level_app, stop_receiver, &mut ready);
        if let Some(sender) = ready.take() {
            let _ = sender.send(match result.as_ref() {
                Ok(_) => Ok(()),
                Err(error) => Err(error.clone()),
            });
        }
        thread_finished.store(true, std::sync::atomic::Ordering::SeqCst);
        let _ = result_sender.send(result);
    });
    // Awaited, not blocked on: this used to park a Tokio worker for up to five seconds
    // while holding the capture lock.
    timeout(Duration::from_secs(5), ready_receiver)
        .await
        .map_err(|_| "The microphone did not become ready in time.".to_owned())?
        .map_err(|_| "The microphone capture ended unexpectedly.".to_owned())??;
    *capture = Some(VoiceCaptureSession {
        stop: stop_sender,
        result: result_receiver,
        started: Instant::now(),
        finished,
    });
    Ok(())
}

#[tauri::command]
pub(crate) async fn voice_stop(
    app: AppHandle,
    runtime: State<'_, RuntimeState>,
    cancel: Option<bool>,
) -> Result<VoiceTranscription, String> {
    let session = runtime
        .voice_capture
        .lock()
        .await
        .take()
        .ok_or_else(|| "Voice capture is not active.".to_owned())?;
    let _ = session.stop.send(());
    let mut audio = session
        .result
        .await
        .map_err(|_| "The microphone capture ended unexpectedly.".to_owned())??;
    if cancel.unwrap_or(false) {
        let duration_ms = audio.duration_ms;
        // Discarded audio is overwritten, not just dropped.
        audio.samples.iter_mut().for_each(|sample| *sample = 0.0);
        audio.samples.clear();
        return Ok(VoiceTranscription {
            text: String::new(),
            duration_ms,
        });
    }
    if audio.samples.is_empty() {
        return Err(
            "No audio reached Agent Room from the microphone. Check that the right input device is selected in your system settings, then try again."
                .to_owned(),
        );
    }
    if audio.duration_ms < 200 {
        return Err("Hold the microphone a little longer before releasing.".to_owned());
    }
    let model = installed_voice_model(&app)?
        .ok_or_else(|| "The local voice model is no longer available.".to_owned())?;
    let engine = installed_voice_engine(&app)?
        .ok_or_else(|| "The local transcription engine is no longer available.".to_owned())?;
    let duration_ms = audio.duration_ms;
    let cache = app
        .path()
        .app_cache_dir()
        .map_err(|error| error.to_string())?
        .join("voice");
    let text = tokio::task::spawn_blocking(move || transcribe(engine, model, cache, audio))
        .await
        .map_err(|error| format!("Local transcription task failed: {error}"))??;
    Ok(VoiceTranscription { text, duration_ms })
}

/// Peak-normalised so ordinary speech fills most of the meter. Conversational RMS on a
/// desktop microphone sits near 0.05-0.20, so a raw value would look like silence.
const LEVEL_GAIN: f32 = 4.0;

fn capture_microphone(
    app: &AppHandle,
    stop: std::sync::mpsc::Receiver<()>,
    ready: &mut Option<tokio::sync::oneshot::Sender<Result<(), String>>>,
) -> Result<CapturedAudio, String> {
    let host = cpal::default_host();
    let device = host
        .default_input_device()
        .ok_or_else(|| "No default microphone is available.".to_owned())?;
    let supported = device
        .default_input_config()
        .map_err(|error| format!("Cannot read the microphone format: {error}"))?;
    let sample_format = supported.sample_format();
    let config: StreamConfig = supported.into();
    let sample_rate = config.sample_rate;
    let channels = config.channels;
    let max_samples = sample_rate as usize * channels as usize * MAX_CAPTURE_SECONDS as usize;
    let samples = std::sync::Arc::new(std::sync::Mutex::new(Vec::<f32>::with_capacity(
        max_samples,
    )));
    // The audio callback must stay allocation- and IPC-free, so it only stores the block
    // RMS here. The capture loop below is what emits, at its own 20 Hz tick.
    let level = std::sync::Arc::new(std::sync::atomic::AtomicU32::new(0));
    let failure = std::sync::Arc::new(std::sync::Mutex::new(None::<String>));
    let failure_callback = failure.clone();
    let error_callback = move |error| {
        if let Ok(mut value) = failure_callback.lock() {
            *value = Some(format!("Microphone capture failed: {error}"));
        }
    };
    let stream = match sample_format {
        SampleFormat::F32 => {
            let samples = samples.clone();
            let level = level.clone();
            device.build_input_stream(
                &config,
                move |data: &[f32], _| {
                    append_samples(&samples, &level, data.iter().copied(), max_samples)
                },
                error_callback,
                None,
            )
        }
        SampleFormat::I16 => {
            let samples = samples.clone();
            let level = level.clone();
            device.build_input_stream(
                &config,
                move |data: &[i16], _| {
                    append_samples(
                        &samples,
                        &level,
                        data.iter().map(|sample| *sample as f32 / i16::MAX as f32),
                        max_samples,
                    )
                },
                error_callback,
                None,
            )
        }
        SampleFormat::U16 => {
            let samples = samples.clone();
            let level = level.clone();
            device.build_input_stream(
                &config,
                move |data: &[u16], _| {
                    append_samples(
                        &samples,
                        &level,
                        data.iter()
                            .map(|sample| (*sample as f32 - 32_768.0) / 32_768.0),
                        max_samples,
                    )
                },
                error_callback,
                None,
            )
        }
        format => return Err(format!("Unsupported microphone sample format: {format:?}")),
    }
    .map_err(|error| microphone_error("Cannot open the microphone", error))?;
    stream
        .play()
        .map_err(|error| microphone_error("Cannot start the microphone", error))?;
    if let Some(sender) = ready.take() {
        let _ = sender.send(Ok(()));
    }
    let started = Instant::now();
    loop {
        match stop.recv_timeout(Duration::from_millis(50)) {
            // A dropped sender means the command that owned this session went away.
            // Treat it as a stop, otherwise the microphone stays live until the cap.
            Ok(()) | Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
        }
        emit_voice_level(
            app,
            f32::from_bits(level.load(std::sync::atomic::Ordering::Relaxed)),
        );
        if started.elapsed() >= Duration::from_secs(MAX_CAPTURE_SECONDS) {
            break;
        }
        if failure
            .lock()
            .ok()
            .and_then(|value| value.clone())
            .is_some()
        {
            break;
        }
    }
    drop(stream);
    // Whatever happens next, the meter must not be left showing a live signal.
    emit_voice_level(app, 0.0);
    if let Some(error) = failure.lock().ok().and_then(|value| value.clone()) {
        return Err(error);
    }
    let samples = samples.lock().map_err(|error| error.to_string())?.clone();
    Ok(CapturedAudio {
        duration_ms: audio_duration_ms(samples.len(), channels, sample_rate),
        samples,
        sample_rate,
        channels,
    })
}

/// Duration of the audio actually captured, not of the wall clock the button was held.
/// The two diverge whenever the backend is slow to deliver its first buffer, which is
/// routine on Windows, and the difference decides whether a real utterance is rejected.
fn audio_duration_ms(samples: usize, channels: u16, sample_rate: u32) -> u64 {
    let frames = samples / channels.max(1) as usize;
    frames as u64 * 1000 / sample_rate.max(1) as u64
}

fn normalize_level(raw: f32) -> f32 {
    if !raw.is_finite() {
        return 0.0;
    }
    (raw * LEVEL_GAIN).clamp(0.0, 1.0)
}

fn emit_voice_level(app: &AppHandle, raw: f32) {
    let _ = app.emit(
        "voice-level",
        VoiceLevel {
            level: normalize_level(raw),
        },
    );
}

fn append_samples(
    destination: &std::sync::Arc<std::sync::Mutex<Vec<f32>>>,
    level: &std::sync::Arc<std::sync::atomic::AtomicU32>,
    source: impl Iterator<Item = f32>,
    limit: usize,
) {
    if let Ok(mut samples) = destination.lock() {
        let start = samples.len();
        let remaining = limit.saturating_sub(start);
        samples.extend(source.take(remaining));
        let appended = &samples[start..];
        if !appended.is_empty() {
            let mean_square =
                appended.iter().map(|sample| sample * sample).sum::<f32>() / appended.len() as f32;
            level.store(
                mean_square.sqrt().to_bits(),
                std::sync::atomic::Ordering::Relaxed,
            );
        }
    }
}

fn transcribe(
    engine: PathBuf,
    model: PathBuf,
    cache: PathBuf,
    audio: CapturedAudio,
) -> Result<String, String> {
    let mono = to_mono(&audio.samples, audio.channels);
    let samples = resample_linear(&mono, audio.sample_rate, TARGET_SAMPLE_RATE);
    std::fs::create_dir_all(&cache)
        .map_err(|error| format!("Cannot create the temporary voice directory: {error}"))?;
    // whisper.cpp reads a file, so audio does briefly touch disk. If the app is killed
    // mid-transcription the scratch file survives, so sweep anything left behind.
    sweep_voice_cache(&cache);
    let id = Uuid::new_v4().to_string();
    let wav = cache.join(format!("{id}.wav"));
    let output_base = cache.join(&id);
    let output_text = cache.join(format!("{id}.txt"));
    let result = (|| {
        write_pcm_wav(&wav, &samples, TARGET_SAMPLE_RATE)?;
        let mut command = StdCommand::new(engine);
        hide_std_command_window(&mut command);
        let mut child = command
            .arg("-m")
            .arg(&model)
            .arg("-f")
            .arg(&wav)
            .arg("-otxt")
            .arg("-of")
            .arg(&output_base)
            .arg("-np")
            .arg("-nt")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|error| format!("Cannot start local transcription: {error}"))?;
        let started = Instant::now();
        let output = loop {
            if child
                .try_wait()
                .map_err(|error| format!("Cannot monitor local transcription: {error}"))?
                .is_some()
            {
                break child
                    .wait_with_output()
                    .map_err(|error| format!("Cannot collect local transcription: {error}"))?;
            }
            if started.elapsed() >= Duration::from_secs(MAX_TRANSCRIPTION_SECONDS) {
                let _ = child.kill();
                let _ = child.wait();
                return Err("Local transcription exceeded the two-minute limit.".to_owned());
            }
            std::thread::sleep(Duration::from_millis(50));
        };
        let transcript = std::fs::read_to_string(&output_text)
            .unwrap_or_else(|_| String::from_utf8_lossy(&output.stdout).into_owned());
        if !output.status.success() {
            return Err(format!(
                "Local transcription failed: {}",
                truncate_utf8(&String::from_utf8_lossy(&output.stderr), 500)
            ));
        }
        let transcript = transcript.split_whitespace().collect::<Vec<_>>().join(" ");
        if transcript.is_empty() {
            return Err("No speech was detected. Your typed draft was left unchanged.".to_owned());
        }
        Ok(transcript)
    })();
    let _ = std::fs::remove_file(&wav);
    let _ = std::fs::remove_file(&output_text);
    result
}

fn sweep_voice_cache(cache: &Path) {
    let Ok(entries) = std::fs::read_dir(cache) else {
        return;
    };
    let now = std::time::SystemTime::now();
    for entry in entries.flatten() {
        let stale = entry
            .metadata()
            .and_then(|metadata| metadata.modified())
            .ok()
            .and_then(|modified| now.duration_since(modified).ok())
            .is_some_and(|age| age > Duration::from_secs(3_600));
        if stale {
            let _ = std::fs::remove_file(entry.path());
        }
    }
}

fn write_pcm_wav(path: &Path, samples: &[f32], sample_rate: u32) -> Result<(), String> {
    let data_len = samples
        .len()
        .checked_mul(2)
        .and_then(|value| u32::try_from(value).ok())
        .ok_or_else(|| "Voice capture is too large to encode.".to_owned())?;
    let mut file = std::fs::File::create(path)
        .map_err(|error| format!("Cannot buffer voice audio: {error}"))?;
    file.write_all(b"RIFF")
        .and_then(|_| file.write_all(&(36_u32 + data_len).to_le_bytes()))
        .and_then(|_| file.write_all(b"WAVEfmt "))
        .and_then(|_| file.write_all(&16_u32.to_le_bytes()))
        .and_then(|_| file.write_all(&1_u16.to_le_bytes()))
        .and_then(|_| file.write_all(&1_u16.to_le_bytes()))
        .and_then(|_| file.write_all(&sample_rate.to_le_bytes()))
        .and_then(|_| file.write_all(&(sample_rate * 2).to_le_bytes()))
        .and_then(|_| file.write_all(&2_u16.to_le_bytes()))
        .and_then(|_| file.write_all(&16_u16.to_le_bytes()))
        .and_then(|_| file.write_all(b"data"))
        .and_then(|_| file.write_all(&data_len.to_le_bytes()))
        .map_err(|error| format!("Cannot encode voice audio: {error}"))?;
    for sample in samples {
        let pcm = (sample.clamp(-1.0, 1.0) * i16::MAX as f32) as i16;
        file.write_all(&pcm.to_le_bytes())
            .map_err(|error| format!("Cannot encode voice audio: {error}"))?;
    }
    Ok(())
}

fn to_mono(samples: &[f32], channels: u16) -> Vec<f32> {
    let channels = channels.max(1) as usize;
    samples
        .chunks_exact(channels)
        .map(|frame| frame.iter().sum::<f32>() / channels as f32)
        .collect()
}

fn resample_linear(samples: &[f32], source_rate: u32, target_rate: u32) -> Vec<f32> {
    if samples.is_empty() || source_rate == target_rate {
        return samples.to_vec();
    }
    let output_len = (samples.len() as u64 * target_rate as u64 / source_rate as u64) as usize;
    (0..output_len)
        .map(|index| {
            let position = index as f64 * source_rate as f64 / target_rate as f64;
            let lower = position.floor() as usize;
            let upper = (lower + 1).min(samples.len() - 1);
            let fraction = (position - lower as f64) as f32;
            samples[lower] * (1.0 - fraction) + samples[upper] * fraction
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stereo_audio_is_mixed_to_mono() {
        assert_eq!(to_mono(&[1.0, -1.0, 0.5, 0.5], 2), vec![0.0, 0.5]);
    }

    #[test]
    fn resampling_preserves_ends_and_expected_length() {
        let result = resample_linear(&[0.0, 1.0, 0.0, -1.0], 4, 8);
        assert_eq!(result.len(), 8);
        assert_eq!(result[0], 0.0);
    }

    #[test]
    fn duration_is_measured_from_captured_frames_not_wall_clock() {
        // One second of stereo audio at 48 kHz.
        assert_eq!(audio_duration_ms(96_000, 2, 48_000), 1_000);
        // A held button that produced no frames is zero-length, not "long enough".
        assert_eq!(audio_duration_ms(0, 2, 48_000), 0);
    }

    #[test]
    fn denied_microphone_access_is_explained_rather_than_shown_as_an_hresult() {
        let message = microphone_error("Cannot open the microphone", "backend error: 0x80070005");
        assert!(message.to_lowercase().contains("microphone"), "{message}");
        assert!(!message.contains("0x80070005"), "{message}");
    }

    #[test]
    fn meter_level_is_bounded_and_survives_a_bad_sample_block() {
        assert_eq!(normalize_level(0.0), 0.0);
        assert_eq!(normalize_level(1.0), 1.0);
        assert_eq!(normalize_level(f32::NAN), 0.0);
        assert_eq!(normalize_level(-1.0), 0.0);
        assert!(normalize_level(0.1) > 0.3 && normalize_level(0.1) < 0.5);
    }

    #[test]
    fn appended_blocks_publish_their_level_and_respect_the_cap() {
        let samples = std::sync::Arc::new(std::sync::Mutex::new(Vec::<f32>::new()));
        let level = std::sync::Arc::new(std::sync::atomic::AtomicU32::new(0));
        append_samples(&samples, &level, [0.5_f32, -0.5, 0.5, -0.5].into_iter(), 3);
        assert_eq!(samples.lock().unwrap().len(), 3, "cap must be enforced");
        let published = f32::from_bits(level.load(std::sync::atomic::Ordering::Relaxed));
        assert!((published - 0.5).abs() < 1e-6, "{published}");
    }

    #[test]
    fn unrelated_microphone_failures_keep_their_detail() {
        let message = microphone_error("Cannot open the microphone", "device disconnected");
        assert_eq!(message, "Cannot open the microphone: device disconnected");
    }
}
