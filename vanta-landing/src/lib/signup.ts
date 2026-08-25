import type { Audience } from "@/content/pages";

export type SignupInput = {
  readonly kind: "creator" | "buyer" | "general";
  readonly audience: Audience;
  readonly name: string;
  readonly email: string;
  readonly company?: string;
  readonly website?: string;
  readonly budget?: string;
  readonly message?: string;
  readonly sourcePath: string;
};

export type SignupResult = {
  readonly id: string;
  readonly kind: string;
  readonly status: string;
};

const PRODUCTION_API_BASE_URL = "https://api-production-4becb.up.railway.app";

const apiBase = (import.meta.env.VITE_VANTA_API_BASE_URL ?? PRODUCTION_API_BASE_URL).replace(/\/$/, "");

export async function submitSignup(input: SignupInput): Promise<SignupResult> {
  const response = await fetch(`${apiBase}/api/v1/landing/signups`, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify(input),
  });

  if (!response.ok) {
    const body = await response.json().catch(() => null);
    const message =
      typeof body?.error === "string" ? body.error : "Signup could not be submitted. Please try again.";
    throw new Error(message);
  }

  return response.json() as Promise<SignupResult>;
}
