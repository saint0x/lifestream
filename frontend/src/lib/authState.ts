import type { User } from "@/types";

export function isSignedInUser(user: User): boolean {
  const id = user.id.trim();
  const handle = user.handle.trim().toLowerCase();
  return Boolean(id) && Boolean(handle) && !id.startsWith("guest-") && !handle.startsWith("guest");
}
