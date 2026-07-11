param(
    [switch]$Rc,
    [string]$Tag,
    [string]$InstallDir = (Join-Path $HOME ".local\bin"),
    [switch]$NoPath
)

$ErrorActionPreference = "Stop"

function Log($Message) {
    Write-Host "==> $Message" -ForegroundColor Blue
}

function Die($Message) {
    Write-Error $Message
    exit 1
}

function Get-ArchitectureSuffix {
    switch ($env:PROCESSOR_ARCHITECTURE) {
        "ARM64" { return "aarch64-windows" }
        "AMD64" { return "x86_64-windows" }
        default { Die "Unsupported Windows architecture: $env:PROCESSOR_ARCHITECTURE" }
    }
}

function Get-Release {
    if ($Tag) {
        Log "Fetching release $Tag"
        return Invoke-RestMethod "https://api.github.com/repos/philip-flop/edit/releases/tags/$Tag"
    }

    if ($Rc) {
        Log "Fetching latest release candidate"
        $releases = Invoke-RestMethod "https://api.github.com/repos/philip-flop/edit/releases?per_page=20"
        $release = $releases | Where-Object { $_.prerelease } | Select-Object -First 1
        if (-not $release) {
            Die "Could not find a release candidate."
        }
        return $release
    }

    Log "Fetching latest release"
    return Invoke-RestMethod "https://api.github.com/repos/philip-flop/edit/releases/latest"
}

if ($Rc -and $Tag) {
    Die "-Rc and -Tag cannot be used together."
}

$InstallDir = [System.IO.Path]::GetFullPath($InstallDir)
$arch = Get-ArchitectureSuffix
$release = Get-Release
$version = $release.tag_name.TrimStart("v")
$assetName = "jedit-$version-$arch.zip"
$asset = $release.assets | Where-Object { $_.name -eq $assetName } | Select-Object -First 1

if (-not $asset) {
    Die "Could not find $assetName on release $($release.tag_name)."
}

$tempDir = Join-Path ([System.IO.Path]::GetTempPath()) ("jedit-install-" + [System.Guid]::NewGuid())
$zipPath = Join-Path $tempDir $assetName
$extractDir = Join-Path $tempDir "extract"

try {
    New-Item -ItemType Directory -Force $tempDir | Out-Null
    New-Item -ItemType Directory -Force $extractDir | Out-Null

    Log "Downloading $assetName"
    Invoke-WebRequest -Uri $asset.browser_download_url -OutFile $zipPath

    Log "Extracting"
    Expand-Archive -Path $zipPath -DestinationPath $extractDir -Force

    $exe = Get-ChildItem -Path $extractDir -Recurse -Filter jedit.exe | Select-Object -First 1
    if (-not $exe) {
        Die "jedit.exe was not found in $assetName."
    }

    Log "Installing to $InstallDir"
    New-Item -ItemType Directory -Force $InstallDir | Out-Null
    Copy-Item -Force $exe.FullName (Join-Path $InstallDir "jedit.exe")

    if (-not $NoPath) {
        $userPath = [Environment]::GetEnvironmentVariable("Path", "User")
        $pathEntries = @()
        if ($userPath) {
            $pathEntries = $userPath -split ";"
        }

        $alreadyInPath = $false
        foreach ($entry in $pathEntries) {
            if ($entry.TrimEnd("\") -ieq $InstallDir.TrimEnd("\")) {
                $alreadyInPath = $true
                break
            }
        }

        if (-not $alreadyInPath) {
            Log "Adding $InstallDir to your user PATH"
            if ($userPath) {
                [Environment]::SetEnvironmentVariable("Path", "$userPath;$InstallDir", "User")
            } else {
                [Environment]::SetEnvironmentVariable("Path", $InstallDir, "User")
            }
            $env:Path = "$env:Path;$InstallDir"
            Write-Host "Open a new terminal for the PATH change to apply everywhere."
        }
    }

    Write-Host "Done. Run 'jedit' to start."
} finally {
    if (Test-Path $tempDir) {
        Remove-Item -Recurse -Force $tempDir
    }
}
