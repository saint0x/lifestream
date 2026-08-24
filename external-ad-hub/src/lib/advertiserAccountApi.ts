import type {
  AdvertiserAccount,
  AdvertiserRole,
} from "@/domain/types";

export interface UpdateAdvertiserCompanyRequest {
  readonly name: string;
  readonly industry: string;
  readonly websiteUrl?: string;
  readonly billingName: string;
  readonly billingEmail: string;
}

export interface CreateAdvertiserInviteRequest {
  readonly email: string;
  readonly name?: string;
  readonly role: AdvertiserRole;
}

export interface UpdateAdvertiserSeatRequest {
  readonly role: AdvertiserRole;
  readonly status?: "active" | "suspended";
}

async function accountRequest<TBody>(
  path: string,
  init?: { readonly method?: "GET" | "PATCH" | "POST"; readonly body?: TBody },
): Promise<AdvertiserAccount> {
  const response = await fetch(path, {
    method: init?.method ?? "GET",
    headers: init?.body === undefined ? undefined : { "Content-Type": "application/json" },
    body: init?.body === undefined ? undefined : JSON.stringify(init.body),
  });

  if (!response.ok) {
    throw new Error(`Advertiser account request failed with ${response.status}`);
  }

  return response.json() as Promise<AdvertiserAccount>;
}

export function fetchAdvertiserAccount(): Promise<AdvertiserAccount> {
  return accountRequest("/api/v1/advertiser/me/account");
}

export function updateAdvertiserCompany(input: UpdateAdvertiserCompanyRequest): Promise<AdvertiserAccount> {
  return accountRequest("/api/v1/advertiser/me/account", { method: "PATCH", body: input });
}

export function createAdvertiserInvite(input: CreateAdvertiserInviteRequest): Promise<AdvertiserAccount> {
  return accountRequest("/api/v1/advertiser/me/invites", { method: "POST", body: input });
}

export function updateAdvertiserSeat(
  userId: string,
  input: UpdateAdvertiserSeatRequest,
): Promise<AdvertiserAccount> {
  return accountRequest(`/api/v1/advertiser/me/seats/${encodeURIComponent(userId)}`, {
    method: "PATCH",
    body: input,
  });
}
