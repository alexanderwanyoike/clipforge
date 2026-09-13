import { Show } from "solid-js";
import { useRecording } from "../stores/recording";

function formatTime(seconds: number): string {
  const h = Math.floor(seconds / 3600);
  const m = Math.floor((seconds % 3600) / 60);
  const s = seconds % 60;
  const pad = (n: number) => n.toString().padStart(2, "0");
  return h > 0 ? `${pad(h)}:${pad(m)}:${pad(s)}` : `${pad(m)}:${pad(s)}`;
}

export default function Recorder() {
  const { state, error, replayBusy, timer, replayActive, toggleRecord, toggleReplay, saveReplay } =
    useRecording();

  const isRecording = () => state().status === "Recording";
  const isIdle = () => state().status === "Idle";
  const isBusy = () =>
    state().status === "Stopping";

  return (
    <div style="display: flex; flex-direction: column; height: 100%">
      <div class="record-container" style="flex: 1">
        <Show when={isRecording()}>
          <div class="record-timer">{formatTime(timer())}</div>
        </Show>

        <button
          class={`record-btn ${isRecording() ? "recording" : ""}`}
          onClick={toggleRecord}
          disabled={isBusy()}
        >
          <div class="record-btn-inner" />
        </button>

        <div class="record-status">
          <Show when={isIdle()}>Click to start recording</Show>
          <Show when={state().status === "Starting"}>Waiting for capture to start. Click again to cancel.</Show>
          <Show when={isRecording()}>Recording</Show>
          <Show when={state().status === "Stopping"}>Stopping...</Show>
        </div>

        <Show when={error()}>
          <div role="alert" style="color: var(--danger, #f87171); max-width: 640px; white-space: pre-wrap; padding: 12px">{error()}</div>
        </Show>

        <Show when={state().file_path}>
          <div class="record-file">{state().file_path}</div>
        </Show>
      </div>

      <div class="replay-bar">
        <button
          class={`toggle ${replayActive() ? "active" : ""}`}
          onClick={toggleReplay}
          disabled={replayBusy()}
          aria-label="Toggle replay buffer"
        />
        <span class="replay-label">
          Replay Buffer {replayActive() ? "(Active)" : "(Off)"}
        </span>
        <button
          class="replay-save-btn"
          onClick={() => saveReplay(30)}
          disabled={!replayActive()}
        >
          Save Last 30s
        </button>
      </div>
    </div>
  );
}
