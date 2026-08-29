import type { GameDefinition, GameProfile } from "../types";

export interface EditionPublisherProps {
  profile: GameProfile;
  onBack: () => void;
  onNotice: (message: string) => void;
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
