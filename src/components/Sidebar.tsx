import { useApp } from "../stores/app";
import type { Page } from "../lib/types";
import { useEffect, useState } from "react";
import { fbIsAdmin } from "../lib/firebase";
import {
  Bot,
  Gamepad2,
  Handshake,
  LayoutGrid,
  MessagesSquare,
  Puzzle,
  ScrollText,
  Server,
  Settings,
  Shield,
  User,
} from "lucide-react";

// Core navigation only — Modpacks, Templates, Boost, Play, Themes, News and
// Config routes were removed from the app entirely.
// FastClient order: PLAY · PROFILES · ACCOUNTS · PARTNERS · … · SETTINGS.
export const NAV_ITEMS: { id: Page; label: string; icon: typeof Gamepad2 }[] = [
  { id: "home", label: "Play", icon: Gamepad2 },
  { id: "instances", label: "Profiles", icon: LayoutGrid },
  { id: "accounts", label: "Accounts", icon: User },
  { id: "partnerships", label: "Partners", icon: Handshake },
  { id: "mods", label: "Mods", icon: Puzzle },
  { id: "servermgr", label: "Host", icon: Server },
  { id: "discord", label: "Chat", icon: MessagesSquare },
  { id: "ai", label: "AI", icon: Bot },
  { id: "logs", label: "Logs", icon: ScrollText },
  { id: "settings", label: "Settings", icon: Settings },
];

export default function Sidebar() {
  const page = useApp((s) => s.page);
  const setPage = useApp((s) => s.setPage);
  const info = useApp((s) => s.info);
  const gameRunning = useApp((s) => s.gameRunning);
  const [isAdmin, setIsAdmin] = useState(false);

  // Admin badge appears only for a signed-in admin session.
  useEffect(() => {
    let alive = true;
    const sync = () => void fbIsAdmin().then((a) => alive && setIsAdmin(a));
    sync();
    window.addEventListener("focus", sync);
    const t = window.setInterval(sync, 30_000);
    return () => {
      alive = false;
      window.removeEventListener("focus", sync);
      window.clearInterval(t);
    };
  }, []);

  const items = [...NAV_ITEMS, ...(isAdmin ? [{ id: "admin" as Page, label: "Admin", icon: Shield }] : [])];

  return (
    <aside className="relative z-20 flex h-full w-24 flex-none flex-col border-r border-white/5 bg-[#0d0e12]/90 backdrop-blur-md transition-all duration-200">
      {/* nav — FastClient card-style items (icon over uppercase label) */}
      <nav className="flex min-h-0 flex-1 flex-col gap-3 overflow-y-auto px-2 pb-2 pt-3.5 [scrollbar-width:none] [&::-webkit-scrollbar]:hidden">
        {items.map((item) => {
          const active = page === item.id;
          const Icon = item.icon;
          return (
            <button
              key={item.id}
              type="button"
              title={item.label}
              aria-current={active ? "page" : undefined}
              onClick={() => setPage(item.id)}
              className={[
                "group relative flex w-full cursor-pointer flex-col items-center gap-1.5 rounded-2xl border px-1 py-3.5 font-[family-name:inherit] transition-all duration-200",
                active
                  ? "border-white/15 bg-white/10 text-white shadow-[inset_0_1px_0_rgba(255,255,255,0.06)]"
                  : "border-transparent bg-transparent text-zinc-400 hover:bg-white/5 hover:text-zinc-200",
              ].join(" ")}
            >
              <Icon
                aria-hidden
                strokeWidth={2}
                className="h-6 w-6 transition-transform duration-200 group-hover:scale-105"
              />
              <span className="text-[10px] font-bold uppercase leading-none tracking-wider">
                {item.label}
              </span>
            </button>
          );
        })}
      </nav>

      {/* foot — version string (FastClient `v4.0.17` slot) */}
      <div className="flex flex-col items-center gap-1 border-t border-white/5 px-2 py-2">
        <span className="text-[9px] font-bold uppercase tracking-wider text-zinc-500">
          {info?.version ? `v${info.version}` : "Ready"}
        </span>
        <span
          className={`h-1.5 w-1.5 rounded-full ${gameRunning ? "bg-white" : "bg-zinc-600"}`}
        />
      </div>
    </aside>
  );
}
