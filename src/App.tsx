import { useEffect, useMemo, useState } from "react";
import "./App.css";
import { bootstrap, detectInstallations, launchProfile, refreshServerStatus, saveProfile, selectProfile, verifyProfileFiles } from "./api";
import { Dashboard } from "./components/Dashboard";
import { SettingsPanel } from "./components/SettingsPanel";
import { Sidebar } from "./components/Sidebar";
import { TitleBar } from "./components/TitleBar";
import { previewHealth } from "./mock";
import type { BootstrapPayload, DetectedInstall, FileVerification, GameProfile, ProfileHealth } from "./types";

function App() {
  const [payload, setPayload] = useState<BootstrapPayload | null>(null);
  const [page, setPage] = useState<"dashboard" | "settings">("dashboard");
  const [busy, setBusy] = useState(false);
  const [notice, setNotice] = useState("");
  const [fatalError, setFatalError] = useState("");
  const [candidates, setCandidates] = useState<DetectedInstall[]>([]);
  const [verifications, setVerifications] = useState<Record<string, FileVerification>>({});

  useEffect(() => {
    void bootstrap()
      .then(setPayload)
      .catch((error) => setFatalError(errorMessage(error)));
  }, []);

  const selectedProfile = useMemo(
    () => payload?.config.profiles.find((profile) => profile.id === payload.config.selectedProfileId),
    [payload],
  );
  const selectedHealth = useMemo(
    () => payload?.health.find((health) => health.profileId === payload.config.selectedProfileId),
    [payload],
  );
  const selectedManifest = useMemo(
    () => payload?.manifests.find((manifest) => manifest.profileId === payload.config.selectedProfileId),
    [payload],
  );
  const selectedServer = useMemo(
    () => payload?.servers.find((server) => server.profileId === payload.config.selectedProfileId),
    [payload],
  );

  async function chooseProfile(profileId: string) {
    if (!payload || profileId === payload.config.selectedProfileId) return;
    setCandidates([]);
    const native = await selectProfile(profileId).catch((error) => {
      setNotice(errorMessage(error));
      return null;
    });
    if (native) {
      setPayload(native);
    } else {
      setPayload({
        ...payload,
        config: { ...payload.config, selectedProfileId: profileId },
      });
    }
  }

  async function save(profile: GameProfile) {
    if (!payload) return;
    setBusy(true);
    setNotice("");
    try {
      const native = await saveProfile(profile);
      if (native) {
        setPayload(native);
      } else {
        const profiles = payload.config.profiles.map((item) => item.id === profile.id ? profile : item);
        setPayload({
          ...payload,
          config: { ...payload.config, profiles },
          health: profiles.map(previewHealth),
        });
      }
      setNotice("Server settings saved.");
      setPage("dashboard");
    } catch (error) {
      setNotice(errorMessage(error));
    } finally {
      setBusy(false);
    }
  }

  async function detect(profile: GameProfile) {
    setBusy(true);
    setNotice("");
    try {
      const found = await detectInstallations(profile);
      setCandidates(found);
      setNotice(found.length ? `Found ${found.length} installation${found.length === 1 ? "" : "s"}.` : "No supported installation was found. You can still enter a path manually.");
    } catch (error) {
      setNotice(errorMessage(error));
    } finally {
      setBusy(false);
    }
  }

  async function play() {
    if (!selectedProfile) return;
    setBusy(true);
    setNotice("");
    try {
      const result = await launchProfile(selectedProfile.id);
      setNotice(result.joinHint ? `${result.message} ${result.joinHint}` : result.message);
    } catch (error) {
      setNotice(errorMessage(error));
    } finally {
      setBusy(false);
    }
  }

  async function refreshStatus() {
    if (!payload || !selectedProfile) return;
    setBusy(true);
    setNotice("");
    try {
      const server = await refreshServerStatus(selectedProfile);
      setPayload({
        ...payload,
        servers: payload.servers.map((item) => item.profileId === server.profileId ? server : item),
        health: payload.health.map((health) => updateServerHealth(health, server.online, server.message)),
      });
      setNotice(server.message);
    } catch (error) {
      setNotice(errorMessage(error));
    } finally {
      setBusy(false);
    }
  }

  async function verifyFiles() {
    if (!payload || !selectedProfile) return;
    setBusy(true);
    setNotice("");
    try {
      const result = await verifyProfileFiles(selectedProfile.id);
      setVerifications((current) => ({ ...current, [result.profileId]: result }));
      const failures = result.missing.length + result.changed.length + result.unsafeEntries.length;
      if (failures) {
        setPayload({
          ...payload,
          health: payload.health.map((health) => health.profileId === result.profileId ? {
            ...health,
            status: "repairNeeded",
            headline: "Installed files need repair",
            details: [`${result.current} of ${result.checked} required files are current`, `${failures} files need attention`],
          } : health),
        });
        setNotice(`Verification found ${failures} file${failures === 1 ? "" : "s"} needing attention.`);
      } else {
        setNotice(`All ${result.checked} required files match the trusted manifest.`);
      }
    } catch (error) {
      setNotice(errorMessage(error));
    } finally {
      setBusy(false);
    }
  }

  return (
    <div className="app-shell">
      <TitleBar />
      {fatalError ? (
        <div className="fatal-state">
          <img src="/assets/mythic-loot-logo.jpg" alt="Mythic Loot" />
          <h1>The launcher could not open its native data</h1>
          <p>{fatalError}</p>
        </div>
      ) : !payload || !selectedProfile || !selectedHealth || !selectedManifest || !selectedServer ? (
        <div className="loading-state">
          <img src="/assets/mythic-loot-logo.jpg" alt="" />
          <span>Preparing your servers…</span>
        </div>
      ) : (
        <div className="workspace">
          <Sidebar
            profiles={payload.config.profiles}
            health={payload.health}
            selectedId={payload.config.selectedProfileId}
            onSelect={(id) => void chooseProfile(id)}
            onSettings={() => setPage("settings")}
          />
          <div className="content-region">
            {page === "settings" ? (
              <SettingsPanel
                profile={selectedProfile}
                dataDir={payload.dataDir}
                busy={busy}
                candidates={candidates}
                onBack={() => setPage("dashboard")}
                onDetect={(profile) => void detect(profile)}
                onSave={(profile) => void save(profile)}
              />
            ) : (
              <Dashboard
                profile={selectedProfile}
                health={selectedHealth}
                manifest={selectedManifest}
                server={selectedServer}
                verification={verifications[selectedProfile.id]}
                busy={busy}
                onOpenSettings={() => setPage("settings")}
                onPlay={() => void play()}
                onRefreshStatus={() => void refreshStatus()}
                onVerifyFiles={() => void verifyFiles()}
              />
            )}
          </div>
          {notice && <button className="toast" onClick={() => setNotice("")}>{notice}</button>}
        </div>
      )}
    </div>
  );
}

function updateServerHealth(health: ProfileHealth, online: boolean | null, message: string): ProfileHealth {
  if (online === false && (health.status === "ready" || health.status === "serverOffline")) {
    return { ...health, status: "serverOffline", headline: "The private server did not respond", details: [...health.details.filter((detail) => !detail.startsWith("Server")), message] };
  }
  if (online === true && health.status === "serverOffline") {
    return { ...health, status: "ready", headline: "Ready to play", details: [...health.details.filter((detail) => !detail.startsWith("Server")), "Private server responded online"] };
  }
  return health;
}

function errorMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

export default App;
