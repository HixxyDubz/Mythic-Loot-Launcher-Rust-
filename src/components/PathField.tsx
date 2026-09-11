import { useState } from "react";
import { FolderOpen } from "lucide-react";
import { chooseLocalPath } from "../api";

export function PathField({ label, value, placeholder, kind = "folder", disabled = false, onChange, onNotice }: {
  label: string;
  value: string;
  placeholder?: string;
  kind?: "folder" | "executable";
  disabled?: boolean;
  onChange: (value: string) => void;
  onNotice: (message: string) => void;
}) {
  const [choosing, setChoosing] = useState(false);
  async function browse() {
    setChoosing(true);
    try {
      const selected = await chooseLocalPath(kind);
      if (selected !== null) onChange(selected);
    } catch (error) {
      onNotice(error instanceof Error ? error.message : String(error));
    } finally { setChoosing(false); }
  }
  return <div className="field field-wide">
    <label><span>{label}</span>
      <input aria-label={label} value={value} placeholder={placeholder} disabled={disabled || choosing} onChange={(event) => onChange(event.target.value)} />
    </label>
    <button className="secondary-action path-browse" type="button" aria-label={`Browse for ${label.toLowerCase()}`} disabled={disabled || choosing} onClick={() => void browse()}>
      <FolderOpen size={16} /> {choosing ? "Choosing…" : "Browse…"}
    </button>
  </div>;
}
