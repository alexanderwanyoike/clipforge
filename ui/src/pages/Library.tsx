import { createSignal, onMount, For, Show } from "solid-js";
import { useNavigate } from "@solidjs/router";
import {
  getRecordings,
  searchRecordings,
  deleteRecording,
  type Recording,
} from "../lib/tauri";
import { convertFileSrc } from "@tauri-apps/api/core";

function formatDuration(seconds: number): string {
  const m = Math.floor(seconds / 60);
  const s = Math.floor(seconds % 60);
  return `${m}:${s.toString().padStart(2, "0")}`;
}

function formatSize(bytes: number): string {
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(0)} KB`;
  if (bytes < 1024 * 1024 * 1024)
    return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
  return `${(bytes / (1024 * 1024 * 1024)).toFixed(2)} GB`;
}

export default function LibraryPage() {
  const navigate = useNavigate();
  const [recordings, setRecordings] = createSignal<Recording[]>([]);
  const [query, setQuery] = createSignal("");

  onMount(async () => {
    const recs = await getRecordings();
    setRecordings(recs);
  });

  async function handleSearch() {
    const q = query().trim();
    if (q) {
      const results = await searchRecordings(q);
      setRecordings(results);
    } else {
      const recs = await getRecordings();
      setRecordings(recs);
    }
  }

  async function handleDelete(id: string) {
    await deleteRecording(id);
    setRecordings((prev) => prev.filter((r) => r.id !== id));
  }

  return (
    <div>
      <h1 class="page-title">Library</h1>

      <div class="search-bar">
        <input
          class="search-input"
          type="text"
          placeholder="Search recordings..."
          value={query()}
          onInput={(e) => setQuery(e.currentTarget.value)}
          onKeyDown={(e) => e.key === "Enter" && handleSearch()}
        />
        <button class="btn btn-primary" onClick={handleSearch}>
          Search
        </button>
      </div>

      <Show
        when={recordings().length > 0}
        fallback={
          <div class="empty-state">
            <p>No recordings yet</p>
            <p>Start recording to see clips here</p>
          </div>
        }
      >
        <div class="library-grid">
          <For each={recordings()}>
            {(rec) => (
              <div class="recording-card">
                <div class="card-thumb">
                  <Show
                    when={rec.thumbnail_path}
                    fallback={<span>No Preview</span>}
                  >
                    <img
                      src={convertFileSrc(rec.thumbnail_path!)}
                      alt={rec.title}
                    />
                  </Show>
                </div>
                <div class="card-info">
                  <div class="card-title">{rec.title}</div>
                  <div class="card-meta">
                    <span>{formatDuration(rec.duration)}</span>
                    <span>{rec.resolution}</span>
                    <span>{formatSize(rec.file_size)}</span>
                  </div>
                  <div class="card-actions">
                    <button
                      class="btn btn-primary btn-sm"
                      onClick={(e) => {
                        e.stopPropagation();
                        navigate(`/export?file=${encodeURIComponent(rec.file_path)}`);
                      }}
                    >
                      Export
                    </button>
                    <button
                      class="btn btn-danger btn-sm"
                      onClick={(e) => {
                        e.stopPropagation();
                        handleDelete(rec.id);
                      }}
                    >
                      Delete
                    </button>
                  </div>
                </div>
              </div>
            )}
          </For>
        </div>
      </Show>
    </div>
  );
}
