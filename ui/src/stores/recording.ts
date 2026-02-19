import { createSignal, onCleanup, onMount } from "solid-js";
import {
  startRecording,
  stopRecording,
  getRecordingStatus,
  onRecordingStateChanged,
  onRecordingTimer,
  toggleReplayBuffer,
  saveReplayClip,
  getReplayStatus,
  onReplayStateChanged,
  type RecordingState,
} from "../lib/tauri";

export function useRecording() {
  const [state, setState] = createSignal<RecordingState>({
    status: "Idle",
    elapsed_secs: 0,
    file_path: null,
  });
  const [timer, setTimer] = createSignal(0);
  const [replayActive, setReplayActive] = createSignal(false);

  onMount(async () => {
    const status = await getRecordingStatus();
    setState(status);

    const replay = await getReplayStatus();
    setReplayActive(replay);

    const unlisten1 = await onRecordingStateChanged((s) => setState(s));
    const unlisten2 = await onRecordingTimer((secs) => setTimer(secs));
    const unlisten3 = await onReplayStateChanged((active) =>
      setReplayActive(active)
    );

    onCleanup(() => {
      unlisten1();
      unlisten2();
      unlisten3();
    });
  });

  async function toggleRecord() {
    if (state().status === "Recording") {
      await stopRecording();
    } else if (state().status === "Idle") {
      await startRecording();
    }
  }

  async function toggleReplay() {
    const active = await toggleReplayBuffer();
    setReplayActive(active);
  }

  async function saveReplay(seconds?: number) {
    return saveReplayClip(seconds);
  }

  return {
    state,
    timer,
    replayActive,
    toggleRecord,
    toggleReplay,
    saveReplay,
  };
}
