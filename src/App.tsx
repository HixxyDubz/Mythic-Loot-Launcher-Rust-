import { useEffect, useMemo, useRef, useState } from "react";
import { isTauri } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import "./App.css";
import { EditionModpackManagerPanel, EditionPublisherPanel, launcherEdition, publisherAvailable } from "@launcher-edition";
import { applyModpackTransaction, bootstrap, checkAppUpdate, detectInstallations, getAppUpdateResult, launchProfile, prepareMinecraftBootstrap, prepareModpackTransaction, refreshPublicCatalog, savePreferences, saveProfile, selectProfile, verifyProfileFiles } from "./api";
import { AppUpdatePanel } from "./components/AppUpdatePanel";
import { Dashboard } from "./components/Dashboard";
import { ActivityPanel } from "./components/ActivityPanel";
import { SafeLaunchPanel } from "./components/SafeLaunchPanel";
import { SettingsPanel } from "./components/SettingsPanel";
import { Sidebar } from "./components/Sidebar";
import { SmartLaunchPanel } from "./components/SmartLaunchPanel";
import { StoragePanel } from "./components/StoragePanel";
import { SupportPanel } from "./components/SupportPanel";
import { TitleBar } from "./components/TitleBar";
import { UpdatePanel } from "./components/UpdatePanel";
import type { BootstrapPayload, DetectedInstall, FileVerification, GameProfile, LauncherPreferences } from "./types";

function App() {
  const [payload, setPayload] = useState<BootstrapPayload | null>(null);
  const [page, updatePage] = useState<"dashboard" | "activity" | "storage" | "support" | "appUpdate" | "settings" | "modpacks" | "publisher" | "update" | "safeLaunch" | "smartLaunch">("dashboard");
  const interacted = useRef(false);
  function setPage(next: typeof page) { interacted.current = true; updatePage(next); }
  const [busy, setBusy] = useState(false);
  const [maintenanceBusy, setMaintenanceBusy] = useState(false);
  const [notice, setNotice] = useState("");
  const [fatalError, setFatalError] = useState("");
  const [candidates, setCandidates] = useState<DetectedInstall[]>([]);
  const [verifications, setVerifications] = useState<Record<string, FileVerification>>({});

  useEffect(() => {
    if (!isTauri()) return;
    const subscription = listen<string>("launcher-close-blocked", (event) => setNotice(event.payload));
    return () => { void subscription.then((unlisten) => unlisten()); };
  }, []);

  useEffect(() => {
    let active = true;
    void bootstrap()
      .then((initial) => {
        if (!active) return;
        setPayload(initial);
        if (!initial.config.preferences.autoCheckUpdates) return;
        void refreshPublicCatalog()
          .then((result) => {
            if (!active) return;
            // A late startup response must not replace a newer local edit,
            // preference save or profile selection.
            setPayload((current) => current === initial && !interacted.current ? result.payload : current);
            if (result.summary.catalogChanged || result.summary.manifestsChanged > 0) {
              setNotice(result.summary.message);
            }
          })
          .catch(() => undefined);
        void checkAppUpdate()
          .then((update) => {
            if (active && update.canInstall) setNotice(update.message);
          })
          .catch(() => undefined);
      })
      .catch((error) => { if (active) setFatalError(errorMessage(error)); });
    void getAppUpdateResult()
      .then((result) => {
        if (active && result) setNotice(result.message);
      })
      .catch(() => undefined);
    return () => { active = false; };
  }, []);

  async function saveLauncherPreferences(preferences: LauncherPreferences) {
    setBusy(true);
    try {
      const saved = await savePreferences(preferences);
      setPayload((current) => current && ({ ...current, config: { ...current.config, preferences: saved } }));
    } finally { setBusy(false); }
  }

  async function refreshCatalogue() {
    setBusy(true);
    try {
      const result = await refreshPublicCatalog();
      setPayload(result.payload);
      setNotice(result.summary.message);
    } catch (error) { setNotice(errorMessage(error)); }
    finally { setBusy(false); }
  }

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
    if (!payload || busy || maintenanceBusy || profileId === payload.config.selectedProfileId) return;
    setBusy(true);
    setCandidates([]);
    await selectProfile(profileId).then(setPayload).catch((error) => {
      setNotice(errorMessage(error));
    }).finally(() => setBusy(false));
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
        setPayload((current) => current && ({
          ...current,
          health: current.health.map((health) => health.profileId === result.profileId ? {
            ...health,
            status: "repairNeeded",
            headline: "Installed files need repair",
            details: [`${result.current} of ${result.checked} required files are current`, `${failures} files need attention`],
          } : health),
        }));
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
    <div className="app-shell"
      data-theme={payload?.config.preferences.theme ?? "amethyst"}
      data-font={payload?.config.preferences.font ?? "system"}
      data-reduce-motion={payload?.config.preferences.reduceMotion ?? false}
      data-decorative-background={payload?.config.preferences.decorativeBackground ?? true}
    >
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
            disabled={busy || maintenanceBusy}
            onSelect={(id) => void chooseProfile(id)}
            onSettings={() => setPage("settings")}
            onActivity={() => setPage("activity")}
            onStorage={() => setPage("storage")}
            onSupport={() => setPage("support")}
            onAppUpdate={() => setPage("appUpdate")}
            onPublisher={() => setPage("publisher")}
            onAddModpack={() => setPage("modpacks")}
          />
          <div className="content-region">
            {page === "activity" ? (
              <ActivityPanel onBack={() => setPage("dashboard")} onNotice={setNotice} />
            ) : page === "storage" ? (
              <StoragePanel onBack={() => setPage("dashboard")} onNotice={setNotice} />
            ) : page === "support" ? (
              <SupportPanel key={selectedProfile.id} profile={selectedProfile} onBack={() => setPage("dashboard")} onNotice={setNotice} />
            ) : page === "appUpdate" ? (
              <AppUpdatePanel onBack={() => setPage("dashboard")} onNotice={setNotice} />
            ) : publisherAvailable && page === "modpacks" ? (
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
                onBusyChange={setMaintenanceBusy}
                profile={selectedProfile}
                health={selectedHealth}
                manifest={selectedManifest}
                onBack={() => setPage("dashboard")}
                onNotice={setNotice}
                onCompleted={() => refreshAfterTransaction()}
              />
            ) : page === "safeLaunch" ? (
              <SafeLaunchPanel
                key={`${selectedProfile.id}|${selectedProfile.installDir}`}
                profile={selectedProfile}
                health={selectedHealth}
                manifest={selectedManifest}
                onBack={() => setPage("dashboard")}
                onNotice={setNotice}
              />
            ) : page === "smartLaunch" ? (
              <SmartLaunchPanel
                onBusyChange={setMaintenanceBusy}
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
                key={selectedProfile.id}
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
                preferences={payload.config.preferences}
                onSavePreferences={saveLauncherPreferences}
                onRefreshCatalogue={() => void refreshCatalogue()}
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
