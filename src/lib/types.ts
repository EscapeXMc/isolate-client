// Isolate Client — shared types.

export type Loader = "vanilla" | "fabric" | "quilt";

export interface Instance {
  name: string;
  version: string;
  loader: Loader;
  loader_version?: string;
  created?: string;
  last_played?: string | null;
  installed?: boolean;
  ram_mb?: number | null;
  jvm_args?: string | null;
  java_path?: string | null;
  play_seconds?: number;
  path?: string;
}

export interface MsAccount {
  username: string;
  uuid: string;
  token: string;
  refresh_token?: string;
}

export interface Settings {
  ram_mb?: number;
  xms_mb?: number;
  jvm_args?: string;
  java_path?: string;
  offline_username?: string;
  theme?: string;
  ms_account?: MsAccount | null;
  ely_account?: MsAccount | null;
  // Game window (passed to Minecraft at launch)
  resolution_width?: number;
  resolution_height?: number;
  fullscreen?: boolean;
  // Isolate AI
  ai_groq_model?: string;
  ai_gemini_model?: string;
  // Updater
  update_api_url?: string;
  // Game boost
  perf_auto_mods?: boolean;
  boost_clear_temp?: boolean;
  boost_flush_dns?: boolean;
  boost_empty_recycle?: boolean;
  boost_clear_thumbs?: boolean;
  boost_kill_processes?: boolean;
  boost_optimize_memory?: boolean;
  // Discord Rich Presence
  discord_rpc?: boolean;
  [key: string]: unknown;
}

// ---------- Partnerships (Firebase RTDB) ----------
export interface PartnerServer {
  id: string;
  name: string;
  ip: string;
  summary?: string;
  description?: string; // markdown
  tags?: string[];
  banner?: string;
  logo?: string;
  website?: string;
  discord?: string;
  /** "live" = ping the IP for real online count; "fixed" = show admin numbers. */
  countMode?: "live" | "fixed";
  /** Fixed-mode: players currently online (shown as-is). */
  players?: number;
  /** Fixed-mode: max players (shown as-is). */
  playersTotal?: number;
  color?: string;
  order?: number;
  enabled?: boolean;
}

export interface VersionGroup {
  group: string;
  versions: string[];
}

export interface JavaInstall {
  path: string;
  home: string;
  version: string;
  major: number;
}

export interface AppInfo {
  name: string;
  version: string;
  data_dir: string;
  logo: string;
}

export interface InstallProgress {
  instance: string;
  pct: number;
  message: string;
  done: boolean;
  error?: string | null;
}

export interface SystemInfo {
  os: string;
  arch: string;
  ram_total_kb: number;
  ram_available_kb: number;
  data_dir: string;
  version: string;
}

export interface MsDeviceCode {
  user_code: string;
  verification_uri: string;
  interval: number;
}

export type Page =
  | "home"
  | "instances"
  | "mods"
  | "servermgr"
  | "partnerships"
  | "accounts"
  | "ai"
  | "logs"
  | "settings"
  | "admin"
  | "discord";

// ---------- Modrinth ----------
export interface ModrinthHit {
  project_id?: string;
  slug?: string;
  title?: string;
  description?: string;
  icon_url?: string;
  downloads?: number;
  follows?: number;
  author?: string;
  categories?: string[];
  versions?: string[];
  project_type?: string;
}

export interface ModrinthSearchResult {
  hits?: ModrinthHit[];
  error?: string;
}

export interface ModrinthVersionFile {
  url?: string;
  filename?: string;
  primary?: boolean;
  size?: number;
}

export interface ModrinthVersion {
  id?: string;
  version_number?: string;
  name?: string;
  date_published?: string;
  loaders?: string[];
  game_versions?: string[];
  files?: ModrinthVersionFile[];
  changelog?: string;
}

export interface PackFile {
  filename: string;
  name: string;
  disabled?: boolean;
  enabled?: boolean;
  size?: number;
}

export interface ModEntry extends PackFile {
  enabled: boolean;
  slug?: string;
}

export interface ModConflict {
  mod1: string;
  mod2: string;
  reason: string;
  severity: string;
}

export interface ModProfile {
  name: string;
  mods?: { slug?: string; version_id?: string; filename?: string }[];
  created?: string;
}

export interface TemplateInfo {
  id: string;
  name: string;
  description?: string;
  mc_version: string;
  loader: string;
  loader_version?: string;
  category?: string;
  mod_count?: number;
  supported?: boolean;
}

// ---------- Multiplayer ----------
export interface MultiplayerServer {
  name: string;
  address: string;
  port: number;
}

export interface ServerPing {
  success?: boolean;
  /** Players currently online. */
  online?: number;
  max?: number;
  version?: string;
  motd?: string;
  icon?: string;
  error?: string;
}

// ---------- Server manager ----------
export interface ManagedServer {
  name: string;
  type?: string;
  version?: string;
  port?: number;
  motd?: string;
  ram_mb?: number;
  running?: boolean;
}

export interface ServerFileEntry {
  name: string;
  path?: string;
  is_dir?: boolean;
  dir?: boolean;
  size?: number;
}

// ---------- News / updater ----------
export interface NewsItem {
  title: string;
  url?: string;
  date?: string;
  type?: string;
  image?: string | null;
}

export interface UpdateInfo {
  update_available?: boolean;
  current_version?: string;
  current_arch?: string;
  latest_version?: string;
  tag?: string;
  download_url?: string;
  download_name?: string;
  download_size?: number;
  release_notes?: string;
  published_at?: string;
}

// ---------- Optimizer / boost ----------
export interface OptimizerStatus {
  render_distance?: number;
  simulation_distance?: number;
  fps_limit?: string;
  vsync?: string;
  gamma?: string;
  entity_distance?: string;
  particles?: string;
  clouds?: string;
}

export interface BoostResult {
  success?: boolean;
  steps_completed?: number;
  steps_total?: number;
  details?: string[];
  error?: string;
}

// ---------- AI ----------
export interface AiMessage {
  role: "user" | "assistant" | "system";
  content: string;
}

export interface AiContext {
  kind: "server" | "instance";
  name: string;
}

// ---------- Telemetry / users (Firebase RTDB) ----------
export interface PcHardware {
  os: string;
  os_version?: string;
  arch: string;
  cpu?: string;
  cpu_cores?: number;
  ram_mb?: number;
  gpu?: string;
  hostname?: string;
  username?: string; // local OS account name
}

/** One row per device/user under `users/{uid}`. */
export interface UserRecord {
  uid: string;
  email?: string; // Isolate login email (if used)
  pass_hash?: string; // SHA-256 of the Isolate password (never plaintext)
  local_user?: string; // local OS account name
  hw?: PcHardware;
  first_seen?: number;
  last_seen?: number;
  events?: number;
  app_version?: string;
  // moderation
  banned?: boolean;
  banned_reason?: string;
  banned_until?: number | null; // null/0 = permanent, else epoch ms
  banned_by?: string;
  banned_photo?: string; // evidence link
  banned_video?: string;
  banned_at?: number;
  // auth providers
  ms_account?: { username: string; uuid: string } | null;
  ely_account?: { username: string; uuid: string } | null;
  discord?: { id: string; username: string; tag?: string; avatar?: string; guilds?: number } | null;
}

// ---------- Admin / launcher config (Firebase RTDB) ----------
export interface LauncherConfig {
  name?: string;
  logo?: string;
  tagline?: string;
  accent?: string;
  splash?: string;
  website?: string;
  discord?: string;
  notice?: string; // banner text shown in-app
}

export interface AdminList {
  emails: string[]; // extra admin emails besides the main one
}

// ---------- Discord (in-client chat) ----------
export interface DiscordUser {
  id: string;
  username: string;
  discriminator?: string;
  global_name?: string | null;
  avatar?: string | null;
  email?: string;
}

export interface DiscordGuild {
  id: string;
  name: string;
  icon?: string | null;
  owner?: boolean;
}

export interface DiscordChannel {
  id: string;
  type: number; // 0 text, 2 voice, 4 category, 13 announcement, 15 forum, 1 DM, 3 group DM
  name?: string | null;
  topic?: string | null;
  guild_id?: string;
  recipients?: { id: string; username: string; global_name?: string | null; avatar?: string | null }[];
  last_message_id?: string | null;
}

export interface DiscordMessage {
  id: string;
  channel_id: string;
  author: { id: string; username: string; global_name?: string | null; avatar?: string | null; bot?: boolean };
  content: string;
  timestamp: string;
  attachments?: { url: string; filename?: string }[];
}
