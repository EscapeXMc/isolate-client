// Thin, typed wrapper around the Tauri IPC surface.
import { invoke } from "@tauri-apps/api/tauri";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type {
  AiContext,
  AiMessage,
  AppInfo,
  BoostResult,
  Instance,
  InstallProgress,
  JavaInstall,
  Loader,
  ManagedServer,
  ModConflict,
  ModEntry,
  ModProfile,
  ModrinthSearchResult,
  ModrinthVersion,
  MsDeviceCode,
  MultiplayerServer,
  NewsItem,
  OptimizerStatus,
  PackFile,
  PcHardware,
  ServerFileEntry,
  ServerPing,
  Settings,
  SystemInfo,
  TemplateInfo,
  UpdateInfo,
  VersionGroup,
} from "./types";

/** Generic JSON result — most backend commands return `{success?, error?}` + payload. */
export type Json = Record<string, unknown>;

export const api = {
  // app / settings
  appInfo: () => invoke<AppInfo>("get_app_info"),
  getSettings: () => invoke<Settings>("get_settings"),
  saveSettings: (partial: Partial<Settings>) =>
    invoke<{ success: boolean }>("save_settings", { partial }),
  saveSetting: (key: string, value: unknown) =>
    invoke<{ success: boolean }>("save_setting", { key, value }),
  systemInfo: () => invoke<SystemInfo>("get_system_info"),
  hardwareInfo: () => invoke<PcHardware & { version?: string }>("get_hardware_info"),
  openPath: (path: string) => invoke<{ success?: boolean; error?: string }>("open_path", { path }),

  // instances
  getInstances: () => invoke<Instance[]>("get_instances"),
  createInstance: (name: string, version: string, loader: Loader, loaderVersion: string) =>
    invoke<{ success?: boolean; error?: string; instance?: Instance }>("create_instance", {
      name,
      version,
      loader,
      loaderVersion,
    }),
  deleteInstance: (name: string) =>
    invoke<{ success?: boolean; error?: string }>("delete_instance", { name }),
  renameInstance: (name: string, newName: string) =>
    invoke<{ success?: boolean; error?: string }>("rename_instance", { name, newName }),
  getInstanceSettings: (name: string) =>
    invoke<Record<string, unknown>>("get_instance_settings", { name }),
  saveInstanceSettings: (name: string, settings: Record<string, unknown>) =>
    invoke<{ success: boolean }>("save_instance_settings", { name, settings }),
  openInstanceFolder: (name: string) =>
    invoke<{ success?: boolean; error?: string }>("open_instance_folder", { name }),
  getPlayTimes: () => invoke<Record<string, number>>("get_play_times"),

  // versions
  getMinecraftVersions: (includeSnapshots = false) =>
    invoke<{ versions?: VersionGroup[]; error?: string }>("get_minecraft_versions", {
      includeSnapshots,
    }),
  getLoaderVersions: (loader: Loader) =>
    invoke<{ versions?: string[]; error?: string }>("get_loader_versions", { loader }),
  getSupportedLoaders: () =>
    invoke<{ id: Loader; label: string; min_version?: string }[]>("get_supported_loaders"),

  // java
  detectJava: (rescan = false) =>
    invoke<{ java: JavaInstall[]; configured: string }>("detect_java", { rescan }),
  installJava: (major: number) =>
    invoke<{ success?: boolean; error?: string; java?: JavaInstall }>("install_java", { major }),

  // install / launch
  installInstance: (name: string) =>
    invoke<{ status?: string; error?: string }>("install_instance", { name }),
  launchGame: (name: string) =>
    invoke<{ success?: boolean; error?: string; message?: string; pid?: number }>("launch_game", {
      name,
    }),
  stopGame: () => invoke<{ success?: boolean; error?: string }>("stop_game"),
  getGameState: () => invoke<{ running: boolean; pid?: number | null }>("get_game_state"),

  // logs
  getLogs: () => invoke<string[]>("get_logs"),
  clearLogs: () => invoke<{ success: boolean }>("clear_logs"),
  getGameOutput: () => invoke<string[]>("get_game_output"),
  clearGameOutput: () => invoke<{ success: boolean }>("clear_game_output"),

  // auth
  createOfflineAccount: (username: string) =>
    invoke<{ success?: boolean; error?: string; username?: string; uuid?: string }>(
      "create_offline_account",
      { username },
    ),
  msStart: () => invoke<MsDeviceCode | { error: string }>("authenticate_microsoft"),
  msPoll: () =>
    invoke<
      | { status: "pending" }
      | { status: "done"; result: { access_token: string; refresh_token: string } }
      | { status: "error"; error: string }
      | { status: "idle" }
    >("poll_microsoft_status"),
  msComplete: (accessToken: string, refreshToken: string) =>
    invoke<{ success?: boolean; error?: string; username?: string; uuid?: string }>(
      "complete_microsoft_auth_cmd",
      { accessToken, refreshToken },
    ),
  signOut: () => invoke<{ success: boolean }>("sign_out"),

  // ---------- Ely.by ----------
  elyStart: () => invoke<Json>("authenticate_elyby"),
  elyPoll: () =>
    invoke<
      | { status: "pending" }
      | { status: "done"; username?: string; uuid?: string }
      | { status: "error"; error: string }
      | Json
    >("poll_elyby_auth"),
  elyComplete: (code: string) => invoke<Json>("complete_elyby_auth_cmd", { code }),
  elySignOut: () => invoke<Json>("sign_out_elyby"),
  ensureAuthlib: () => invoke<Json>("ensure_authlib_injector"),

  // ---------- skins ----------
  getSkinInfo: () => invoke<Json>("get_skin_info"),
  setCustomSkin: (filePath: string) => invoke<Json>("set_custom_skin", { filePath }),
  clearCustomSkin: () => invoke<Json>("clear_custom_skin"),

  // ---------- Modrinth ----------
  searchModrinth: (
    query: string,
    projectType = "mod",
    loader = "",
    mcVersion = "",
    limit = 20,
  ) =>
    invoke<ModrinthSearchResult | Json>("search_modrinth", {
      query,
      projectType,
      loader,
      mcVersion,
      limit,
    }),
  getModrinthProject: (slug: string) => invoke<Json>("get_modrinth_project", { slug }),
  getModrinthVersions: (slug: string, loader = "", mcVersion = "") =>
    invoke<ModrinthVersion[] | Json>("get_modrinth_versions", { slug, loader, mcVersion }),
  installMod: (instance: string, slug: string, versionId = "") =>
    invoke<Json>("install_mod", { instance, slug, versionId }),
  installResourcepack: (instance: string, slug: string, versionId = "") =>
    invoke<Json>("install_resourcepack", { instance, slug, versionId }),
  listMods: (instance: string) =>
    invoke<{ mods?: PackFile[]; manifest?: unknown; error?: string }>("list_mods", { instance }),
  listResourcepacks: (instance: string) =>
    invoke<{ packs?: PackFile[]; error?: string }>("list_resourcepacks", { instance }),
  deletePackFile: (instance: string, sub: string, filename: string) =>
    invoke<Json>("delete_pack_file", { instance, sub, filename }),
  toggleMod: (instance: string, filename: string, enabled: boolean) =>
    invoke<Json>("toggle_mod", { instance, filename, enabled }),
  batchInstallMods: (instance: string, slugs: string[]) =>
    invoke<Json>("batch_install_mods", { instance, slugs }),
  checkModUpdates: (instance: string) => invoke<Json>("check_mod_updates", { instance }),
  resolveModDependencies: (instance: string, slug: string) =>
    invoke<Json>("resolve_mod_dependencies", { instance, slug }),
  detectModConflicts: (instance: string) =>
    invoke<{ conflicts?: ModConflict[]; error?: string }>("detect_mod_conflicts", { instance }),
  installModpack: (instance: string, url: string) =>
    invoke<Json>("install_modpack", { instance, url }),
  createInstanceFromModpack: (name: string, url: string) =>
    invoke<Json>("create_instance_from_modpack", { name, url }),
  exportMrpack: (instance: string, outputPath: string) =>
    invoke<Json>("export_instance_mrpack", { instance, outputPath }),

  // ---------- instance extras (Batch A) ----------
  getInstanceMods: (instanceName: string) =>
    invoke<{ mods?: ModEntry[] }>("get_instance_mods", { instanceName }),
  getInstanceResourcepacks: (instanceName: string) =>
    invoke<{ packs?: PackFile[] }>("get_instance_resourcepacks", { instanceName }),
  deleteMod: (instanceName: string, filename: string) =>
    invoke<Json>("delete_mod", { instanceName, filename }),
  deleteResourcepack: (instanceName: string, filename: string) =>
    invoke<Json>("delete_resourcepack", { instanceName, filename }),
  moveModFile: (instanceName: string, sourcePath: string) =>
    invoke<Json>("move_mod_file", { instanceName, sourcePath }),
  moveResourcepackFile: (instanceName: string, sourcePath: string) =>
    invoke<Json>("move_resourcepack_file", { instanceName, sourcePath }),
  autoEnableResourcepack: (instanceName: string, filename: string) =>
    invoke<Json>("auto_enable_resourcepack", { instanceName, filename }),
  editInstance: (
    instanceName: string,
    newName: string,
    mcVersion: string,
    loader: string,
    loaderVersion: string,
  ) =>
    invoke<Json>("edit_instance", {
      instanceName,
      newName,
      mcVersion,
      loader,
      loaderVersion,
    }),
  getInstanceIcon: (instanceName: string) =>
    invoke<{ success?: boolean; data?: string; error?: string }>("get_instance_icon", {
      instanceName,
    }),
  setInstanceIcon: (instanceName: string, iconData: string) =>
    invoke<Json>("set_instance_icon", { instanceName, iconData }),
  openInstancePath: (instanceName: string, subFolder: string) =>
    invoke<Json>("open_instance_path", { instanceName, subFolder }),
  pinInstance: (instance: string) => invoke<Json>("pin_instance", { instance }),
  unpinInstance: (instance: string) => invoke<Json>("unpin_instance", { instance }),
  getScreenshots: (instance: string) => invoke<Json>("get_screenshots", { instance }),
  cloneInstance: (instance: string, newName: string) =>
    invoke<Json>("clone_instance", { instance, newName }),
  exportInstanceConfig: (instance: string, outputPath: string) =>
    invoke<Json>("export_instance_config", { instance, outputPath }),
  importInstanceConfig: (filePath: string) =>
    invoke<Json>("import_instance_config", { filePath }),
  analyzeCrashLog: (instance: string) => invoke<Json>("analyze_crash_log", { instance }),
  clearInstanceCache: (instance: string) => invoke<Json>("clear_instance_cache", { instance }),
  getGameLogs: (instance: string) => invoke<Json>("get_game_logs", { instance }),
  launchServer: (instanceName: string) => invoke<Json>("launch_server", { instanceName }),

  // ---------- java ----------
  getJavaInstallations: () =>
    invoke<{ java?: JavaInstall[]; error?: string }>("get_java_installations"),
  checkJavaCompatibility: (mcVersion: string) =>
    invoke<Json>("check_java_compatibility", { mcVersion }),
  downloadJava: (majorVersion: number) => invoke<Json>("download_java", { majorVersion }),
  setJavaForInstance: (javaPath: string) => invoke<Json>("set_java_for_instance", { javaPath }),
  getFabricVersions: () => invoke<Json>("get_fabric_versions"),

  // ---------- templates ----------
  getTemplates: () =>
    invoke<{ templates?: TemplateInfo[]; error?: string }>("get_instance_templates"),
  createFromTemplate: (name: string, templateId: string) =>
    invoke<Json>("create_instance_from_template", { name, templateId }),
  applyTemplate: (instanceName: string, templateId: string) =>
    invoke<Json>("apply_instance_template", { instanceName, templateId }),

  // ---------- performance ----------
  installPerfMods: (instance: string, mcVersion: string, loader: string) =>
    invoke<Json>("install_performance_mods", { instance, mcVersion, loader }),
  getOptimizerStatus: (instance: string) =>
    invoke<OptimizerStatus | Json>("get_optimizer_status", { instance }),
  applyOptimizer: (instance: string, settings: Record<string, unknown>) =>
    invoke<Json>("apply_optimizer", { instance, settings }),
  applyFpsPreset: (instance: string, preset: string) =>
    invoke<Json>("apply_fps_preset", { instance, preset }),
  gameBoost: (instanceName: string) => invoke<BoostResult | Json>("game_boost", { instanceName }),
  getPublicIp: () => invoke<Json>("get_public_ip"),

  // Discord REST proxy (token stays client-side; requests go through Rust
  // because discord.com blocks browser CORS).
  discordApi: (method: string, path: string, token: string, body?: unknown) =>
    invoke<{ ok: boolean; status: number; body?: unknown; error?: string }>("discord_api", {
      method,
      path,
      token,
      body: body === undefined ? undefined : JSON.stringify(body),
    }),
  // Public, tokenless invite info: live member/online counts + online-member
  // list from the guild widget (right-hub community card).
  discordInviteInfo: (code: string) => invoke<{
    ok: boolean;
    members?: number;
    online?: number;
    guild_name?: string;
    icon?: string | null;
    list?: { id: string; name: string; avatar?: string | null; status?: string }[];
    error?: string;
  }>("discord_invite_info", { code }),
  // Windows optimization suite (Phases 1-5) — also auto-runs on game launch.
  runSystemOptimizer: () => invoke<Json>("run_system_optimizer"),

  // ---------- multiplayer ----------
  getServers: () => invoke<MultiplayerServer[]>("get_servers"),
  addServer: (name: string, address: string, port: number) =>
    invoke<Json>("add_server", { name, address, port }),
  editServer: (name: string, address: string, port: number) =>
    invoke<Json>("edit_server", { name, address, port }),
  deleteServer: (name: string) => invoke<Json>("delete_server", { name }),
  pingServer: (address: string, port: number) =>
    invoke<ServerPing | Json>("ping_server", { address, port }),
  getServerStatus: (host: string, port: string) =>
    invoke<Json>("get_server_status", { host, port }),

  // ---------- server manager ----------
  serverTypes: () => invoke<Json>("server_mgr_server_types"),
  serverVersions: (serverType: string) =>
    invoke<{ versions?: string[]; error?: string }>("server_mgr_list_versions", { serverType }),
  serverBuilds: (mcVersion: string) => invoke<Json>("get_server_builds", { mcVersion }),
  paperVersions: () => invoke<Json>("get_server_versions"),
  serverList: () =>
    invoke<{ servers?: ManagedServer[]; error?: string }>("server_mgr_list"),
  serverCreate: (
    name: string,
    serverType: string,
    mcVersion: string,
    port: number,
    motd?: string,
    ramMb?: number,
  ) =>
    invoke<Json>("server_mgr_create", {
      name,
      serverType,
      mcVersion,
      port,
      motd: motd ?? null,
      ramMb: ramMb ?? null,
    }),
  serverStart: (name: string) => invoke<Json>("server_mgr_start", { name }),
  serverStop: (name: string) => invoke<Json>("server_mgr_stop", { name }),
  serverKill: (name: string) => invoke<Json>("server_mgr_kill", { name }),
  serverCommand: (name: string, command: string) =>
    invoke<Json>("server_mgr_command", { name, command }),
  serverStatus: (name: string) =>
    invoke<{ running?: boolean; error?: string }>("server_mgr_status", { name }),
  serverLogs: (name: string) =>
    invoke<{ success?: boolean; lines?: string[]; error?: string }>("server_mgr_logs", { name }),
  serverClearLogs: (name: string) => invoke<Json>("server_mgr_clear_logs", { name }),
  serverDelete: (name: string) => invoke<Json>("server_mgr_delete", { name }),
  serverStats: (name: string) => invoke<Json>("server_mgr_stats", { name }),
  serverGetProperties: (name: string) =>
    invoke<{ success?: boolean; properties?: Record<string, string>; error?: string }>(
      "server_mgr_get_properties",
      { name },
    ),
  serverSetProperties: (name: string, properties: Record<string, string>) =>
    invoke<Json>("server_mgr_set_properties", { name, properties }),
  serverSetMeta: (name: string, ramMb: number | null, motd: string | null, port: number | null) =>
    invoke<Json>("server_mgr_set_meta", { name, ramMb, motd, port }),
  serverUpdateMeta: (name: string, patch: Record<string, unknown>) =>
    invoke<Json>("server_mgr_update_meta", { name, patch }),
  serverEula: (name: string, accept: boolean) =>
    invoke<Json>("server_mgr_eula", { name, accept }),
  serverClone: (source: string, newName: string, newPort: number | null) =>
    invoke<Json>("server_mgr_clone", { source, newName, newPort }),
  serverReinstall: (name: string, version: string) =>
    invoke<Json>("server_mgr_reinstall", { name, version }),
  serverSearchPlugins: (query: string, mcVersion: string, limit = 15) =>
    invoke<ModrinthSearchResult | Json>("server_mgr_search_plugins", { query, mcVersion, limit }),
  serverInstallPlugin: (name: string, slug: string, versionId = "") =>
    invoke<Json>("server_mgr_install_plugin", { name, slug, versionId }),
  serverListPlugins: (name: string) =>
    invoke<{ success?: boolean; plugins?: PackFile[]; error?: string }>(
      "server_mgr_list_plugins",
      { name },
    ),
  serverDeletePlugin: (name: string, filename: string) =>
    invoke<Json>("server_mgr_delete_plugin", { name, filename }),
  serverTogglePlugin: (name: string, filename: string) =>
    invoke<Json>("server_mgr_plugin_toggle", { name, filename }),
  serverFilesList: (name: string, subpath = "") =>
    invoke<{ success?: boolean; entries?: ServerFileEntry[]; error?: string }>(
      "server_mgr_files_list",
      { name, subpath },
    ),
  serverFileRead: (name: string, path: string) =>
    invoke<{ content?: string; error?: string }>("server_mgr_file_read", { name, path }),
  serverFileWrite: (name: string, path: string, content: string) =>
    invoke<Json>("server_mgr_file_write", { name, path, content }),
  serverFileDelete: (name: string, path: string) =>
    invoke<Json>("server_mgr_file_delete", { name, path }),
  serverFileMkdir: (name: string, path: string) =>
    invoke<Json>("server_mgr_file_mkdir", { name, path }),
  serverFileRename: (name: string, path: string, newName: string) =>
    invoke<Json>("server_mgr_file_rename", { name, path, newName }),
  serverFileUpload: (name: string, subpath: string, filename: string, data: string) =>
    invoke<Json>("server_mgr_file_upload", { name, subpath, filename, data }),
  serverFileReadB64: (name: string, path: string) =>
    invoke<Json>("server_mgr_file_read_b64", { name, path }),
  serverFileDownload: (name: string, path: string) =>
    invoke<Json>("server_mgr_file_download", { name, path }),
  serverReadIcon: (name: string) => invoke<Json>("server_mgr_read_icon", { name }),
  serverBackups: (name: string) =>
    invoke<{ success?: boolean; backups?: { filename: string; size?: number }[]; error?: string }>(
      "server_mgr_backup_list",
      { name },
    ),
  serverBackupCreate: (name: string, label?: string) =>
    invoke<Json>("server_mgr_backup_create", { name, label: label ?? null }),
  serverBackupRestore: (name: string, file: string) =>
    invoke<Json>("server_mgr_backup_restore", { name, file }),
  serverBackupDelete: (name: string, file: string) =>
    invoke<Json>("server_mgr_backup_delete", { name, file }),
  serverWorlds: (name: string) =>
    invoke<{ success?: boolean; worlds?: string[]; error?: string }>("server_mgr_list_worlds", {
      name,
    }),
  serverDeleteWorld: (name: string, world: string) =>
    invoke<Json>("server_mgr_delete_world", { name, world }),
  serverResetWorld: (name: string, world: string) =>
    invoke<Json>("server_mgr_reset_world", { name, world }),
  serverCheckPort: (port: number) => invoke<Json>("server_mgr_check_port", { port }),
  serverNextPort: (start?: number) =>
    invoke<Json>("server_mgr_next_port", { start: start ?? null }),
  serverListJava: () => invoke<Json>("server_mgr_list_java"),
  serverOpenFolder: (name: string) => invoke<Json>("server_mgr_open_folder", { name }),

  // ---------- mod profiles ----------
  saveModProfile: (name: string, instanceName: string) =>
    invoke<Json>("save_mod_profile", { name, instanceName }),
  loadModProfiles: () => invoke<ModProfile[]>("load_mod_profiles"),
  deleteModProfile: (name: string) => invoke<Json>("delete_mod_profile", { name }),
  applyModProfile: (name: string, instanceName: string) =>
    invoke<Json>("apply_mod_profile", { name, instanceName }),

  // ---------- updater ----------
  checkLauncherUpdate: () => invoke<UpdateInfo | Json>("check_launcher_update"),
  checkUpdates: () => invoke<UpdateInfo | Json>("check_updates"),
  getLatestReleaseTag: () => invoke<Json>("get_latest_release_tag"),
  downloadUpdate: (url: string, filename: string, sha256?: string) =>
    invoke<Json>("download_update", { url, filename, sha256 }),
  installUpdate: (path: string) => invoke<Json>("install_update", { path }),
  // Auto-check on boot (anti-loop filtered) + guard bookkeeping.
  startupUpdateCheck: () => invoke<UpdateInfo | Json>("startup_update_check"),
  // Live release state (GitHub ⊕ Firebase RTDB) + real-time listener.
  checkReleaseLive: () => invoke<UpdateInfo | Json>("check_release_live"),
  startReleaseListener: () => invoke<Json>("start_release_listener"),
  markUpdateAttempt: (version: string) =>
    invoke<Json>("mark_update_attempt", { version }),
  dismissUpdate: (version: string) => invoke<Json>("dismiss_update", { version }),

  // ---------- news / backgrounds ----------
  getNews: () => invoke<NewsItem[]>("get_minecraft_news"),
  getNewsEnhanced: () => invoke<NewsItem[]>("get_minecraft_news_enhanced"),
  saveBackgroundImage: (data: string) => invoke<{ success?: boolean; path?: string; error?: string }>(
    "save_background_image",
    { data },
  ),
  getBackgroundImagePath: () => invoke<string>("get_background_image_path"),
  deleteBackgroundImage: () => invoke<Json>("delete_background_image"),
  saveBackgroundVideo: (data: string) => invoke<{ success?: boolean; path?: string; error?: string }>(
    "save_background_video",
    { data },
  ),
  getBackgroundVideoPath: () => invoke<string>("get_background_video_path"),
  deleteBackgroundVideo: () => invoke<Json>("delete_background_video"),

  // ---------- system / misc ----------
  runSystemCommand: (command: string) => invoke<Json>("run_system_command", { command }),
  getSettingCmd: (key: string, dflt: unknown = null) =>
    invoke<Json>("get_setting_cmd", { key, default: dflt }),
  saveSettingCmd: (key: string, value: unknown) =>
    invoke<Json>("save_setting_cmd", { key, value }),
  uninstallLauncher: (deleteData: boolean) =>
    invoke<Json>("uninstall_launcher", { deleteData }),
  imageCacheGet: (url: string) => invoke<Json>("image_cache_get", { url }),
  imageCacheWarm: (urls: string[]) => invoke<Json>("image_cache_warm", { urls }),
  getSelfhostStatus: () => invoke<Json>("get_selfhost_status"),
  downloadSelfhost: () => invoke<Json>("download_selfhost"),
  launchSelfhost: () => invoke<Json>("launch_selfhost"),
  fetchPartnersRemote: () => invoke<Json>("fetch_partners_remote"),
  showNotification: (title: string, body: string) =>
    invoke<Json>("show_notification", { title, body }),
  playNotificationSound: () => invoke<Json>("play_notification_sound"),
  readFilePath: (path: string) =>
    invoke<{ content?: string; error?: string }>("read_file_content", { path }),
  writeFilePath: (path: string, content: string) =>
    invoke<Json>("write_file_content", { path, content }),
  listDirectory: (path: string) =>
    invoke<{
      dirs?: { name: string; size?: number; modified?: number }[];
      files?: { name: string; size?: number; modified?: number }[];
      error?: string;
    }>("list_directory", { path }),
  deletePath: (path: string) => invoke<Json>("delete_path", { path }),
  openUrl: (url: string) => invoke<Json>("open_url", { url }),
  checkFirstLaunch: () => invoke<{ first_launch?: boolean }>("check_first_launch"),
  completeFirstLaunch: () => invoke<Json>("complete_first_launch"),

  // ---------- discord rpc ----------
  discordInit: () => invoke<Json>("init_discord_rpc"),
  discordUpdate: (
    details: string,
    largeImage: string,
    largeText: string,
    smallImage: string,
    smallText: string,
  ) =>
    invoke<Json>("update_discord_rpc", {
      details,
      largeImage,
      largeText,
      smallImage,
      smallText,
    }),
  discordStop: () => invoke<Json>("stop_discord_rpc"),
  discordStatus: () =>
    invoke<{ connected?: boolean; enabled?: boolean; client_id?: string }>("get_discord_status"),

  // ---------- AI ----------
  aiChat: (messages: AiMessage[], context: AiContext | null) =>
    invoke<{ reply?: string; error?: string }>("ai_chat", { messages, context }),
  aiContextSet: (kind: string, name: string) =>
    invoke<Json>("ai_context_set", { kind, name }),
  aiContextGet: () => invoke<{ context: AiContext | null }>("ai_context_get_app"),
  aiContextClear: () => invoke<Json>("ai_context_clear_app"),
};

/** Subscribe to a Tauri event. Returns the unlisten function. */
export function onEvent<T>(name: string, cb: (payload: T) => void): Promise<UnlistenFn> {
  return listen<T>(name, (e) => cb(e.payload));
}

/** Native open-file dialog → absolute path (or null if cancelled). */
export async function pickFile(
  label?: string,
  filters?: { name: string; extensions: string[] }[],
): Promise<string | null> {
  const { open } = await import("@tauri-apps/api/dialog");
  const r = await open({ multiple: false, title: label, filters });
  if (typeof r === "string") return r;
  if (Array.isArray(r) && typeof r[0] === "string") return r[0];
  return null;
}

/** Native save-file dialog → absolute path (or null if cancelled). */
export async function pickSave(
  defaultName?: string,
  filters?: { name: string; extensions: string[] }[],
): Promise<string | null> {
  const { save } = await import("@tauri-apps/api/dialog");
  const r = await save({ defaultPath: defaultName, filters });
  return typeof r === "string" ? r : null;
}
