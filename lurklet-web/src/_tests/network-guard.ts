import process from "node:process";
import { Interceptor } from "@mswjs/interceptors";
import { SocketInterceptor } from "@mswjs/interceptors/net";

type NetworkGuard = {
  phase: "idle" | "mocking" | "closed";
  violations: string[];
};

const key = Symbol.for("lurklet.test.network-guard");
const existing = Reflect.get(globalThis, key) as NetworkGuard | undefined;
export const networkGuard: NetworkGuard = existing ?? { phase: "idle", violations: [] };

if (!existing) {
  const sockets = Interceptor.singleton(SocketInterceptor);
  sockets.on("connection", ({ socket, controller, connectionOptions }) => {
    const reject = () => {
      const message = `Unexpected real network connection: ${connectionOptions.host}:${connectionOptions.port} (${networkGuard.phase})`;
      const error = new Error(message);
      networkGuard.violations.push(message);
      socket.destroy(error);
      if (networkGuard.phase === "closed") {
        queueMicrotask(() => {
          throw error;
        });
      }
      return socket;
    };

    controller.passthrough = reject;
    if (networkGuard.phase !== "mocking") reject();
  });
  // Keep the guard installed after MSW closes, including between files in a reused worker.
  sockets.apply(networkGuard);
  Reflect.set(globalThis, key, networkGuard);
  process.on("exit", () => {
    if (networkGuard.violations.length > 0) {
      process.stderr.write(`${networkGuard.violations.join("\n")}\n`);
      process.exitCode = 1;
    }
  });
}
