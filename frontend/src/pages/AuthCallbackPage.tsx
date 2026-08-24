import { useEffect } from "react";
import { useNavigate, useSearchParams } from "react-router-dom";
import { setAccessToken } from "@/lib/api";
import { useAppStore } from "@/lib/store";

export function AuthCallbackPage() {
  const [params] = useSearchParams();
  const navigate = useNavigate();
  const hydrate = useAppStore((s) => s.hydrate);

  useEffect(() => {
    const token = params.get("accessToken")?.trim();
    if (token) {
      setAccessToken(token);
      void hydrate().finally(() => navigate("/", { replace: true }));
      return;
    }
    navigate("/", { replace: true });
  }, [hydrate, navigate, params]);

  return <div className="ls-live-watch__route-state mono">Finishing sign in...</div>;
}
