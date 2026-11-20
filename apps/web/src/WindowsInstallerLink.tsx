export function WindowsInstallerLink({
  primary = false,
}: {
  primary?: boolean;
}) {
  return (
    <a
      className={primary ? "button" : "installer-link"}
      href="https://github.com/Felipebmaitan/podetelar/releases/latest"
      referrerPolicy="no-referrer"
    >
      Get the Windows installer
    </a>
  );
}
