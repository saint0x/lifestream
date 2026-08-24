import {
  createContext,
  useContext,
  useEffect,
  useMemo,
  useState,
  type ReactNode,
} from "react";

export interface PageCrumb {
  readonly label: string;
  readonly href?: string;
}

interface PageNavigationValue {
  readonly crumbs: ReadonlyArray<PageCrumb> | null;
  readonly setCrumbs: (crumbs: ReadonlyArray<PageCrumb> | null) => void;
}

const PageNavigationContext = createContext<PageNavigationValue | null>(null);

export function PageNavigationProvider({ children }: { readonly children: ReactNode }) {
  const [crumbs, setCrumbs] = useState<ReadonlyArray<PageCrumb> | null>(null);
  const value = useMemo(() => ({ crumbs, setCrumbs }), [crumbs]);

  return (
    <PageNavigationContext.Provider value={value}>
      {children}
    </PageNavigationContext.Provider>
  );
}

export function usePageNavigation() {
  const context = useContext(PageNavigationContext);
  if (!context) throw new Error("usePageNavigation must be used inside PageNavigationProvider");
  return context;
}

export function usePageBreadcrumbs(crumbs: ReadonlyArray<PageCrumb> | null) {
  const { setCrumbs } = usePageNavigation();
  const signature = JSON.stringify(crumbs);

  useEffect(() => {
    setCrumbs(signature === "null" ? null : JSON.parse(signature) as ReadonlyArray<PageCrumb>);
    return () => setCrumbs(null);
  }, [signature, setCrumbs]);
}
