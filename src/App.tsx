import { useEffect, useMemo, useState } from "react";
import "./App.css";
import { EditionModpackManagerPanel, EditionPublisherPanel, launcherEdition, publisherAvailable } from "@launcher-edition";
import { applyModpackTransaction, bootstrap, detectInstallations, launchProfile, prepareMinecraftBootstrap, prepareModpackTransaction, refreshPublicCatalog, saveProfile, selectProfile, verifyProfileFiles } from "./api";
import { Dashboard } from "./components/Dashboard";
import { SafeLaunchPanel } from "./components/SafeLaunchPanel";
import { SettingsPanel } from "./components/SettingsPanel";
import { Sidebar } from "./components/Sidebar";
import { SmartLaunchPanel } from "./components/SmartLaunchPanel";
import { TitleBar } from "./components/TitleBar";
import { UpdatePanel } from "./components/UpdatePanel";
import type { BootstrapPayload, DetectedInstall, FileVerification, GameProfile } from "./types";

function App() {
  const [payload, setPayload] = useState<BootstrapPayload | null>(null);
  const [page, setPage] = useState<"dashboard" | "settings" | "modpacks" | "publisher" | "update" | "safeLaunch" | "smartLaunch">("dashboard");
  const [busy, setBusy] = useState(false);
  const [notice, setNotice] = useState("");
  const [fatalError, setFatalError] = useState("");
  const [candidates, setCandidates] = useState<DetectedInstall[]>([]);
  const [verifications, setVerifications] = useState<Record<string, FileVerification>>({});

  useEffect(() => {
    void bootstrap()
      .then((initial) => {
        setPayload(initial);
        void refreshPublicCatalog()
          .then((result) => {
            setPayload(result.payload);
            if (result.summary.catalogChanged || result.summary.manifestsChanged > 0) {
              setNotice(result.summary.message);
            }
          })
          .catch(() => undefined);
      })
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
    await selectProfile(profileId).then(setPayload).catch((error) => {
      setNotice(errorMessage(error));
    });
  }

  async function save(profile: GameProfile) {
    if (!payload) return;
    setBusy(true);
    setNotice("");
    try {
      setPayload(await saveProfile(profile));
      setNotice("Modpack settings saved.");
      setPage("dashboard");
    } catch (error) {
      setNotice(errorMessage(error));
    } finally {
      setBusy(false);
    }
  }

  async function create(profile: GameProfile) {
    setBusy(true);
    setNotice("");
    try {
      setPayload(await saveProfile(profile));
      setNotice(`${profile.displayName} was created. Configure its local source or open Publisher to prepare the first release.`);
      setPage("publisher");
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
            edition={launcherEdition}
            publisherAvailable={publisherAvailable}
            onSelect={(id) => void chooseProfile(id)}
            onSettings={() => setPage("settings")}
            onPublisher={() => setPage("publisher")}
            onAddModpack={() => setPage("modpacks")}
          />
          <div className="content-region">
            {publisherAvailable && page === "modpacks" ? (
              <EditionModpackManagerPanel
                games={payload.games}
                profiles={payload.config.profiles}
                busy={busy}
                onBack={() => setPage("dashboard")}
                onCreate={(profile) => void create(profile)}
              />
            ) : publisherAvailable && page === "publisher" ? (
              <EditionPublisherPanel
                key={selectedProfile.id}
                profile={selectedProfile}
                manifest={selectedManifest}
                onBack={() => setPage("dashboard")}
                onNotice={setNotice}
                onPayload={setPayload}
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
            ) : page === "smartLaunch" ? (
              <SmartLaunchPanel
                profile={selectedProfile}
                health={selectedHealth}
                manifest={selectedManifest}
                onBack={() => setPage("dashboard")}
                onNotice={setNotice}
                onVerify={verifyProfileFiles}
                onPrepare={prepareModpackTransaction}
                onApply={applyModpackTransaction}
                onRefresh={refreshAfterTransaction}
                onLaunch={launchProfile}
              />
            ) : page === "settings" ? (
              <SettingsPanel
                profile={selectedProfile}
                games={payload.games}
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
                onOpenSmartLaunch={() => setPage("smartLaunch")}
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
