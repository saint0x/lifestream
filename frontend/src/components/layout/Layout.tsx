import type { ReactNode } from "react";
import { Header } from "./Header";
import { Sidebar } from "./Sidebar";
import "./Layout.css";

interface LayoutProps {
  readonly children: ReactNode;
  readonly chromeless?: boolean; // true = no sidebar padding (immersive watch pages)
}

export function Layout({ children, chromeless = false }: LayoutProps) {
  return (
    <div className={`ls-layout ${chromeless ? "ls-layout--chromeless" : ""}`}>
      <Sidebar />
      <div className="ls-layout__main">
        <Header />
        <main className="ls-layout__content">{children}</main>
      </div>
    </div>
  );
}
