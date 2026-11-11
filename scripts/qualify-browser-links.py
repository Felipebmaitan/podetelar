"""Windows integration qualification. Invitation is supplied as JSON on stdin, never argv.
Usage: python scripts/qualify-browser-links.py --executable <portable EXE>
Requires a running local FreedomStream API. Refuses to replace an existing handler.
"""
import argparse
import ctypes
import json
import os
from pathlib import Path
import shutil
import struct
import subprocess
import sys
import tempfile
import time
import urllib.parse
import urllib.request
import winreg

KEY = r"Software\Classes\freedomstream"

def processes():
    result = subprocess.run(
        ["powershell.exe", "-NoProfile", "-NonInteractive", "-WindowStyle", "Hidden", "-Command",
         "Get-CimInstance Win32_Process | Where-Object { $_.Name -like '*freedomstream*.exe' } | Select-Object ProcessId,ParentProcessId,ExecutablePath,CommandLine | ConvertTo-Json -Compress"],
        capture_output=True, creationflags=0x08000000, check=True,
    )
    value = json.loads(result.stdout.decode("utf-8-sig") or "[]")
    return value if isinstance(value, list) else [value]

def registered():
    with winreg.OpenKey(winreg.HKEY_CURRENT_USER, KEY + r"\shell\open\command") as key:
        command = winreg.QueryValueEx(key, "")[0]
    assert command.endswith(' --browser-link "%1"'), "Incorrect Windows activation arguments"
    return Path(command.split('"')[1])

def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--executable", required=True, type=Path)
    args = parser.parse_args()
    invitation = json.load(sys.stdin)["invitationUrl"]
    url = urllib.parse.urlsplit(invitation)
    assert url.scheme == "http" and url.hostname in ("localhost", "127.0.0.1"), "Qualification requires a local HTTP test API"
    origin = f"{url.scheme}://{url.netloc}"
    room_id = url.path.removeprefix("/r/")
    secret = urllib.parse.parse_qs(url.fragment)["invite"][0]
    try:
        winreg.OpenKey(winreg.HKEY_CURRENT_USER, KEY).Close()
    except FileNotFoundError:
        pass
    else:
        raise RuntimeError("Refusing to replace an existing browser-link registration")
    root = Path(tempfile.mkdtemp(prefix="freedomstream-browser-links-")).resolve()
    previous = os.environ.get("LOCALAPPDATA")
    appdata = root / "profile"
    appdata.mkdir()
    os.environ["LOCALAPPDATA"] = str(appdata)
    executable = root / "FreedomStream.exe"
    shutil.copyfile(args.executable.resolve(), executable)
    def run(option):
        result = subprocess.run([str(executable), option], capture_output=True, creationflags=0x08000000)
        assert result.returncode == 0, "Portable browser-link settings operation failed"
    try:
        run("--enable-browser-links")
        first = registered()
        assert first.is_file() and first.is_relative_to(appdata), "Missing managed launcher"
        # A harmless PE overlay byte simulates different release bytes while retaining
        # the exact payload and valid executable. The full EXE, not only payload, is addressed.
        data = executable.read_bytes()
        size = struct.unpack("<Q", data[-56:-48])[0]
        boundary = len(data) - 56 - size
        executable.write_bytes(data[:boundary] + b"\0" + data[boundary:])
        run("--enable-browser-links")
        current = registered()
        assert current != first and current.is_file(), "Updating links must select the new EXE"
        executable.rename(root / "Moved-FreedomStream.exe")
        executable = root / "Moved-FreedomStream.exe"
        request = urllib.request.Request(origin + f"/api/rooms/{room_id}/app-link",
            data=json.dumps({"invitation": secret}).encode(), headers={"Origin": origin, "Content-Type": "application/json"})
        with urllib.request.urlopen(request, timeout=10) as response:
            link = json.load(response)["appUrl"]
        assert secret not in link, "Permanent invitation must stay out of activation arguments"
        shell = ctypes.windll.shell32.ShellExecuteW
        shell.restype = ctypes.c_void_p
        assert shell(None, "open", link, None, None, 1) > 32, "Windows URL dispatch failed"
        deadline = time.monotonic() + 15
        while True:
            owned = [p for p in processes() if p.get("ExecutablePath") and Path(p["ExecutablePath"]).is_relative_to(appdata)]
            publishers = [p for p in owned if Path(p["ExecutablePath"]).name.lower() == "freedomstream-publisher.exe"]
            if publishers:
                assert all(secret not in (p.get("CommandLine") or "") for p in owned), "Permanent invitation leaked to process arguments"
                break
            assert time.monotonic() < deadline, "Registered URL did not start the native publisher process"
            time.sleep(0.2)
        print("PASS: per-user enable, update, moved-download dispatch and native process launch; no permanent invitation in argv.")
    finally:
        for process in processes():
            if process.get("ExecutablePath") and Path(process["ExecutablePath"]).is_relative_to(appdata):
                handle = ctypes.windll.kernel32.OpenProcess(1, False, process["ProcessId"])
                if handle:
                    ctypes.windll.kernel32.TerminateProcess(handle, 0)
                    ctypes.windll.kernel32.CloseHandle(handle)
        run("--disable-browser-links")
        try:
            winreg.OpenKey(winreg.HKEY_CURRENT_USER, KEY).Close()
        except FileNotFoundError:
            pass
        else:
            raise RuntimeError("Disable did not remove the registration")
        if previous is None:
            os.environ.pop("LOCALAPPDATA", None)
        else:
            os.environ["LOCALAPPDATA"] = previous
        assert root.parent == Path(tempfile.gettempdir()).resolve() and root.name.startswith("freedomstream-browser-links-"), "Unexpected cleanup path"
        time.sleep(0.5)
        shutil.rmtree(root)
    print("PASS: disable removed the registration and test processes/files were cleaned up.")

if __name__ == "__main__":
    main()
