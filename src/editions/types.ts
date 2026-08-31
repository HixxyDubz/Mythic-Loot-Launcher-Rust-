import type { BootstrapPayload, GameDefinition, GameProfile, ManifestSummary } from "../types";

export interface EditionPublisherProps {
  profile: GameProfile;
  manifest: ManifestSummary;
  onBack: () => void;
  onNotice: (message: string) => void;
  onPayload: (payload: BootstrapPayload) => void;
}

export interface EditionModpackManagerProps {
  games: GameDefinition[];
  profiles: GameProfile[];
  busy: boolean;
  onBack: () => void;
  onCreate: (profile: GameProfile) => void;
}

export interface EditionProfileMetadataProps {
  draft: GameProfile;
  games: GameDefinition[];
  onUpdate: <K extends keyof GameProfile>(key: K, value: GameProfile[K]) => void;
}
