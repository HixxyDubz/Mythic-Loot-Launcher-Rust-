import { useState } from "react";
import { FileCheck2, Newspaper, Plus, RefreshCw, Save, Trash2 } from "lucide-react";
import { saveManifestContent } from "../api";
import type {
  BootstrapPayload,
  ChangelogEntry,
  ManifestContentInput,
  ManifestSummary,
} from "../types";

interface ManifestContentEditorProps {
  profileId: string;
  manifest: ManifestSummary;
  onNotice: (message: string) => void;
  onPayload: (payload: BootstrapPayload) => void;
}

export function ManifestContentEditor({
  profileId,
  manifest,
  onNotice,
  onPayload,
}: ManifestContentEditorProps) {
  const [draft, setDraft] = useState<ManifestContentInput>(() => fromManifest(manifest));
  const [busy, setBusy] = useState(false);

  function update<K extends keyof ManifestContentInput>(key: K, value: ManifestContentInput[K]) {
    setDraft((current) => ({ ...current, [key]: value }));
  }

  function updateChangelog(index: number, value: ChangelogEntry) {
    update("changelog", draft.changelog.map((entry, entryIndex) => entryIndex === index ? value : entry));
  }

  function addChangelog() {
    update("changelog", [
      {
        version: manifest.modpackVersion,
        date: new Date().toISOString().slice(0, 10),
        added: [],
        changed: [],
        fixed: [],
        notes: "",
      },
      ...draft.changelog,
    ]);
  }

  async function save() {
    setBusy(true);
    try {
      const result = await saveManifestContent(profileId, cleanContent(draft));
      onPayload(result.payload);
      onNotice(result.changed
        ? "Manifest content saved locally and will be included in the next modpack release."
        : "Manifest content already matches the saved local copy.");
    } catch (error) {
      onNotice(errorMessage(error));
    } finally {
      setBusy(false);
    }
  }

  return (
    <section className="settings-section panel-card manifest-content-editor">
      <div className="section-title">
        <Newspaper />
        <div>
          <h2>Manifest content editor</h2>
          <p>Edit the real News, Rules and Changelog content shown by Player for this modpack.</p>
        </div>
      </div>

      <div className="form-stack">
        <label className="field">
          <span>News announcement</span>
          <textarea
            aria-label="News announcement"
            value={draft.announcement}
            maxLength={20_000}
            placeholder="Write the current modpack announcement. Leave blank to show no announcement."
            onChange={(event) => update("announcement", event.target.value)}
          />
        </label>
        <label className="field">
          <span>News banner HTTPS URL</span>
          <input
            aria-label="News banner HTTPS URL"
            type="url"
            value={draft.newsBannerUrl}
            maxLength={2_048}
            placeholder="https://example.com/banner.webp"
            onChange={(event) => update("newsBannerUrl", event.target.value)}
          />
        </label>
        <label className="field">
          <span>How to install or join</span>
          <textarea
            aria-label="How to install or join"
            value={draft.rulesGuide.howToJoin}
            maxLength={20_000}
            placeholder="Explain how to install and start the modpack."
            onChange={(event) => update("rulesGuide", {
              ...draft.rulesGuide,
              howToJoin: event.target.value,
            })}
          />
        </label>
        <div className="form-grid">
          <LineListField
            label="Rules (one per line)"
            value={draft.rulesGuide.rules}
            onChange={(rules) => update("rulesGuide", { ...draft.rulesGuide, rules })}
          />
          <LineListField
            label="Common fixes (one per line)"
            value={draft.rulesGuide.commonFixes}
            onChange={(commonFixes) => update("rulesGuide", { ...draft.rulesGuide, commonFixes })}
          />
        </div>
      </div>

      <div className="content-editor-heading">
        <div><strong>Changelog entries</strong><small>Newest entries should appear first.</small></div>
        <button className="secondary-action" onClick={addChangelog} disabled={busy || draft.changelog.length >= 200}>
          <Plus size={15} /> Add entry
        </button>
      </div>

      {draft.changelog.length === 0 ? (
        <div className="content-editor-empty">No changelog entries. Player will show a truthful empty state.</div>
      ) : (
        <div className="content-editor-changelog">
          {draft.changelog.map((entry, index) => (
            <ChangelogEditor
              key={index}
              index={index}
              entry={entry}
              busy={busy}
              onChange={(value) => updateChangelog(index, value)}
              onRemove={() => update("changelog", draft.changelog.filter((_, entryIndex) => entryIndex !== index))}
            />
          ))}
        </div>
      )}

      <div className="safety-note publisher-safety">
        <FileCheck2 size={15} /> Saving writes only to the launcher-owned trusted manifest. Package URLs, hashes, file inventories, versions and multipart data are preserved and fully revalidated. This local save does not contact GitHub; the next release carries the content to Player.
      </div>
      <button className="primary-action publisher-preview" onClick={() => void save()} disabled={busy}>
        {busy ? <RefreshCw className="spin" size={17} /> : <Save size={17} />} Save manifest content locally
      </button>
    </section>
  );
}

function LineListField({
  label,
  value,
  onChange,
}: {
  label: string;
  value: string[];
  onChange: (value: string[]) => void;
}) {
  return (
    <label className="field">
      <span>{label}</span>
      <textarea
        aria-label={label}
        value={value.join("\n")}
        placeholder="One item per line"
        onChange={(event) => onChange(splitLines(event.target.value, false))}
      />
    </label>
  );
}

function ChangelogEditor({
  index,
  entry,
  busy,
  onChange,
  onRemove,
}: {
  index: number;
  entry: ChangelogEntry;
  busy: boolean;
  onChange: (entry: ChangelogEntry) => void;
  onRemove: () => void;
}) {
  const number = index + 1;
  return (
    <article className="content-editor-entry">
      <header>
        <strong>Entry {number}</strong>
        <button className="icon-danger" aria-label={`Remove changelog entry ${number}`} onClick={onRemove} disabled={busy}>
          <Trash2 size={14} />
        </button>
      </header>
      <div className="form-grid">
        <label className="field">
          <span>Version</span>
          <input aria-label={`Changelog version ${number}`} value={entry.version} maxLength={128} onChange={(event) => onChange({ ...entry, version: event.target.value })} />
        </label>
        <label className="field">
          <span>Date</span>
          <input aria-label={`Changelog date ${number}`} type="date" value={entry.date} onChange={(event) => onChange({ ...entry, date: event.target.value })} />
        </label>
        <label className="field field-wide">
          <span>Notes</span>
          <textarea aria-label={`Changelog notes ${number}`} value={entry.notes} maxLength={20_000} onChange={(event) => onChange({ ...entry, notes: event.target.value })} />
        </label>
        <LineListField label={`Added in entry ${number} (one per line)`} value={entry.added} onChange={(added) => onChange({ ...entry, added })} />
        <LineListField label={`Changed in entry ${number} (one per line)`} value={entry.changed} onChange={(changed) => onChange({ ...entry, changed })} />
        <label className="field field-wide">
          <span>{`Fixed in entry ${number} (one per line)`}</span>
          <textarea aria-label={`Fixed in entry ${number} (one per line)`} value={entry.fixed.join("\n")} onChange={(event) => onChange({ ...entry, fixed: splitLines(event.target.value, false) })} />
        </label>
      </div>
    </article>
  );
}

function fromManifest(manifest: ManifestSummary): ManifestContentInput {
  return {
    announcement: manifest.announcement,
    newsBannerUrl: manifest.newsBannerUrl,
    rulesGuide: structuredClone(manifest.rulesGuide),
    changelog: structuredClone(manifest.changelog),
  };
}

function cleanContent(content: ManifestContentInput): ManifestContentInput {
  return {
    announcement: content.announcement.trim(),
    newsBannerUrl: content.newsBannerUrl.trim(),
    rulesGuide: {
      howToJoin: content.rulesGuide.howToJoin.trim(),
      rules: content.rulesGuide.rules.map((item) => item.trim()).filter(Boolean),
      commonFixes: content.rulesGuide.commonFixes.map((item) => item.trim()).filter(Boolean),
    },
    changelog: content.changelog.map((entry) => ({
      version: entry.version.trim(),
      date: entry.date.trim(),
      notes: entry.notes.trim(),
      added: entry.added.map((item) => item.trim()).filter(Boolean),
      changed: entry.changed.map((item) => item.trim()).filter(Boolean),
      fixed: entry.fixed.map((item) => item.trim()).filter(Boolean),
    })),
  };
}

function splitLines(value: string, trim = true): string[] {
  const lines = value.replace(/\r/g, "").split("\n");
  return trim ? lines.map((line) => line.trim()).filter(Boolean) : lines;
}

function errorMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}
