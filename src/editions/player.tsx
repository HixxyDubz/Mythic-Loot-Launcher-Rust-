import type { EditionModpackManagerProps, EditionProfileMetadataProps, EditionPublisherProps } from "./types";

export const launcherEdition = "player" as const;
export const publisherAvailable = false;

export function EditionPublisherPanel(_props: EditionPublisherProps) {
  return null;
}

export function EditionModpackManagerPanel(_props: EditionModpackManagerProps) {
  return null;
}

export function EditionProfileMetadataSection(_props: EditionProfileMetadataProps) {
  return null;
}
