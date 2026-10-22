$ErrorActionPreference = 'Stop'
$installers = @(Get-ChildItem target/release/bundle/nsis -Filter *.exe)
if ($installers.Count -ne 1) { throw "Expected exactly one NSIS installer" }
$installRoot = Join-Path $env:RUNNER_TEMP 'WatchTogether-InstallSmoke'
$process = Start-Process -FilePath $installers[0].FullName -ArgumentList @('/S', "/D=$installRoot") -Wait -PassThru
if ($process.ExitCode -ne 0) { throw "Installer failed: $($process.ExitCode)" }
$executables = @(Get-ChildItem $installRoot -Filter *.exe -Recurse | Where-Object { $_.Name -notmatch 'uninstall|webview' })
if ($executables.Count -ne 1) {
    Get-ChildItem $installRoot -Recurse
    throw 'Installed app executable missing or ambiguous'
}
$dll = Join-Path $executables[0].DirectoryName 'mpv-2.dll'
if (-not (Test-Path $dll)) { throw "Installed media runtime missing: $dll" }
$verifyRuntime = @'
import ctypes, os, pathlib, sys
path = pathlib.Path(sys.argv[1])
with os.add_dll_directory(str(path.parent)):
    api = ctypes.WinDLL(str(path))
    api.mpv_create.restype = ctypes.c_void_p
    api.mpv_destroy.argtypes = [ctypes.c_void_p]
    handle = api.mpv_create()
    assert handle, 'Installed libmpv could not create a player'
    api.mpv_destroy(handle)
print('PASS: installer installs the executable and a loadable libmpv runtime')
'@
python -c $verifyRuntime $dll
if ($LASTEXITCODE -ne 0) { throw 'Installed libmpv runtime verification failed' }
