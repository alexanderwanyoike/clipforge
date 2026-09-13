import { listen } from "@tauri-apps/api/event";
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
  const [error, setError] = createSignal<string | null>(null);
  const [replayBusy, setReplayBusy] = createSignal(false);

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

    const unlisten4 = await listen<string>("recording-error", (event) => setError(event.payload));

    onCleanup(() => {
      unlisten1();
      unlisten2();
      unlisten3();
      unlisten4();
    });
  });

  async function toggleRecord() {
    setError(null);
    try {
      if (state().status === "Recording" || state().status === "Starting") {
        setState({ ...state(), status: "Stopping" });
        await stopRecording();
      } else if (state().status === "Idle") {
        setState({ status: "Starting", elapsed_secs: 0, file_path: null });
        setTimer(0);
        await startRecording();
      }
    } catch (error) {
      setError(String(error));
      setState(await getRecordingStatus());
    }
  }

  async function toggleReplay() {
    if (replayBusy()) return;
    setReplayBusy(true);
    setError(null);
    try {
      setReplayActive(await toggleReplayBuffer());
    } catch (error) {
      setError(String(error));
    } finally {
      setReplayBusy(false);
    }
  }

  async function saveReplay(seconds?: number) {
    setError(null);
    try {
      return await saveReplayClip(seconds);
    } catch (error) {
      setError(String(error));
    }
  }

  return {
    state,
    error,
    replayBusy,
    timer,
    replayActive,
    toggleRecord,
    toggleReplay,
    saveReplay,
  };
}
