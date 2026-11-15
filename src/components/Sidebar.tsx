import { useApp } from "../stores/app";
import type { Page } from "../lib/types";
import { useEffect, useState } from "react";
import { fbIsAdmin } from "../lib/firebase";
import {
  Bot,
  Handshake,
  Home,
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
export const NAV_ITEMS: { id: Page; label: string; icon: typeof Home }[] = [
  { id: "home", label: "Home", icon: Home },
  { id: "instances", label: "Profiles", icon: LayoutGrid },
  { id: "mods", label: "Mods", icon: Puzzle },
  { id: "servermgr", label: "Host", icon: Server },
  { id: "partnerships", label: "Partners", icon: Handshake },
  { id: "discord", label: "Chat", icon: MessagesSquare },
  { id: "accounts", label: "Accounts", icon: User },
  { id: "ai", label: "AI", icon: Bot },
  { id: "logs", label: "Logs", icon: ScrollText },
  { id: "settings", label: "Settings", icon: Settings },
];

export default function Sidebar() {
  const page = useApp((s) => s.page);
  const setPage = useApp((s) => s.setPage);
  const info = useApp((s) => s.info);
  const gameRunning = useApp((s) => s.gameRunning);
  const config = useApp((s) => s.config);
  const [isAdmin, setIsAdmin] = useState(false);

  const brandName = (config.name || "Isolate").replace(/ client$/i, "");
  const brandLogo = config.logo || "/logo.jpeg";

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

  return (
    <aside className="relative z-20 flex h-full w-24 flex-none flex-col border-r border-white/5 bg-[#0d0e12]/90 backdrop-blur-md transition-all duration-200">
      {/* brand */}
      <div className="flex flex-col items-center gap-1 px-2 pb-1.5 pt-2.5">
        <img
          src={brandLogo}
          alt={brandName}
          className="block h-10 w-10 rounded-xl border border-white/15 object-cover"
        />
        <span className="text-[10px] font-bold uppercase leading-none tracking-wider text-white">
          {brandName}
        </span>
      </div>

      {/* nav — larger card-style items */}
      <nav className="flex min-h-0 flex-1 flex-col gap-3 overflow-y-auto px-2 py-1.5 [scrollbar-width:none] [&::-webkit-scrollbar]:hidden">
        {[...(isAdmin ? [{ id: "admin" as Page, label: "Admin", icon: Shield }] : []), ...NAV_ITEMS].map(
          (item) => {
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
                  "group relative flex w-full cursor-pointer flex-col items-center gap-1 rounded-2xl border px-1 py-3.5 font-[family-name:inherit] transition-all duration-200",
                  active
                    ? "border-white/10 bg-white/10 text-white shadow-[inset_0_1px_0_rgba(255,255,255,0.06)]"
                    : "border-transparent bg-transparent text-zinc-400 hover:bg-white/5 hover:text-zinc-200",
                ].join(" ")}
              >
                {active && (
                  <span
                    aria-hidden
                    className="absolute left-0 top-1/2 h-6 w-1 -translate-y-1/2 rounded-r-full bg-blue-500"
                  />
                )}
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
          },
        )}
      </nav>

      {/* foot status */}
      <div className="flex flex-col items-center gap-1 border-t border-white/5 px-2 py-2">
        <span className="text-[9px] font-bold uppercase tracking-wider text-zinc-500">
          {gameRunning ? "Live" : info?.version ? `v${info.version}` : "Ready"}
        </span>
        <span
          className={`h-1.5 w-1.5 rounded-full ${gameRunning ? "bg-emerald-400" : "bg-zinc-600"}`}
        />
      </div>
    </aside>
  );
}
