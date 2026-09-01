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

export interface RulesGuide {
  howToJoin: string;
  rules: string[];
  commonFixes: string[];
}

export interface ChangelogEntry {
  version: string;
  date: string;
  added: string[];
  changed: string[];
  fixed: string[];
  notes: string;
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
  announcement: string;
  newsBannerUrl: string;
  rulesGuide: RulesGuide;
  changelog: ChangelogEntry[];
}

export interface ManifestContentInput {
  announcement: string;
  newsBannerUrl: string;
  rulesGuide: RulesGuide;
  changelog: ChangelogEntry[];
}

export interface ManifestContentSaveOutcome {
  changed: boolean;
  payload: BootstrapPayload;
}

export interface ContentReleasePreview {
  previewId: string;
  profileId: string;
  repository: string;
  tag: string;
  manifestUrl: string;
  manifestPath: string;
  modpackVersion: string;
  bytes: number;
  sha256: string;
  packageAssetsPreserved: number;
  requiredFileCount: number;
  rulesCount: number;
  changelogCount: number;
  issues: string[];
  ready: boolean;
}

export interface ContentReleasePublication {
  profileId: string;
  repository: string;
  tag: string;
  manifestUrl: string;
  url: string;
  message: string;
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

export type ActivityKind =
  | "catalogue"
  | "storage"
  | "support"
  | "verifying"
  | "updating"
  | "repairing"
  | "restoring"
  | "publishing"
  | "launching"
  | "setup";

export interface ActivityItem {
  id: string;
  title: string;
  kind: ActivityKind;
  message: string;
  progress: number | null;
  bytesDone: number | null;
  bytesTotal: number | null;
  startedAt: number;
  updatedAt: number;
  done: boolean;
  success: boolean | null;
}

export type StorageCleanupKind = "oldBackups" | "metadataCache" | "temporaryWork";

export interface StorageBucket {
  key: string;
  label: string;
  category: string;
  path: string;
  bytesUsed: number;
  fileCount: number;
  directoryCount: number;
  exists: boolean;
  truncated: boolean;
  cleanupKind: StorageCleanupKind | null;
}

export interface StorageReport {
  dataDir: string;
  launcherBytes: number;
  profileBytes: number;
  measuredAt: number;
  temporaryRetentionHours: number;
  backupsKeptPerProfile: number;
  buckets: StorageBucket[];
  issues: string[];
  truncated: boolean;
}

export interface SupportPreview {
  previewId: string;
  profileId: string;
  displayName: string;
  latestLogPath: string;
  latestLogName: string;
  sourceBytes: number;
  includedBytes: number;
  truncated: boolean;
  summary: string;
  redactedLog: string;
  files: string[];
  issues: string[];
  ready: boolean;
  message: string;
}

export interface SupportBundleOutcome {
  profileId: string;
  path: string;
  directory: string;
  fileName: string;
  bytes: number;
  sha256: string;
  files: string[];
  message: string;
}

export interface StorageCleanupOutcome {
  kind: StorageCleanupKind;
  deletedEntries: number;
  reclaimedBytes: number;
  skippedEntries: number;
  complete: boolean;
  message: string;
  report: StorageReport;
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
