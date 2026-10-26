import { useSyncExternalStore } from "react";

const listeners = new Set<() => void>();
let currentTime = Date.now();
let timer: ReturnType<typeof setInterval> | undefined;

const clock = {
  subscribe(listener: () => void) {
    listeners.add(listener);
    if (listeners.size === 1) {
      currentTime = Date.now();
      timer = setInterval(() => {
        currentTime = Date.now();
        for (const notify of listeners) notify();
      }, 60_000);
    }

    return () => {
      listeners.delete(listener);
      if (listeners.size === 0) {
        clearInterval(timer);
        timer = undefined;
      }
    };
  },
  getSnapshot: () => currentTime,
};

export function useMinuteClock() {
  return useSyncExternalStore(clock.subscribe, clock.getSnapshot);
}
