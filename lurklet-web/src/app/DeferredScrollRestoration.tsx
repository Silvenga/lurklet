import { useLayoutEffect, useSyncExternalStore } from "react";
import type { DataRouter } from "react-router";

export function DeferredScrollRestoration({ router }: { router: DataRouter }) {
  const { location, navigation, restoreScrollPosition } = useSyncExternalStore(
    router.subscribe,
    () => router.state,
  );

  useLayoutEffect(() => {
    if (navigation.state !== "idle" || typeof restoreScrollPosition !== "number") {
      return;
    }

    // React Query content can arrive after the router restores against a shorter loading view.
    const observer = new ResizeObserver(() => {
      window.scrollTo(0, restoreScrollPosition);
      if (window.scrollY === restoreScrollPosition) {
        stop();
      }
    });
    const events = ["pointerdown", "wheel", "touchstart", "keydown"] as const;
    function stop() {
      observer.disconnect();
      for (const event of events) {
        window.removeEventListener(event, stop, true);
      }
    }

    observer.observe(document.documentElement);
    for (const event of events) {
      window.addEventListener(event, stop, { capture: true, passive: true });
    }
    return stop;
  }, [location.key, navigation.state, restoreScrollPosition]);

  return null;
}
