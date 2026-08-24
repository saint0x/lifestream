import { useEffect } from "react";
import { RouterProvider } from "react-router-dom";
import { useAppStore } from "./lib/store";
import { router } from "./router";

export function App() {
  const hydrationStatus = useAppStore((state) => state.hydrationStatus);
  const hydrationMessage = useAppStore((state) => state.hydrationMessage);
  const hydrate = useAppStore((state) => state.hydrate);

  useEffect(() => {
    if (hydrationStatus === "idle") {
      void hydrate();
    }
  }, [hydrate, hydrationStatus]);

  if (hydrationStatus !== "ready") {
    const isLoading = hydrationStatus === "idle" || hydrationStatus === "loading";
    return (
      <div className="grain">
        <main className="ls-boot">
          <div className="ls-boot__kicker mono">vanta / session</div>
          <h1 className="ls-boot__title">{isLoading ? "Opening VANTA" : "Unable to open"}</h1>
          <p className="ls-boot__sub">
            {isLoading
              ? "Loading your shell and creator workspace."
              : hydrationMessage ?? "VANTA could not start."}
          </p>
          {!isLoading ? (
            <button className="ls-boot__button" type="button" onClick={() => void hydrate()}>
              Retry
            </button>
          ) : null}
        </main>
      </div>
    );
  }

  return (
    <div className="grain">
      <RouterProvider router={router} />
    </div>
  );
}
