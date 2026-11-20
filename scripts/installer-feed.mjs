// Installer builds must have a usable, public feed. Local qualification builds
// compile their own absolute FileSource directly and never use this packager.
export function installerUpdateFeed(config, override) {
  const feed = override?.trim() || config.updateFeed;
  let url;
  try {
    url = new URL(feed);
  } catch {
    throw new Error("An HTTPS installer update feed is required.");
  }
  if (
    url.protocol !== "https:" ||
    !url.hostname ||
    url.username ||
    url.password ||
    url.href.includes("?") ||
    url.href.includes("#")
  )
    throw new Error(
      "Installer update feeds must be credential-free HTTPS URLs.",
    );
  if (
    url.hostname === "github.com" &&
    (url.port || !/^\/[\w-]+\/[\w.-]+\/?$/.test(url.pathname))
  )
    throw new Error("GitHub update feeds must name one owner/repository.");
  return url.href.replace(/\/$/, "");
}
