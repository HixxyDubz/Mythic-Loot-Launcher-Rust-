export type ReadinessStatus =
  | "ready"
  | "updateRequired"
  | "repairNeeded"
  | "gamePathMissing"
  | "setupRequired"
  | "checking"
  | "failed";

export interface GameProfile {
  id: string;
  game: string;
  displayName: string;
  requiredGameVersion: string;
  requiredModpackVersion: string;
  localModpackVersion: string;
  manifestPath: string;
  installDir: string;
  gameDir: string;
  gameExePath: string;
  launchArgs: string;
  discordInvite: string;
  updateSource: string;
  manifestUrl: string;
  deploymentSubdir: string;
  logoPath: string;
}

export interface LauncherConfig {
  schemaVersion: number;
  selectedProfileId: string;
  profiles: GameProfile[];
  preferences: {
    reduceMotion: boolean;
    autoCheckUpdates: boolean;
    closeAfterLaunch: boolean;
  };
}

export interface GameDefinition {
  id: string;
  displayName: string;
  detectionKind: string;
}

export interface ProfileHealth {
  profileId: string;
  status: ReadinessStatus;
  headline: string;
  details: string[];
}

export interface ManifestSummary {
  profileId: string;
  valid: boolean;
  manifestVersion: string;
  modpackVersion: string;
  releaseDate: string;
  requiredFileCount: number;
  optionalFileCount: number;
  obsoleteFileCount: number;
  updateSize: number | null;
  source: string;
  errors: string[];
}

export interface FileVerification {
  profileId: string;
  checked: number;
  current: number;
  missing: string[];
  changed: string[];
  unsafeEntries: string[];
}

export interface BootstrapPayload {
  config: LauncherConfig;
  games: GameDefinition[];
  health: ProfileHealth[];
  manifests: ManifestSummary[];
  dataDir: string;
}

export interface DetectedInstall {
  label: string;
  exePath: string | null;
  installDir: string;
  source: string;
}

export interface LaunchOutcome {
  pid: number;
  message: string;
}

export interface PublisherStatus {
  ghAvailable: boolean;
  authenticated: boolean;
  account: string;
  message: string;
}

export interface RepositoryRequest {
  repository: string;
  description: string;
  visibility: "private" | "public";
  confirmed: boolean;
}

export interface RepositoryCreation {
  repository: string;
  url: string;
  message: string;
}

export interface PackageRequest {
  profileId: string;
  sourceDir: string;
  version: string;
  releaseDate: string;
  repository: string;
  releaseNotes: string;
}

export interface PackagePreview {
  previewId: string;
  profileId: string;
  version: string;
  tag: string;
  repository: string;
  sourceDir: string;
  outputDir: string;
  packagePath: string;
  manifestPath: string;
  fileCount: number;
  excludedCount: number;
  totalBytes: number;
  packageBytes: number;
  packageSha256: string;
  added: number;
  changed: number;
  removed: number;
  issues: string[];
  ready: boolean;
}

export interface ReleasePublication {
  repository: string;
  tag: string;
  url: string;
  message: string;
}
