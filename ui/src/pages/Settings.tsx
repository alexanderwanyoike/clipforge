import { createSignal, onMount, For, Show } from "solid-js";
import {
  getConfig,
  updateConfig,
  getEncoders,
  getAudioSources,
  runDoctor,
  type EncoderInfo,
  type AudioSource,
  type DiagnosticReport,
} from "../lib/tauri";

export default function SettingsPage() {
  const [config, setConfig] = createSignal<any>(null);
  const [encoders, setEncoders] = createSignal<EncoderInfo[]>([]);
  const [audioSources, setAudioSources] = createSignal<AudioSource[]>([]);
  const [diagnostics, setDiagnostics] = createSignal<DiagnosticReport | null>(
    null
  );
  const [saved, setSaved] = createSignal(false);

  onMount(async () => {
    const [c, e, a] = await Promise.all([
      getConfig(),
      getEncoders(),
      getAudioSources(),
    ]);
    setConfig(c);
    setEncoders(e);
    setAudioSources(a);
  });

  async function handleSave() {
    if (!config()) return;
    await updateConfig(config());
    setSaved(true);
    setTimeout(() => setSaved(false), 2000);
  }

  async function handleDoctor() {
    const report = await runDoctor();
    setDiagnostics(report);
  }

  return (
    <div>
      <h1 class="page-title">Settings</h1>

      <Show when={config()}>
        <div class="settings-section">
          <h2>Recording</h2>
          <div class="setting-row">
            <div>
              <div class="setting-label">Framerate</div>
              <div class="setting-desc">Capture framerate</div>
            </div>
            <select
              value={config()?.recording?.fps ?? 60}
              onChange={(e) => {
                const c = { ...config() };
                c.recording = { ...c.recording, fps: parseInt(e.currentTarget.value) };
                setConfig(c);
              }}
            >
              <option value="30">30 FPS</option>
              <option value="60">60 FPS</option>
              <option value="120">120 FPS</option>
            </select>
          </div>
          <div class="setting-row">
            <div>
              <div class="setting-label">Audio Source</div>
              <div class="setting-desc">Audio capture device</div>
            </div>
            <select
              value={config()?.recording?.audio_source ?? "default"}
              onChange={(e) => {
                const c = { ...config() };
                c.recording = { ...c.recording, audio_source: e.currentTarget.value };
                setConfig(c);
              }}
            >
              <option value="default">Default</option>
              <For each={audioSources()}>
                {(source) => <option value={source.id}>{source.name}</option>}
              </For>
            </select>
          </div>
        </div>

        <div class="settings-section">
          <h2>Replay Buffer</h2>
          <div class="setting-row">
            <div>
              <div class="setting-label">Duration</div>
              <div class="setting-desc">Seconds to keep in replay buffer</div>
            </div>
            <select
              value={config()?.replay?.duration_secs ?? 120}
              onChange={(e) => {
                const c = { ...config() };
                c.replay = { ...c.replay, duration_secs: parseInt(e.currentTarget.value) };
                setConfig(c);
              }}
            >
              <option value="30">30 seconds</option>
              <option value="60">60 seconds</option>
              <option value="120">120 seconds</option>
              <option value="300">5 minutes</option>
            </select>
          </div>
        </div>

        <div class="settings-section">
          <h2>Hotkeys</h2>
          <div class="setting-row">
            <div class="setting-label">Toggle Recording</div>
            <input
              type="text"
              value={config()?.hotkeys?.toggle_recording ?? ""}
              onInput={(e) => {
                const c = { ...config() };
                c.hotkeys = { ...c.hotkeys, toggle_recording: e.currentTarget.value };
                setConfig(c);
              }}
            />
          </div>
          <div class="setting-row">
            <div class="setting-label">Save Replay</div>
            <input
              type="text"
              value={config()?.hotkeys?.save_replay ?? ""}
              onInput={(e) => {
                const c = { ...config() };
                c.hotkeys = { ...c.hotkeys, save_replay: e.currentTarget.value };
                setConfig(c);
              }}
            />
          </div>
        </div>

        <button class="btn btn-primary" onClick={handleSave}>
          {saved() ? "Saved!" : "Save Settings"}
        </button>
      </Show>

      <div class="settings-section" style="margin-top: 28px">
        <h2>Encoders</h2>
        <For each={encoders()}>
          {(enc) => (
            <div class="setting-row">
              <div>
                <div class="setting-label">{enc.name}</div>
                <div class="setting-desc">
                  {enc.hw_accel} {enc.device ? `(${enc.device})` : ""}
                </div>
              </div>
              <span style={`color: ${enc.available ? "var(--success)" : "var(--danger)"}`}>
                {enc.available ? "Available" : "Unavailable"}
              </span>
            </div>
          )}
        </For>
      </div>

      <div class="settings-section">
        <h2>System Diagnostics</h2>
        <button class="btn btn-primary" onClick={handleDoctor}>
          Run Diagnostics
        </button>
        <Show when={diagnostics()}>
          <div style="margin-top: 12px">
            <For each={diagnostics()!.checks}>
              {(check) => (
                <div class="check-item">
                  <span
                    class={
                      check.status === "Pass"
                        ? "check-pass"
                        : check.status === "Warn"
                          ? "check-warn"
                          : "check-fail"
                    }
                  >
                    [{check.status.toUpperCase()}]
                  </span>
                  <span>
                    <strong>{check.name}:</strong> {check.detail}
                  </span>
                  <Show when={check.recommendation}>
                    <span style="color: var(--text-secondary); font-size: 12px">
                      {" "}
                      - {check.recommendation}
                    </span>
                  </Show>
                </div>
              )}
            </For>
          </div>
        </Show>
      </div>
    </div>
  );
}
