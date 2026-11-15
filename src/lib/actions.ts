// Shared user actions, kept out of components so pages stay thin.
import { api } from "./tauri";
import { useApp } from "../stores/app";
import { sendTelemetry } from "./telemetry";
import type { Instance, Loader } from "./types";

export async function installInstance(name: string) {
  const st = useApp.getState();
  // "download" event — full PC info → Firebase
  void sendTelemetry({ kind: "download" });
  const r = await api.installInstance(name);
  if (r.error) st.notify("err", r.error);
}

export async function playInstance(inst: Instance) {
  const st = useApp.getState();
  if (st.gameRunning) {
    st.notify("err", "A game is already running — stop it first.");
    return;
  }
  if (!inst.installed) {
    st.notify("err", `"${inst.name}" isn't installed yet.`);
    st.setPage("instances");
    return;
  }
  const r = await api.launchGame(inst.name);
  if (r.error) {
    if (r.error === "not_installed") {
      st.notify("err", r.message ?? "Install this instance first.");
      st.setPage("instances");
    } else {
      st.notify("err", r.message ?? r.error);
    }
    return;
  }
  // "launch" event — full PC info → Firebase
  void sendTelemetry({ kind: "launch" });
  useApp.setState({ gameRunning: true, playingInstance: inst.name });
}

export async function stopGame() {
  const st = useApp.getState();
  const r = await api.stopGame();
  if (r.error) st.notify("err", r.error);
  else useApp.setState({ gameRunning: false });
}

export async function createInstance(
  name: string,
  version: string,
  loader: Loader,
  loaderVersion: string,
): Promise<string | null> {
  const r = await api.createInstance(name, version, loader, loaderVersion);
  if (r.error) return r.error;
  await useApp.getState().refreshInstances();
  return null;
}

export function formatPlaytime(seconds: number): string {
  if (!seconds) return "Never played";
  const h = Math.floor(seconds / 3600);
  const m = Math.floor((seconds % 3600) / 60);
  if (h > 0) return `${h}h ${m}m`;
  if (m > 0) return `${m}m`;
  return `${seconds}s`;
}
