"""Build a per-user, unsigned Windows x64 test installer with Inno Setup 6.3+."""
import argparse
import json
import os
from pathlib import Path
import shutil
import struct
import subprocess
import sys
import tempfile
from distribution import copy_asset_licenses, validate_version, verify_notices, write_checksums, write_manifest

ROOT = Path(__file__).resolve().parents[1]
TARGET = "x86_64-pc-windows-msvc"


def verify_pe(path):
    with Path(path).open("rb") as source:
        if source.read(2) != b"MZ":
            raise ValueError("Not a Windows executable")
        source.seek(60)
        offset = source.read(4)
        if len(offset) != 4:
            raise ValueError("Truncated PE header offset")
        source.seek(struct.unpack("<I", offset)[0])
        header = source.read(6)
        if header != b"PE\0\0\x64\x86":
            raise ValueError("Expected an AMD64 PE executable")


def resource_text(version):
    validate_version(version)
    numbers = ",".join(version.split(".") + ["0"])
    icon = (ROOT / "assets/flow.ico").as_posix()
    return '''#pragma code_page(65001)
1 ICON "{icon}"
1 VERSIONINFO
FILEVERSION {numbers}
PRODUCTVERSION {numbers}
FILEOS 0x40004
FILETYPE 0x1
BEGIN
 BLOCK "StringFileInfo"
 BEGIN
  BLOCK "040904b0"
  BEGIN
   VALUE "FileDescription", "FlowTorrent"
   VALUE "FileVersion", "{version}.0"
   VALUE "ProductName", "FlowTorrent"
   VALUE "ProductVersion", "{version}"
   VALUE "OriginalFilename", "flowtorrent.exe"
  END
 END
 BLOCK "VarFileInfo"
 BEGIN
  VALUE "Translation", 0x409, 1200
 END
END
'''.format(icon=icon, numbers=numbers, version=version)


def locate_tools(explicit_iscc):
    if explicit_iscc:
        candidates = [Path(explicit_iscc)]
    else:
        found = shutil.which("ISCC.exe")
        candidates = ([Path(found)] if found else []) + [
            Path(os.environ.get("ProgramFiles(x86)", r"C:\Program Files (x86)")) / "Inno Setup 6/ISCC.exe",
            Path(os.environ.get("ProgramFiles", r"C:\Program Files")) / "Inno Setup 6/ISCC.exe",
        ]
    # PATH may contain a Chocolatey shim rather than the installed compiler.
    iscc = next((path.resolve() for path in candidates
                 if path.is_file() and (path.resolve().parent / "license.txt").is_file()), None)
    if iscc is None:
        raise ValueError("Use an installed Inno Setup compiler with its adjacent license.txt; checked: "
                         + ", ".join(str(path) for path in candidates))
    rc = shutil.which("rc.exe")
    if not rc:
        kits = Path(os.environ.get("ProgramFiles(x86)", r"C:\Program Files (x86)")) / "Windows Kits/10/bin"
        candidates = list(kits.glob("10.*/x64/rc.exe"))
        candidates.sort(key=lambda path: tuple(map(int, path.parent.parent.name.split("."))))
        rc = str(candidates[-1]) if candidates else None
    if rc is None:
        raise ValueError("Install Inno Setup 6.3+ and the Windows SDK resource compiler")
    return iscc, rc


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, default=ROOT / "target/test-packages/windows-x64")
    parser.add_argument("--notices", type=Path, required=True)
    parser.add_argument("--iscc")
    args = parser.parse_args()
    if sys.platform != "win32":
        parser.error("Build this installer on Windows with the MSVC toolchain and Windows SDK")
    verify_notices(args.notices, TARGET, ROOT / "Cargo.lock")
    output = args.output.absolute()
    if output.exists() or output.is_symlink():
        parser.error("Use a fresh artifact directory; existing packages are never overwritten")
    iscc, rc = locate_tools(args.iscc)
    print("Inno Setup compiler and license directory: {}".format(iscc.parent), flush=True)
    print("Windows resource compiler: {}".format(rc), flush=True)
    cargo = os.environ.get("CARGO", "cargo")
    environment = dict(os.environ)
    # Explicit --target confines target flags to the Windows build (host proc-macros stay normal).
    environment["RUSTFLAGS"] = environment.get("RUSTFLAGS", "") + " -C target-feature=+crt-static"
    metadata = json.loads(subprocess.check_output([cargo, "metadata", "--locked", "--no-deps", "--format-version", "1"], cwd=ROOT, env=environment, text=True))
    version = next(package["version"] for package in metadata["packages"] if package["name"] == "flowtorrent")
    validate_version(version)
    output.parent.mkdir(parents=True, exist_ok=True)
    output.mkdir()
    with tempfile.TemporaryDirectory(prefix=".flowtorrent-package-", dir=output.parent) as temporary:
        temporary = Path(temporary); resource = temporary / "flowtorrent.rc"; compiled = temporary / "flowtorrent.res"
        resource.write_text(resource_text(version), encoding="utf-8")
        subprocess.run([rc, "/nologo", "/fo" + str(compiled), str(resource)], check=True)
        subprocess.run([cargo, "rustc", "--locked", "--release", "--bin", "flowtorrent", "--target", TARGET, "-j", "2", "--", "-C", "link-arg=" + str(compiled)], cwd=ROOT, env=environment, check=True)
        executable = Path(metadata["target_directory"]) / TARGET / "release/flowtorrent.exe"
        verify_pe(executable)
        subprocess.run(["powershell.exe", "-NoProfile", "-NonInteractive", "-File", str(ROOT / "scripts/verify-windows-binary.ps1"), "-Path", str(executable), "-Version", version], check=True)
        payload = temporary / "payload"; payload.mkdir()
        shutil.copy2(executable, payload / "flowtorrent.exe")
        shutil.copy2(ROOT / "assets/flow.ico", payload / "flow.ico")
        copy_asset_licenses(ROOT, payload)
        shutil.copy2(ROOT / "LICENSE", payload / "LICENSE")
        shutil.copy2(iscc.parent / "license.txt", payload / "INNO_SETUP_LICENSE.txt")
        shutil.copytree(args.notices, payload / "ThirdParty", symlinks=True)
        (payload / "LOCAL_BUILD.txt").write_text("FlowTorrent {} local test build. Unsigned; not the public 1.0 release.\n\nQuit the previous version before updating. Installation and removal preserve\nyour session and downloads. File registration adds an Open With candidate;\nyou choose the default app in Windows. Release/device checks remain pending.\n".format(version), encoding="utf-8")
        (payload / "LOCAL_BUILD.ru.txt").write_text(
            "FlowTorrent {} — локальная тестовая сборка без цифровой подписи.\n"
            "Это не публичный выпуск 1.0.\n\n"
            "Перед обновлением завершите предыдущую версию приложения. Установка\n"
            "и удаление сохраняют сеанс и загруженные файлы. FlowTorrent добавляется\n"
            "в список «Открыть с помощью»; приложение по умолчанию вы выбираете\n"
            "в Windows. Проверки выпуска и проверки на устройствах ещё не завершены.\n".format(version),
            encoding="utf-8-sig",
        )
        subprocess.run([str(iscc), "/DAppVersion=" + version, "/DPayloadDir=" + str(payload), "/DArtifactDir=" + str(output), str(ROOT / "packaging/windows/flowtorrent.iss")], check=True)
    installer = output / "FlowTorrent-{}-local-test-windows-x64-setup.exe".format(version)
    subprocess.run(["powershell.exe", "-NoProfile", "-NonInteractive", "-File", str(ROOT / "scripts/verify-windows-binary.ps1"), "-Path", str(installer), "-Version", version], check=True)
    write_checksums([installer], Path(str(installer) + ".sha256"))
    revision = subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip()
    if subprocess.check_output(["git", "status", "--porcelain"], cwd=ROOT, text=True).strip():
        revision += "+dirty"
    write_manifest([installer], Path(str(installer) + ".json"), version, TARGET, ROOT / "Cargo.lock", revision)
    print("Unsigned local test installer: {}".format(installer))


if __name__ == "__main__":
    main()
