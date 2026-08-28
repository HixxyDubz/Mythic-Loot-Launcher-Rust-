import { useEffect, useMemo, useState } from "react";
import "./App.css";
import { bootstrap, detectInstallations, launchProfile, prepareMinecraftBootstrap, saveProfile, selectProfile, verifyProfileFiles } from "./api";
import { Dashboard } from "./components/Dashboard";
import { PublisherPanel } from "./components/PublisherPanel";
import { SafeLaunchPanel } from "./components/SafeLaunchPanel";
import { SettingsPanel } from "./components/SettingsPanel";
import { Sidebar } from "./components/Sidebar";
import { TitleBar } from "./components/TitleBar";
import { UpdatePanel } from "./components/UpdatePanel";
import { previewHealth } from "./mock";
import type { BootstrapPayload, DetectedInstall, FileVerification, GameProfile } from "./types";

function App() {
  const [payload, setPayload] = useState<BootstrapPayload | null>(null);
  const [page, setPage] = useState<"dashboard" | "settings" | "publisher" | "update" | "safeLaunch">("dashboard");
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
      setNotice("Modpack settings saved.");
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
      setNotice(result.message);
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

  async function refreshAfterTransaction() {
    const refreshed = await bootstrap();
    setPayload(refreshed);
    setVerifications((current) => {
      const next = { ...current };
      if (selectedProfile) delete next[selectedProfile.id];
      return next;
    });
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
      ) : !payload || !selectedProfile || !selectedHealth || !selectedManifest ? (
        <div className="loading-state">
          <img src="/assets/mythic-loot-logo.jpg" alt="" />
          <span>Preparing your modpacks…</span>
        </div>
      ) : (
        <div className="workspace">
          <Sidebar
            profiles={payload.config.profiles}
            health={payload.health}
            selectedId={payload.config.selectedProfileId}
            onSelect={(id) => void chooseProfile(id)}
            onSettings={() => setPage("settings")}
            onPublisher={() => setPage("publisher")}
          />
          <div className="content-region">
            {page === "publisher" ? (
              <PublisherPanel
                profile={selectedProfile}
                onBack={() => setPage("dashboard")}
                onNotice={setNotice}
              />
            ) : page === "update" ? (
              <UpdatePanel
                profile={selectedProfile}
                health={selectedHealth}
                manifest={selectedManifest}
                onBack={() => setPage("dashboard")}
                onNotice={setNotice}
                onCompleted={() => refreshAfterTransaction()}
              />
            ) : page === "safeLaunch" ? (
              <SafeLaunchPanel
                profile={selectedProfile}
                health={selectedHealth}
                manifest={selectedManifest}
                onBack={() => setPage("dashboard")}
                onNotice={setNotice}
              />
            ) : page === "settings" ? (
              <SettingsPanel
                profile={selectedProfile}
                dataDir={payload.dataDir}
                busy={busy}
                candidates={candidates}
                onBack={() => setPage("dashboard")}
                onDetect={(profile) => void detect(profile)}
                onSave={(profile) => void save(profile)}
                onPrepareMinecraftBootstrap={prepareMinecraftBootstrap}
                onNotice={setNotice}
              />
            ) : (
              <Dashboard
                profile={selectedProfile}
                health={selectedHealth}
                manifest={selectedManifest}
                verification={verifications[selectedProfile.id]}
                busy={busy}
                onOpenSettings={() => setPage("settings")}
                onPlay={() => void play()}
                onVerifyFiles={() => void verifyFiles()}
                onOpenUpdates={() => setPage("update")}
                onOpenSafeLaunch={() => setPage("safeLaunch")}
              />
            )}
          </div>
          {notice && <button className="toast" onClick={() => setNotice("")}>{notice}</button>}
        </div>
      )}
    </div>
  );
}

function errorMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

export default App;
