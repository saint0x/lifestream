export async function shareCurrentPage(title: string): Promise<string> {
  const url = window.location.href;
  if (navigator.share) {
    await navigator.share({ title, url });
    return "Shared.";
  }
  await navigator.clipboard.writeText(url);
  return "Link copied.";
}
