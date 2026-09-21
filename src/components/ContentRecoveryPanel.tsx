import { useState } from "react";
import { applyContentRecovery, inspectContentRecovery } from "../api";
import type { ContentRecoveryState, ManifestContentSaveOutcome } from "../types";
import { useOperationScope } from "../useOperationScope";

interface Props {
  profileId: string;
  disabled: boolean;
  onBusy: (busy: boolean) => void;
  onApplied: (result: ManifestContentSaveOutcome) => void;
  onNotice: (message: string) => void;
}

export function ContentRecoveryPanel({ profileId, disabled, onBusy, onApplied, onNotice }: Props) {
  const [review, setReview] = useState<ContentRecoveryState | null>(null);
  const [selected, setSelected] = useState("");
  const [confirmed, setConfirmed] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const scope = useOperationScope();
  const candidate = review?.candidates.find((item) => item.id === selected);
  function working(value: boolean) { setBusy(value); onBusy(value); }

  async function inspect() {
    if (disabled || busy) return;
    const current = scope();
    working(true); setError(""); setReview(null); setSelected(""); setConfirmed(false);
    try {
      const result = await inspectContentRecovery(profileId);
      if (!current()) return;
      if (result.profileId !== profileId) throw new Error("Recovery result belongs to a different profile; review again.");
      setReview(result);
    } catch (error) { if (current()) setError(String(error)); }
    finally { if (current()) working(false); }
  }
  async function apply(discard: boolean) {
    if (disabled || busy || !confirmed || !review || (discard ? !review.hasDraft : !candidate?.content)) return;
    const current = scope();
    working(true); setError("");
    try {
      const result = await applyContentRecovery(profileId, discard ? null : selected, review.draftRevision, true);
      if (!current()) return;
      setReview(null); setConfirmed(false); setSelected("");
      onApplied(result);
      onNotice(discard ? "Saved content draft discarded; a local recovery copy was retained. Published files and GitHub were not changed." : "Reviewed content recovered into the local draft. Nothing was published.");
    } catch (error) { if (current()) { setError(String(error)); setReview(null); setConfirmed(false); } }
    finally { if (current()) working(false); }
  }
  return <div className="content-recovery-panel">
    <h3>Recover or discard saved content</h3>
    <p>Older cached text may already have been published. Review it before recovering; the launcher cannot identify which old edits were unpublished. Recovery changes only News, Rules and Changelog, never modpack files, versions or download details.</p>
    <button type="button" className="secondary-action" disabled={disabled || busy} onClick={() => void inspect()}>Review saved content recovery</button>
    {error && <p role="alert">{error}</p>}
    {review && <div>
      <p>{review.hasDraft ? "A saved local draft exists. Replacing or discarding it keeps a recovery copy." : "No saved local draft exists."}</p>
      {review.limited && <p>Recovery history is limited to a bounded recent selection. Older copies remain in local content-recovery storage and have not been deleted.</p>}
      <label className="field"><span>Content recovery entry</span><select aria-label="Content recovery entry" value={selected} disabled={disabled || busy} onChange={(event) => { setSelected(event.target.value); setConfirmed(false); }}>
        <option value="">Choose an entry to review</option>
        {review.candidates.map((entry) => <option key={entry.id} value={entry.id}>{entry.label} · {entry.id.slice(0, 19)}</option>)}
      </select></label>
      {review.candidates.length === 0 && <p>No recovery copies are available yet. Content overwritten by an older version without a backup cannot be reconstructed.</p>}
      {candidate?.problem && <p role="alert">{candidate.problem}</p>}
      {candidate?.content && <pre className="content-recovery-preview" aria-label="Reviewed content preview">{JSON.stringify(candidate.content, null, 2)}</pre>}
      <label className="check-field"><input type="checkbox" checked={confirmed} disabled={disabled || busy} onChange={(event) => setConfirmed(event.target.checked)} /> I reviewed this action. Replace the editor's unsaved text and the saved local draft only; do not publish.</label>
      <div className="content-recovery-actions">
        <button type="button" className="secondary-action" disabled={disabled || busy || !confirmed || !candidate?.content} onClick={() => void apply(false)}>Recover reviewed content locally</button>
        <button type="button" className="secondary-action" disabled={disabled || busy || !confirmed || !review.hasDraft} onClick={() => void apply(true)}>Discard saved draft with recovery copy</button>
      </div>
    </div>}
  </div>;
}
