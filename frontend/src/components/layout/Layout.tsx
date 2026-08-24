import type { ReactNode } from "react";
import { useAppStore } from "@/lib/store";
import { Header } from "./Header";
import { Sidebar } from "./Sidebar";
import "./Layout.css";

interface LayoutProps {
  readonly children: ReactNode;
  readonly chromeless?: boolean; // true = no sidebar padding (immersive watch pages)
}

export function Layout({ children, chromeless = false }: LayoutProps) {
  const actionError = useAppStore((state) => state.actionError);
  const clearActionError = useAppStore((state) => state.clearActionError);

  return (
    <div className={`ls-layout ${chromeless ? "ls-layout--chromeless" : ""}`}>
      <Sidebar />
      <div className="ls-layout__main">
        <Header />
        {actionError ? (
          <div className="ls-layout__notice" role="status">
            <span>{actionError}</span>
            <button type="button" onClick={clearActionError}>
              Dismiss
            </button>
          </div>
        ) : null}
        <main className="ls-layout__content">{children}</main>
      </div>
    </div>
  );
}
