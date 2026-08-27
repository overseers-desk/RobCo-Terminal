# Build the self-contained single-file robco-settings image for Windows: one
# .exe that runs on a host with no Tcl installed.
#
# The Windows twin of zipfs/build-selfcontained.sh, stage for stage: a static
# Tcl 9, a static Tk 9, a custom wish linking the two, and zipfs/build.tcl
# folding robco-settings's payload onto that wish. Only the toolchain
# differs. Tcl's supported Windows build is nmake against MSVC, so the two
# configure/make stages become nmake -f makefile.vc with OPTS=static,msvcrt,
# and the link step is cl.exe rather than cc.
#
# Two Windows-only choices in the link:
#   - /SUBSYSTEM:WINDOWS with /ENTRY:mainCRTStartup. A GUI program that keeps a
#     console subsystem opens a console window behind its Tk window; the entry
#     override drops the console while leaving appinit.c's plain main() intact,
#     so one source file serves all three platforms.
#   - the system libraries Tcl and Tk call into (sockets, shell, common
#     controls, the print spooler Tk's GDI code opens) are named here, because
#     a static build resolves them at our link rather than inside a DLL of its
#     own.
#
# The .exe is unsigned. SmartScreen will interpose on first run for anything
# downloaded without a code-signing certificate.
#
# Requires: Visual Studio 2019+ with the C++ toolset (the hosted windows
# runners carry it), and tar + curl (in Windows 10 1803 and later).
#
# -Embed additionally produces what the terminal's own build needs to carry
# this window inside robco-term.exe: the payload as a plain zip, and the
# include and library paths of the static Tcl and Tk this script just built,
# printed as NAME=value lines and appended to $env:GITHUB_ENV where CI set
# one. The standalone image is still built and still the proof that the
# payload runs; without the switch nothing about this script changes.
#
# Usage:
#   pwsh -File zipfs/build-selfcontained.ps1
#   pwsh -File zipfs/build-selfcontained.ps1 -Embed
#   $env:BUILD_DIR = 'C:\rsbuild'; pwsh -File zipfs/build-selfcontained.ps1
#   $env:ROBCO_SETTINGS_DIST_DIR = 'C:\out'; pwsh -File zipfs/build-selfcontained.ps1

param([switch]$Embed)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

# Dependency versions, matching build-selfcontained.sh so both platforms ship
# the same interpreter.
$TclVer = if ($env:TCL_VER) { $env:TCL_VER } else { '9.0.2' }
$TkVer  = if ($env:TK_VER)  { $env:TK_VER }  else { '9.0.2' }

$RepoRoot = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$BuildDir = if ($env:BUILD_DIR) { $env:BUILD_DIR } else {
    Join-Path ([System.IO.Path]::GetTempPath()) ("robco-settings-selfcontained-" + [System.Guid]::NewGuid().ToString('N').Substring(0, 8))
}
$Src     = Join-Path $BuildDir 'src'
$Stage   = Join-Path $BuildDir 'interp'
$Runtime = Join-Path $BuildDir 'runtime'
foreach ($d in @($Src, $Stage, $Runtime)) { New-Item -ItemType Directory -Force -Path $d | Out-Null }
Write-Host "build dir: $BuildDir"

# Run a command and stop the build on a non-zero exit; nmake and cl report
# failure that way rather than by throwing.
function Invoke-Checked {
    param([string]$Exe, [string[]]$Arguments, [string]$WorkDir)
    Push-Location $WorkDir
    try {
        & $Exe @Arguments
        if ($LASTEXITCODE -ne 0) {
            throw "$Exe $($Arguments -join ' ') failed with exit code $LASTEXITCODE"
        }
    } finally { Pop-Location }
}

# Import the MSVC x64 environment into this session: vswhere locates the
# install, and the variables VsDevCmd sets (INCLUDE, LIB, PATH) are read back
# out of a child cmd and applied here, since a child process cannot export to
# its parent.
function Import-VsDevEnv {
    $vswhere = Join-Path ${env:ProgramFiles(x86)} 'Microsoft Visual Studio\Installer\vswhere.exe'
    if (-not (Test-Path $vswhere)) { throw "vswhere not found at $vswhere" }
    $vsPath = & $vswhere -latest -products * `
        -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 `
        -property installationPath
    if (-not $vsPath) { throw 'no Visual Studio install with the C++ toolset' }
    $devCmd = Join-Path $vsPath 'Common7\Tools\VsDevCmd.bat'
    Write-Host "== msvc environment ($vsPath) =="
    $lines = & "${env:COMSPEC}" /c "`"$devCmd`" -arch=amd64 -host_arch=amd64 >nul && set"
    foreach ($line in $lines) {
        if ($line -match '^([^=]+)=(.*)$') {
            Set-Item -Path ("env:" + $matches[1]) -Value $matches[2]
        }
    }
}

# Download and unpack one source tarball. curl rather than Invoke-WebRequest,
# so both platforms fetch through the same client and a download that behaves
# on one behaves on the other; SourceForge answers a download URL with a
# redirect to a mirror, and the two clients do not negotiate that alike.
#
# The unpack is guarded rather than left to fail on its own, because what
# arrives when a mirror declines is a readable HTML page, and "unrecognized
# archive format" two stages before the compiler says nothing about it.
function Get-Source {
    param([string]$Url, [string]$OutFile)
    $dest = Join-Path $Src $OutFile
    Invoke-Checked -Exe 'curl.exe' -WorkDir $Src `
        -Arguments @('-fsSL', '--retry', '3', '-o', $dest, $Url)
    & tar tzf $dest > $null 2>&1
    if ($LASTEXITCODE -ne 0) {
        $size = (Get-Item $dest).Length
        $head = (Get-Content $dest -TotalCount 5 -ErrorAction SilentlyContinue) -join "`n"
        throw "$Url did not deliver a tarball ($size bytes). It begins:`n$head"
    }
    Invoke-Checked -Exe 'tar' -Arguments @('xzf', $dest) -WorkDir $Src
}

# The one static .lib a stage produced, searched for by pattern because the
# name carries the version and the build-type suffix, and Tk prefixes its
# with the Tcl generation. The largest match wins: a stage that also leaves a
# stub library beside the real one leaves a far smaller file.
function Find-StaticLib {
    param([string]$Root, [string]$Pattern)
    $hit = Get-ChildItem -Path $Root -Recurse -Filter $Pattern -ErrorAction SilentlyContinue |
        Sort-Object Length -Descending | Select-Object -First 1
    if (-not $hit) { throw "no library matching $Pattern under $Root" }
    Write-Host "  found $($hit.FullName)"
    return $hit.FullName
}

Import-VsDevEnv

Write-Host '== fetching sources =='
Get-Source "https://prdownloads.sourceforge.net/tcl/tcl$TclVer-src.tar.gz" 'tcl.tar.gz'
Get-Source "https://prdownloads.sourceforge.net/tcl/tk$TkVer-src.tar.gz"   'tk.tar.gz'

$TclSrc = Join-Path $Src "tcl$TclVer"
$TkSrc  = Join-Path $Src "tk$TkVer"

# OPTS=static links Tcl/Tk into our binary; msvcrt keeps the C runtime shared,
# so the .exe uses the system CRT every Windows install already has rather than
# carrying a second copy of it.
$Opts = 'OPTS=static,msvcrt'

Write-Host '== 1. static Tcl =='
Invoke-Checked -Exe 'nmake' -WorkDir (Join-Path $TclSrc 'win') `
    -Arguments @('-f', 'makefile.vc', $Opts, "INSTALLDIR=$Stage", 'release', 'install')

Write-Host '== 2. static Tk =='
Invoke-Checked -Exe 'nmake' -WorkDir (Join-Path $TkSrc 'win') `
    -Arguments @('-f', 'makefile.vc', $Opts, "TCLDIR=$TclSrc", "INSTALLDIR=$Stage", 'release', 'install')

Write-Host '== 3. custom wish =='
$TclLib = Find-StaticLib -Root $TclSrc -Pattern 'tcl*s.lib'
$TkLib  = Find-StaticLib -Root $TkSrc  -Pattern '*tk*s.lib'
# Tk's static library still reaches Tcl through the stubs table, so the stub
# library joins the link exactly as libtclstub.a does on the Unix side.
$StubLib = Find-StaticLib -Root $TclSrc -Pattern 'tclstub*.lib'
$Wish = Join-Path $BuildDir 'robco-settings-wish.exe'

# STATIC_BUILD switches tcl.h and tk.h from the stubs table to direct entry
# points, which is what a statically linked interpreter needs.
#
# /MD selects the DLL C runtime, the half of OPTS=static,msvcrt that governs
# this link: Tcl and Tk were compiled against it, and their objects call the
# CRT through its import symbols. Linking them into a binary built for the
# static CRT leaves every one of those unresolved.
#
# The include directories are one list, used both for this link and for the
# terminal's own build of the same source file under -Embed. Two lists would
# drift the first time a header moved, and the failure would land in the
# other build.
$IncludeDirs = @(
    (Join-Path $Stage 'include')
    (Join-Path $TclSrc 'generic')
    (Join-Path $TkSrc 'generic')
    (Join-Path $TkSrc 'win')
    (Join-Path $TkSrc 'xlib')
)
$clArgs = @(
    '/nologo', '/O2', '/MD', '/DSTATIC_BUILD'
) + ($IncludeDirs | ForEach-Object { "/I$_" }) + @(
    (Join-Path $RepoRoot 'zipfs\appinit.c'),
    "/Fe:$Wish",
    '/link', '/SUBSYSTEM:WINDOWS', '/ENTRY:mainCRTStartup',
    $TkLib, $TclLib, $StubLib,
    'netapi32.lib', 'user32.lib', 'advapi32.lib', 'userenv.lib', 'ws2_32.lib',
    'gdi32.lib', 'comdlg32.lib', 'imm32.lib', 'comctl32.lib', 'shell32.lib',
    'uuid.lib', 'ole32.lib', 'oleaut32.lib', 'winspool.lib'
)
Invoke-Checked -Exe 'cl' -Arguments $clArgs -WorkDir $BuildDir
if (-not (Test-Path $Wish)) { throw "custom wish was not produced at $Wish" }

Write-Host '== 4. runtime tree + image =='
# As on Unix: a static interpreter's script library lives in the zip appended
# to the stock tclsh, not on disk, so the authoritative trees are the source
# library/ dirs.
Copy-Item -Recurse -Force (Join-Path $TclSrc 'library') (Join-Path $Runtime 'tcl_library')
Copy-Item -Recurse -Force (Join-Path $TkSrc  'library') (Join-Path $Runtime 'tk_library')

# The interpreter that runs build.tcl: it needs zipfs and nothing else, so
# either the installed copy or the one left in the build tree serves. Install
# is asked first, the build tree second, because a static build does not
# always leave an interpreter under the install prefix.
$tclsh = @(
    Get-ChildItem -Path $Stage  -Recurse -Filter 'tclsh*.exe' -ErrorAction SilentlyContinue
    Get-ChildItem -Path $TclSrc -Recurse -Filter 'tclsh*.exe' -ErrorAction SilentlyContinue
) | Select-Object -First 1
if (-not $tclsh) {
    $seen = (Get-ChildItem -Path $Stage -Recurse -Filter '*.exe' -ErrorAction SilentlyContinue |
        ForEach-Object { $_.FullName }) -join "`n"
    throw "no tclsh under $Stage or $TclSrc. Executables installed:`n$seen"
}
Write-Host "  interpreter $($tclsh.FullName)"

$env:ROBCO_SETTINGS_WISH = $Wish
$env:ROBCO_SETTINGS_RUNTIME = $Runtime
Invoke-Checked -Exe $tclsh.FullName `
    -Arguments @((Join-Path $RepoRoot 'zipfs\build.tcl')) -WorkDir $RepoRoot

if ($Embed) {
    Write-Host '== 5. payload zip + the terminal build''s environment =='
    # The same payload again, this time as a plain zip for the terminal to
    # carry in its own image. The wish is cleared for this run so build.tcl
    # writes the zip alone; the runtime tree stays, because an embedded
    # interpreter needs the script libraries exactly as the standalone image
    # does.
    $Payload = Join-Path $BuildDir 'robco-settings-payload.zip'
    $env:ROBCO_SETTINGS_WISH = ''
    $env:ROBCO_SETTINGS_ZIP_OUT = $Payload

    # tcltest is a module and no part of the script library, so it is found
    # and named here or the image cannot run its own suites. The stage is
    # asked first and the source tree second, the same order as the tclsh
    # above and for the same reason.
    $tm = @(
        Get-ChildItem -Path $Stage -Recurse -Filter 'tcltest-*.tm' -ErrorAction SilentlyContinue
        Get-ChildItem -Path (Join-Path $TclSrc 'library') -Recurse -Filter 'tcltest-*.tm' -ErrorAction SilentlyContinue
    ) | Select-Object -First 1
    if ($tm) {
        Write-Host "  tcltest $($tm.FullName)"
        $env:ROBCO_SETTINGS_TCLTEST = $tm.FullName
    } else {
        Write-Host '  no tcltest module found; the image will not run its own suites'
    }

    Invoke-Checked -Exe $tclsh.FullName `
        -Arguments @((Join-Path $RepoRoot 'zipfs\build.tcl')) -WorkDir $RepoRoot
    if (-not (Test-Path $Payload)) { throw "payload zip was not produced at $Payload" }

    # What the terminal's own build reads: the payload to embed, and the
    # static Tcl and Tk to link appinit.c against. The include list is the
    # one the cl invocation above used, joined the way a Windows path list
    # is; the libraries are the ones already found for that link.
    $CargoEnv = [ordered]@{
        ROBCO_SETTINGS_ZIP = $Payload
        ROBCO_TCL_INCLUDE  = ($IncludeDirs -join ';')
        ROBCO_TCL_LIB      = $TclLib
        ROBCO_TK_LIB       = $TkLib
        ROBCO_TCL_STUB_LIB = $StubLib
    }
    # Printed always, so a hand-run build can be copied out of the log, and
    # appended to GITHUB_ENV when there is one, so a workflow's later steps
    # inherit it without repeating any of this.
    foreach ($name in $CargoEnv.Keys) {
        $line = "$name=$($CargoEnv[$name])"
        Write-Host $line
        if ($env:GITHUB_ENV) { Add-Content -Path $env:GITHUB_ENV -Value $line }
    }
}

Write-Host "done. Keep $BuildDir for reuse, or remove it."
