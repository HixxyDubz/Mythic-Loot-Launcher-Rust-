export type ReadinessStatus =
  | "ready"
  | "updateRequired"
  | "repairNeeded"
  | "serverOffline"
  | "gamePathMissing"
  | "setupRequired"
  | "checking"
  | "failed";

export interface GameProfile {
  id: string;
  game: string;
  displayName: string;
  serverName: string;
  serverIp: string;
  serverPort: number;
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

export interface ServerStatus {
  profileId: string;
  configured: boolean;
  checked: boolean;
  online: boolean | null;
  players: number | null;
  maxPlayers: number | null;
  latencyMs: number | null;
  version: string;
  motd: string;
  map: string;
  message: string;
  cached: boolean;
  checkedAtEpoch: number | null;
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
  servers: ServerStatus[];
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
  joinHint: string;
}
