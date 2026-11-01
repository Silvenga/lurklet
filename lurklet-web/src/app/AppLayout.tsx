import { FaGithub } from "react-icons/fa";
import { Link, Outlet, ScrollRestoration } from "react-router";
import { version } from "../../package.json";

export function AppLayout() {
  return (
    <div className="mx-auto w-full max-w-5xl px-4 sm:px-6">
      <ScrollRestoration />
      <header className="flex h-14 items-center justify-between">
        <Link className="text-2xl font-light tracking-tight" to="/">
          Lurklet
        </Link>
        <a
          className="p-2"
          href="https://github.com/Silvenga/lurklet"
          target="_blank"
          rel="noopener noreferrer"
          aria-label="GitHub repository"
        >
          <FaGithub aria-hidden="true" className="size-5" />
        </a>
      </header>
      <main className="pt-2 pb-4 sm:py-8">
        <Outlet />
      </main>
      <footer className="text-muted-foreground py-4 text-center text-xs select-text">
        Lurklet v{version}
      </footer>
    </div>
  );
}
