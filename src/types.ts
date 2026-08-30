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
  minecraftLauncher: string;
  discordInvite: string;
  updateSource: string;
  manifestUrl: string;
  deploymentSubdir: string;
  logoPath: string;
  catalogVisible: boolean;
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

export interface CatalogRefreshOutcome {
  payload: BootstrapPayload;
  summary: {
    catalogChanged: boolean;
    manifestsChanged: number;
    manifestsChecked: number;
    online: boolean;
    message: string;
  };
}

export interface DetectedInstall {
  label: string;
  exePath: string | null;
  installDir: string;
  source: string;
}

export type MinecraftLauncher = "curseforge" | "modrinth";

export interface MinecraftBootstrapRequest {
  profileId: string;
  launcher: MinecraftLauncher;
}

export interface MinecraftBootstrapArtifact {
  launcher: MinecraftLauncher;
  fileName: string;
  path: string;
  bytes: number;
  sha256: string;
  message: string;
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

export interface PackageAssetPreview {
  fileName: string;
  path: string;
  bytes: number;
  sha256: string;
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
  multipart: boolean;
  assets: PackageAssetPreview[];
  added: number;
  changed: number;
  removed: number;
  issues: string[];
  ready: boolean;
}

export interface ReleasePublication {
  profileId: string;
  version: string;
  repository: string;
  tag: string;
  manifestUrl: string;
  url: string;
  message: string;
}

export interface ModpackPublicationOutcome {
  publication: ReleasePublication;
  payload: BootstrapPayload;
}

export interface CatalogProfilePreview {
  id: string;
  displayName: string;
  version: string;
  manifestUrl: string;
}

export interface CatalogPreview {
  previewId: string;
  repository: string;
  branch: string;
  publicUrl: string;
  outputPath: string;
  bytes: number;
  sha256: string;
  profiles: CatalogProfilePreview[];
  hiddenProfiles: number;
  issues: string[];
  ready: boolean;
}

export interface CatalogPublication {
  repository: string;
  branch: string;
  publicUrl: string;
  commitUrl: string;
  profiles: number;
  message: string;
}

export type TransactionKind = "update" | "repair";

export interface TransactionRequest {
  profileId: string;
  kind: TransactionKind;
}

export interface TransactionPreview {
  previewId: string;
  profileId: string;
  kind: TransactionKind;
  version: string;
  source: string;
  stagedFiles: number;
  stagedBytes: number;
  existingFilesToBackup: number;
  newFiles: number;
  obsoletePaths: number;
  issues: string[];
  ready: boolean;
  nothingToDo: boolean;
  message: string;
}

export interface TransactionOutcome {
  profileId: string;
  kind: TransactionKind;
  success: boolean;
  applied: string[];
  removed: string[];
  backupPath: string;
  rolledBack: boolean;
  rollbackError: string;
  message: string;
  error: string;
}

export interface RestorePointSummary {
  backupId: string;
  profileId: string;
  label: string;
  createdAt: number;
  sizeBytes: number;
  fileCount: number;
  removesOnRestore: number;
  localModpackVersion: string;
  valid: boolean;
  issues: string[];
}

export interface RestorePreview {
  previewId: string;
  backupId: string;
  profileId: string;
  label: string;
  createdAt: number;
  localModpackVersion: string;
  stagedFiles: number;
  stagedBytes: number;
  existingFilesToBackup: number;
  filesToRemove: number;
  ready: boolean;
  message: string;
}

export interface RestoreOutcome {
  profileId: string;
  backupId: string;
  success: boolean;
  restored: string[];
  removed: string[];
  recoveryBackupPath: string;
  rolledBack: boolean;
  rollbackError: string;
  message: string;
  error: string;
}

export interface SafeLaunchStatus {
  profileId: string;
  active: boolean;
  sessionId: string;
  installDir: string;
  gameProcessId: number;
  gameProcessRunning: boolean;
  disabledFiles: number;
  startedAt: number;
  recoverable: boolean;
  message: string;
}

export interface SafeLaunchOutcome {
  profileId: string;
  sessionId: string;
  pid: number;
  disabled: string[];
  message: string;
}

export interface SafeLaunchRecovery {
  profileId: string;
  restored: string[];
  message: string;
}
