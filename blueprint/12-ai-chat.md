# Xryloth — AI Assistant (Groq → Gemini agent with 216 tools)

Source: `src-tauri/src/lib.rs` §AI ASSISTANT, lines **8095-10435**. Line refs exact.

---

## 1. Architecture at a glance

```
frontend ChatPage / XrylothAIPage
   │  invoke("ai_chat", {messages, context})
   ▼
ai_chat (10301)  ── builds system prompt (ai_system_prompt 8166)
   │              ── selects a subset of tools (ai_select_tools 8892)
   │              ── attempt chain: user model → GROQ_MODELS[4] → GEMINI_MODELS[3]
   │                    │ streaming SSE ── emit "ai-chunk" {text}
   │                    │ rate limit    ── emit "ai-status", wait, retry
   │                    ▼
   │              AiResponse {content, tool_calls}
   │                    │ no tool_calls → emit "ai-done" {reply} → return
   │                    ▼
   │              for each tool_call:
   │                 emit "ai-tool" {tool, status:"start"}
   │                 ai_execute_tool (9277 → spawn_blocking → ai_execute_tool_blocking 9287)
   │                 emit "ai-tool" {tool, status:"done|error", summary}
   │                 push {role:"tool", content} into convo
   └── loop up to AI_MAX_TOOL_ROUNDS = 12 (8130)
```

**Provider constants** (8096-8130):

| const | value |
|---|---|
| `GROQ_API` | `https://api.groq.com/openai/v1/chat/completions` |
| `GROQ_MODELS` | `["openai/gpt-oss-120b", "openai/gpt-oss-20b", "qwen/qwen3.8-27b", "llama-3.1-8b-instant"]` |
| `GEMINI_API` | `https://generativelanguage.googleapis.com/v1beta/models` |
| `GEMINI_MODELS` | `["gemini-3.5-flash-lite", "gemini-2.5-flash", "gemini-2.0-flash"]` |
| `AI_MAX_TOOL_ROUNDS` | `12` |

Comments note the original primary (`llama-3.2-11b-vision-preview`) was **decommissioned by Groq**, so the chain now leads with the user's requested model, then the tool-capable Groq list, then Gemini.

---

## 2. API key resolution — `ai_resolve_key` (8109-8141)

Order (first hit wins), for Groq (`ai_groq_key` / `XRYLOTH_GROQ_KEY` / `groq_api_key`) and Gemini (`ai_gemini_key` / `XRYLOTH_GEMINI_KEY` / `gemini_api_key`):

1. **Launcher settings** — `settings.ai_groq_key` / `settings.ai_gemini_key` (set on the Xryloth AI page)
2. **`~/.xryloth/ai_keys.json`** — `{"groq_api_key":…, "gemini_api_key":…}`
3. **Environment variables** `XRYLOTH_GROQ_KEY` / `XRYLOTH_GEMINI_KEY`
4. **Compile-time embedded defaults** — `embedded_keys::DEFAULT_GROQ_KEY` / `DEFAULT_GEMINI_KEY`, baked by `build.rs` from `src-tauri/secrets.json` (or env) into `OUT_DIR/embedded_keys.rs` (see `11-build-release.md` §secrets).

If both resolve empty, `ai_chat` emits `ai-chunk` with "⚠️ No AI API keys configured…" and returns early (10331).

> 🔑 `src-tauri/secrets.json` contains exactly `groq_api_key` and `gemini_api_key` (values never read into any doc here).

---

## 3. System prompt — `ai_system_prompt` (8166-8206)

Built fresh each call from `app.ai_context` + filesystem facts:

- Identity: "Xryloth AI, the built-in assistant of the Xryloth Minecraft launcher. You control the launcher, its game profiles and its self-hosted Minecraft servers using tools."
- Facts line: launcher `APP_DISPLAY_VERSION`, OS, data folder, servers folder, instances folder.
- **ACTIVE WORKSPACE** block when `ai_context` is set: `kind ∈ {server, instance}` + name; for a server it also injects `type`, `minecraft` version, `port`, `ram` MB, folder (from `server.json`); for a profile: minecraft/loader/loader_version/folder.
- Instruction: *"Every target-specific action applies to it unless the user names a different server/profile."*
- Instruction to prefer `ai_guide_*` tools for how-to questions and to build answers **only** from returned real facts — never invent paths/versions/URLs (8199).

---

## 4. Tool system

### 4.1 Definition helpers (8411-8431)

```rust
ai_tool(name, desc, &[("param","type","desc", required), …])   // full JSON-schema fn
ai_simple(name, desc)                                          // no params
ai_target(name, desc)                                          // + optional {server} param
```

Emitted as OpenAI-style `{"type":"function","function":{name, description, parameters}}`.

### 4.2 Tool count — **216 total** (`ai_tool_defs` 8433-8838)

| group | count | how generated |
|---|---|---|
| Static tools (instances, app, mods, files, backups, plugins, …) | **102** | literal `t.push(ai_tool(…))` |
| `server_console_*` (say/op/kick/ban/tps/stop/…) | **26** | `console_cmds` array loop (8474-8512) |
| `server_set_<property>` (motd/port/ram/gamemode/…) | **28** | `setters` array loop, `format!("server_set_{}", key)` (8514-8548) |
| `app_navigate_<page>` (play/instances/mods/…/configeditor) | **14** | `pages` array loop (8744-8752) |
| `ai_guide_*` (how-to fact fetchers) | **46** | `guides` array loop (8784-8832) |
| **total** | **216** | (code comment says "200+ real tools") |

**Static groups** (the 102): `server_*` (list/create/delete/start/stop/kill/restart/stats/status/clone/reinstall/eula/files/backups/worlds/ops/bans/whitelist/players/plugins/viaversion/port/java/icon/jar_info/…), `instance_*` (list/create/delete/launch/mods/settings/versions/folder/playtime), `app_*` (settings get/set/toggle/ram/java/theme, system_info, get_paths, live_version, news, navigate, open_folder/mod_browser/create_instance, refresh, check_updates), `ai_context_*`.

**Guide tools** return *real data* via `ai_guide_facts` (9081-9274) — e.g. `ai_guide_create_server` returns the actual server types + downloadable versions + default port 25565 + an example call; `ai_guide_install_plugin` returns real Modrinth matches for the workspace server's MC version and the real `plugins/` path; `ai_guide_java_install` returns actually-detected Javas.

### 4.3 Tool selection — `ai_select_tools` (8892-8936)

Groq's free tier is ~8k tokens/min, so the full 216-tool payload (~10k tokens) is never sent. Instead:

- Keyword buckets map user text → tool prefixes, e.g. `["guide","how do i","how to","tutorial","steps","explain"] → guide_`, plus buckets for server/instance/settings/console actions (8858-8871).
- Score each tool: base **5**, `+100` for a wanted prefix, `-25` for a conflicting prefix, `+30` for `ai_*`/`app_*`, `+60` when the workspace is a server/instance, `+40` for **core tools**.
- **Core tools** (`is_core_ai_tool` 8872): `server_list`, `instance_list`, `app_settings_get`, `ai_context_*`, `app_navigate*`, `app_system_info`, `app_get_paths`, `app_live_version`, `app_news`, `server_types_list`, `server_java_list`, `server_versions_list`, `instance_versions_list`, `server_status`, `server_stats`, `server_console_command`, `server_plugins_list`, `instance_mods_list`.
- Sort by score desc (ties by index), `take(limit)`.

### 4.4 Execution — `ai_execute_tool` (9277) / `ai_execute_tool_blocking` (9287)

- Wrapped in `tokio::task::spawn_blocking` (9278-9284) because the body does blocking reqwest/FS work.
- Helpers: `s(k)` string, `n(k)` u64, `b(k)` bool args; `ctx_server` / `ctx_instance` from `ai_context`; `target_server()` = explicit `server` arg → else active workspace → sanitized (9295-9300).
- **`emit_action`** closure pushes `ai-action` events the UI reacts to: `{"type":"refresh-servers"}` / `refresh-instances` / `refresh-mods` etc. (`note_refresh(kind)`), so the UI re-reads state after a tool mutates it.
- Dispatch styles:
  - big `match tool { "server_create" => …, … }`
  - **prefix handlers**: `server_console_*` (9615 — maps to concrete console commands via `server_mgr_command`), `server_set_*` (9659 — patches `server.json` and, for gameplay keys, mirrors into `server.properties`), `app_navigate_*` (9336 — returns `{"type":"navigate","page":…}` for the UI).
- Every successful mutation returns a `summary` string, which is surfaced in the `ai-tool` done event so the chat can show "✅ server_start → Started 'MyServer'".

---

## 5. Provider calls

### 5.1 Groq — `ai_groq_chat` (8938)

OpenAI-compatible `POST /openai/v1/chat/completions` with `stream: true`; parses SSE `data:` lines, invoking the `on_chunk` callback per delta (8969-9010). Returns `AiResponse {content, tool_calls, streamed}`.

### 5.2 Gemini — `ai_gemini_chat` (9011)

`POST {GEMINI_API}/{model}:generateContent?key={key}`; converts the OpenAI-shaped convo into Gemini `contents` (assistant `functionCall` parts, `functionResponse` parts — 9027-9044); returns matching `AiResponse`.

### 5.3 Attempt loop (in `ai_chat`, 10344-10382)

```
attempts = [(Groq, user_model?)] + GROQ_MODELS + GEMINI_MODELS
for each (provider, model):
    ok → break
    rate_limited → ai_rate_limit_wait(err) parses "try again in Ns" (8884-8890),
                   emit "ai-status" {"text":"Rate limited — retrying in Ns…"}, sleep ≤30 s, retry
    other error → emit "ai-status" {"text":"<model> failed, trying the next model…"}
all failed → emit ai-chunk + ai-done {"reply":"All AI providers failed…"} and return {reply, error}
```

### 5.4 Agent rounds (10336-10416)

Each round: call provider → if `tool_calls` empty → `ai-done` and return; else push assistant msg with `tool_calls`, execute each (emitting `ai-tool start` → `ai-tool done|error` with `summary`), push `{role:"tool", tool_call_id, name, content}` and loop. At `rounds > 12` it appends *"I've reached the maximum number of steps for this request…"* (10341-10342).

---

## 6. Events contract (frontend listens for these)

| event | payload | emitted at |
|---|---|---|
| `ai-chunk` | `{text}` — streamed token / status text | 10331, 10361-10362, 10388, 10394 |
| `ai-status` | `{text}` — "Rate limited…", "model failed…" | 10372, 10377 |
| `ai-tool` | `{tool, status:"start"}` then `{tool, status:"done"\|"error", summary}` | 10404, 10410 |
| `ai-action` | `{type:"refresh-servers"\|"refresh-instances"\|…}` / `{type:"navigate", page}` | via `emit_action` (9301) |
| `ai-done` | `{reply}` — final answer | 10342, 10389, 10397 |

Return value of `ai_chat`: `{"reply": …}` (+ `error` on total failure) — `Result<Value, String>`.

---

## 7. Workspace context commands

| command | line | behavior |
|---|---|---|
| `ai_context_set(kind, name)` | 10419 | Stores `{kind:"server"|"instance", name}` in `AppState.ai_context`; injected into system prompt + default target of `server_*`/`instance_*` tools. |
| `ai_context_get_app` | 10425 | Returns it. |
| `ai_context_clear_app` | 10430 | Clears it. |

The frontend also exposes `ai_context_set_server` / `ai_context_set_instance` / `ai_context_get` / `ai_context_clear` as **tools for the model** (defined 8433+, handled in the match) so the assistant can switch its own workspace.

---

## 8. Image disk cache (AI-adjacent, 8206-8410)

Used so remote images (avatars, news images, Modrinth art) are fetched once:

- `image_cache_dir()` = `~/.xryloth/image_cache/`; key = `DefaultHasher` of the URL (8221-8227) → `(file, meta.json)` pair.
- `image_cache_get(url)` (8330): rejects non-http(s) URLs; **hit** → `{success, cached:true, stale, path}`; **stale** (meta present) → background conditional GET (`If-None-Match` / `If-Modified-Since`, 8207-8209) and re-emits an `image-cache` event `{url, path}` when refreshed; **miss** → download now, `{success, cached:false, path}`. Binary sanity check `image_bytes_look_like_image` (8258) prevents caching HTML error pages.
- `image_cache_warm(urls[])` (8371): queues background downloads, `{success, queued:n}`, emits `image-cache` per finished URL.
- Frontend reads the returned local path through Tauri's **asset protocol** (whose runtime scope is widened to `~/.xryloth` at startup — see `01-architecture.md` §3.1).

---

## 9. Gotchas

1. **Keys are compiled into the binary by default** (build.rs + secrets.json) — every shipped launcher carries working Groq/Gemini keys. Anyone who decompiles the binary gets them; rotate regularly and prefer per-user keys.
2. **No refresh of conversation size limits** — the whole `messages` array from the UI is replayed every call; long chats will blow the Groq free-tier window (mitigated only by tool subset shrinking, not by message trimming).
3. **`AI_MAX_TOOL_ROUNDS = 12`** caps deep tasks; there is no summarization/continuation.
4. **Tool selection is heuristic keyword scoring** — ambiguous phrasing can drop the needed tool (`-25` penalty for conflicting prefixes).
5. **`ai_execute_tool_blocking` runs arbitrary shell/FS** via the `run_system_command` / `read_file_content` / `write_file_content` / `delete_path` commands — the model has full machine access by design.
6. **Gemini conversion loses streaming** for function-call turns (`generateContent`, not `streamGenerateContent`) — replies with tools appear only at the end.
7. Rate-limit handling trusts the model of `try again in Ns` string; non-Groq 429 formats just advance to the next model.
8. `ai_guide_*` tools take an optional free-text `query` but most of their payload is workspace-derived — without `ai_context_set` they answer for "no server selected".
