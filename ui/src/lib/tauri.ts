import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

// Recording
export interface RecordingState {
  status: "Idle" | "Starting" | "Recording" | "Stopping";
  elapsed_secs: number;
  file_path: string | null;
}

export async function startRecording(): Promise<void> {
  return invoke("start_recording");
}

export async function stopRecording(): Promise<string> {
  return invoke("stop_recording");
}

export async function getRecordingStatus(): Promise<RecordingState> {
  return invoke("get_recording_status");
}

export function onRecordingStateChanged(
  callback: (state: RecordingState) => void
): Promise<UnlistenFn> {
  return listen<RecordingState>("recording-state-changed", (event) =>
    callback(event.payload)
  );
}

export function onRecordingTimer(
  callback: (secs: number) => void
): Promise<UnlistenFn> {
  return listen<number>("recording-timer", (event) =>
    callback(event.payload)
  );
}

// Replay
export async function toggleReplayBuffer(): Promise<boolean> {
  return invoke("toggle_replay_buffer");
}

export async function saveReplayClip(seconds?: number): Promise<string> {
  return invoke("save_replay_clip", { seconds });
}

export async function getReplayStatus(): Promise<boolean> {
  return invoke("get_replay_status");
}

export function onReplayStateChanged(
  callback: (active: boolean) => void
): Promise<UnlistenFn> {
  return listen<boolean>("replay-state-changed", (event) =>
    callback(event.payload)
  );
}

export function onReplaySaved(
  callback: (path: string) => void
): Promise<UnlistenFn> {
  return listen<string>("replay-saved", (event) =>
    callback(event.payload)
  );
}

// Export
export interface ExportPreset {
  id: string;
  name: string;
  description: string;
  resolution: [number, number] | null;
  fps: number | null;
  codec: string;
  bitrate: string | null;
  crop_aspect: [number, number] | null;
  loudnorm: boolean;
  container: string;
}

export async function getExportPresets(): Promise<ExportPreset[]> {
  return invoke("get_export_presets");
}

export async function startExport(params: {
  input: string;
  preset_id: string;
  trim_start?: number;
  trim_end?: number;
  output?: string;
}): Promise<string> {
  return invoke("start_export", params);
}

export function onExportCompleted(
  callback: (path: string) => void
): Promise<UnlistenFn> {
  return listen<string>("export-completed", (event) =>
    callback(event.payload)
  );
}

// Library
export interface Recording {
  id: string;
  title: string;
  file_path: string;
  file_size: number;
  duration: number;
  resolution: string;
  fps: number;
  codec: string;
  container: string;
  source_type: string;
  game_name: string | null;
  created_at: string;
  thumbnail_path: string | null;
}

export async function getRecordings(
  limit?: number,
  offset?: number
): Promise<Recording[]> {
  return invoke("get_recordings", { limit, offset });
}

export async function searchRecordings(query: string): Promise<Recording[]> {
  return invoke("search_recordings", { query });
}

export async function deleteRecording(id: string): Promise<void> {
  return invoke("delete_recording", { id });
}

// System
export interface EncoderInfo {
  name: string;
  hw_accel: string;
  available: boolean;
  device: string | null;
}

export interface AudioSource {
  id: string;
  name: string;
  source_type: string;
}

export interface DiagnosticCheck {
  name: string;
  status: "Pass" | "Warn" | "Fail";
  detail: string;
  recommendation: string | null;
}

export interface DiagnosticReport {
  checks: DiagnosticCheck[];
}

export async function getEncoders(): Promise<EncoderInfo[]> {
  return invoke("get_encoders");
}

export async function getAudioSources(): Promise<AudioSource[]> {
  return invoke("get_audio_sources");
}

export async function getConfig(): Promise<any> {
  return invoke("get_config");
}

export async function updateConfig(config: any): Promise<void> {
  return invoke("update_config", { config });
}

export async function runDoctor(): Promise<DiagnosticReport> {
  return invoke("run_doctor");
}
