import { createSignal, onMount, For, Show } from "solid-js";
import {
  getExportPresets,
  startExport,
  onExportCompleted,
  type ExportPreset,
} from "../lib/tauri";

export default function ExportPage() {
  const [presets, setPresets] = createSignal<ExportPreset[]>([]);
  const [selectedPreset, setSelectedPreset] = createSignal<string>("");
  const [inputFile, setInputFile] = createSignal("");
  const [trimStart, setTrimStart] = createSignal<number | undefined>();
  const [trimEnd, setTrimEnd] = createSignal<number | undefined>();
  const [exporting, setExporting] = createSignal(false);
  const [exportResult, setExportResult] = createSignal<string | null>(null);

  onMount(async () => {
    const p = await getExportPresets();
    setPresets(p);
    if (p.length > 0) setSelectedPreset(p[0].id);

    await onExportCompleted((path) => {
      setExporting(false);
      setExportResult(path);
    });
  });

  async function handleExport() {
    if (!inputFile() || !selectedPreset()) return;
    setExporting(true);
    setExportResult(null);

    try {
      await startExport({
        input: inputFile(),
        preset_id: selectedPreset(),
        trim_start: trimStart(),
        trim_end: trimEnd(),
      });
    } catch (e) {
      setExporting(false);
      console.error("Export failed:", e);
    }
  }

  return (
    <div>
      <h1 class="page-title">Export</h1>

      <div class="settings-section">
        <h2>Input File</h2>
        <input
          class="search-input"
          type="text"
          placeholder="Path to recording..."
          value={inputFile()}
          onInput={(e) => setInputFile(e.currentTarget.value)}
          style="width: 100%"
        />
      </div>

      <div class="settings-section">
        <h2>Preset</h2>
        <div class="preset-grid">
          <For each={presets()}>
            {(preset) => (
              <div
                class={`preset-card ${selectedPreset() === preset.id ? "selected" : ""}`}
                onClick={() => setSelectedPreset(preset.id)}
              >
                <div class="preset-name">{preset.name}</div>
                <div class="preset-desc">{preset.description}</div>
                <Show when={preset.resolution}>
                  <div class="preset-desc" style="margin-top: 4px">
                    {preset.resolution![0]}x{preset.resolution![1]}
                    {preset.fps ? ` @ ${preset.fps}fps` : ""}
                  </div>
                </Show>
              </div>
            )}
          </For>
        </div>
      </div>

      <div class="settings-section">
        <h2>Trim (Optional)</h2>
        <div style="display: flex; gap: 12px">
          <div>
            <label class="setting-label">Start (seconds)</label>
            <input
              type="number"
              min="0"
              step="0.1"
              value={trimStart() ?? ""}
              onInput={(e) => {
                const v = parseFloat(e.currentTarget.value);
                setTrimStart(isNaN(v) ? undefined : v);
              }}
            />
          </div>
          <div>
            <label class="setting-label">End (seconds)</label>
            <input
              type="number"
              min="0"
              step="0.1"
              value={trimEnd() ?? ""}
              onInput={(e) => {
                const v = parseFloat(e.currentTarget.value);
                setTrimEnd(isNaN(v) ? undefined : v);
              }}
            />
          </div>
        </div>
      </div>

      <button
        class="btn btn-primary"
        onClick={handleExport}
        disabled={exporting() || !inputFile()}
      >
        {exporting() ? "Exporting..." : "Export"}
      </button>

      <Show when={exportResult()}>
        <div style="margin-top: 16px; color: var(--success); font-size: 14px">
          Exported: {exportResult()}
        </div>
      </Show>
    </div>
  );
}
