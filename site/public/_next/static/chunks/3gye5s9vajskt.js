/**
 * Stub for the Tebex checkout chunk.
 *
 * The export shipped every chunk *except* this one, so the store page could
 * never hydrate: Turbopack tried to load it, got a 404, and the module
 * registry failed — which took the whole store bundle down with it. The page
 * showed the server-rendered markup and then sat on the "Loading store…"
 * skeleton forever, because the catalog fetch lives in a client effect.
 *
 * The real chunk loads the third-party Tebex widget. That widget is not part
 * of this repository, and the store is flagged as unavailable anyway
 * (STORE_AVAILABLE === false), so this stub provides the same interface with
 * the checkout disabled and an honest message.
 *
 * Interface (as used by the store bundle):
 *   useTebexCheckout(onComplete, customerId, checkoutIdent, orderId, status)
 *     -> { ready, opening, awaitingConfirmation, error, notice, displayName,
 *          launch }
 */
(globalThis.TURBOPACK || (globalThis.TURBOPACK = [])).push([
  "object" == typeof document ? document.currentScript : void 0,
  10688,
  (e) => {
    "use strict";
    var t = e.i(77217);
    e.s(["default", () => t.useTebexCheckout]);
  },
  77217,
  (e) => {
    "use strict";
    var t = e.i(71645);
    function n(e, s, r, i, o) {
      let [a, c] = (0, t.useState)(!1),
        [u, d] = (0, t.useState)(!1),
        [m, g] = (0, t.useState)(null),
        [b, y] = (0, t.useState)(null);
      return {
        ready: !1,
        opening: a,
        awaitingConfirmation: u,
        error: m,
        notice: b,
        displayName: "Tebex",
        launch: async () => {
          g("Checkout is not available yet.");
          y("The store is coming soon — your basket is saved.");
        },
      };
    }
    e.s(["useTebexCheckout", 0, n]);
  },
]);
