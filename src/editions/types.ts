import type { GameProfile } from "../types";

export interface EditionPublisherProps {
  profile: GameProfile;
  onBack: () => void;
  onNotice: (message: string) => void;
}
