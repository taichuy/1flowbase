param(
  [string]$DbPassword = $env:FLOWBASE_DB_PASSWORD,
  [string]$RootAccount = $env:FLOWBASE_ROOT_ACCOUNT,
  [string]$RootPassword = $env:FLOWBASE_ROOT_PASSWORD,
  [string]$ProviderSecret = $env:FLOWBASE_PROVIDER_SECRET,
  [string]$WebPort = $env:FLOWBASE_WEB_PORT,
  [string]$CookieSecure = $env:FLOWBASE_COOKIE_SECURE,
  [string]$DatabaseMode = $env:FLOWBASE_DATABASE_MODE,
  [string]$ExternalPostgresHost = $env:FLOWBASE_EXTERNAL_POSTGRES_HOST,
  [string]$ExternalPostgresPort = $env:FLOWBASE_EXTERNAL_POSTGRES_PORT,
  [string]$ExternalPostgresDb = $env:FLOWBASE_EXTERNAL_POSTGRES_DB,
  [string]$ExternalPostgresUser = $env:FLOWBASE_EXTERNAL_POSTGRES_USER,
  [string]$ExternalPostgresPassword = $env:FLOWBASE_EXTERNAL_POSTGRES_PASSWORD,
  [string]$ExternalPostgresSslmode = $env:FLOWBASE_EXTERNAL_POSTGRES_SSLMODE,
  [string]$PluginGithubProxyUrl = $env:FLOWBASE_OFFICIAL_PLUGIN_GITHUB_PROXY_URL,
  [string]$OfficialPluginSignatureRequired = $env:FLOWBASE_OFFICIAL_PLUGIN_SIGNATURE_REQUIRED,
  [switch]$Pull,
  [switch]$NoPull,
  [switch]$Start,
  [switch]$NoStart,
  [switch]$PrepareOnly,
  [switch]$NonInteractive
)

$ErrorActionPreference = "Stop"
$ProgressPreference = "SilentlyContinue"

$FlowbaseRepo = if ($env:FLOWBASE_REPO) { $env:FLOWBASE_REPO } else { "taichuy/1flowbase" }
$FlowbaseRef = if ($env:FLOWBASE_REF) { $env:FLOWBASE_REF } else { "main" }
$FlowbaseArchiveUrl = if ($env:FLOWBASE_ARCHIVE_URL) {
  $env:FLOWBASE_ARCHIVE_URL
} else {
  "https://codeload.github.com/$FlowbaseRepo/tar.gz/refs/heads/$FlowbaseRef"
}
$FlowbaseArchiveDockerDir = "1flowbase-$FlowbaseRef/docker"
$DefaultOfficialPluginGithubProxyUrl = "https://gh-proxy.com/"
$ShouldPrompt = -not ($NonInteractive -or $env:FLOWBASE_NON_INTERACTIVE -eq "1" -or $env:FLOWBASE_NON_INTERACTIVE -eq "true")
$PullImages = $null
$StartContainers = $null

if ($Pull -or $env:FLOWBASE_PULL_IMAGES -eq "1" -or $env:FLOWBASE_PULL_IMAGES -eq "true") {
  $PullImages = $true
}
if ($NoPull -or $env:FLOWBASE_PULL_IMAGES -eq "0" -or $env:FLOWBASE_PULL_IMAGES -eq "false") {
  $PullImages = $false
}
if ($Start -or $env:FLOWBASE_START_CONTAINERS -eq "1" -or $env:FLOWBASE_START_CONTAINERS -eq "true") {
  $StartContainers = $true
}
if ($NoStart -or $PrepareOnly -or $env:FLOWBASE_START_CONTAINERS -eq "0" -or $env:FLOWBASE_START_CONTAINERS -eq "false") {
  $StartContainers = $false
}
if (-not $PluginGithubProxyUrl -and $env:API_OFFICIAL_PLUGIN_GITHUB_PROXY_URL) {
  $PluginGithubProxyUrl = $env:API_OFFICIAL_PLUGIN_GITHUB_PROXY_URL
}
if (-not $OfficialPluginSignatureRequired -and $env:API_OFFICIAL_PLUGIN_SIGNATURE_REQUIRED) {
  $OfficialPluginSignatureRequired = $env:API_OFFICIAL_PLUGIN_SIGNATURE_REQUIRED
}
if (-not $CookieSecure -and $env:API_COOKIE_SECURE) {
  $CookieSecure = $env:API_COOKIE_SECURE
}

function Fail([string]$Message) {
  Write-Host $Message
  exit 1
}

function Read-EnvValue([string]$Key, [string]$Path) {
  if (-not (Test-Path $Path)) {
    return $null
  }

  foreach ($Line in Get-Content $Path) {
    if ($Line -match "^$([regex]::Escape($Key))=(.*)$") {
      return $matches[1]
    }
  }

  return $null
}

function Set-EnvValue([string]$Key, [string]$Value, [string]$Path) {
  $Lines = [System.Collections.Generic.List[string]]::new()
  if (Test-Path $Path) {
    foreach ($Line in Get-Content $Path) {
      $Lines.Add($Line)
    }
  }

  $Found = $false
  for ($Index = 0; $Index -lt $Lines.Count; $Index++) {
    if ($Lines[$Index] -match "^$([regex]::Escape($Key))=") {
      $Lines[$Index] = "$Key=$Value"
      $Found = $true
    }
  }

  if (-not $Found) {
    $Lines.Add("$Key=$Value")
  }

  Set-Content -Path $Path -Value $Lines -Encoding UTF8
}

function Test-ProviderSecretMasterKeyPlaceholder([string]$Value) {
  if ($null -eq $Value) {
    return $true
  }

  $TrimmedValue = $Value.Trim()
  return $TrimmedValue -in @(
    "",
    "change-me-provider-secret-master-key",
    "dev-provider-secret-master-key-unsafe"
  )
}

function New-ProviderSecretMasterKey() {
  $Bytes = New-Object byte[] 32
  $Generator = [System.Security.Cryptography.RandomNumberGenerator]::Create()
  try {
    $Generator.GetBytes($Bytes)
  } finally {
    $Generator.Dispose()
  }

  return [System.BitConverter]::ToString($Bytes).Replace("-", "").ToLowerInvariant()
}

function Ensure-ProviderSecretMasterKey() {
  $CurrentValue = Read-EnvValue "API_PROVIDER_SECRET_MASTER_KEY" ".\docker\.env"
  if (Test-ProviderSecretMasterKeyPlaceholder $CurrentValue) {
    Set-EnvValue "API_PROVIDER_SECRET_MASTER_KEY" (New-ProviderSecretMasterKey) ".\docker\.env"
    Write-Host "Generated API_PROVIDER_SECRET_MASTER_KEY in docker/.env."
  }
}

function Ensure-OfficialPluginSignatureRequired() {
  $CurrentValue = Read-EnvValue "API_OFFICIAL_PLUGIN_SIGNATURE_REQUIRED" ".\docker\.env"
  if (-not $CurrentValue) {
    Set-EnvValue "API_OFFICIAL_PLUGIN_SIGNATURE_REQUIRED" "true" ".\docker\.env"
    Write-Host "Added API_OFFICIAL_PLUGIN_SIGNATURE_REQUIRED=true to docker/.env."
  }
}

function Ensure-CookieSecureDefault() {
  $CurrentValue = Read-EnvValue "API_COOKIE_SECURE" ".\docker\.env"
  if (-not $CurrentValue) {
    Set-EnvValue "API_COOKIE_SECURE" "true" ".\docker\.env"
    Write-Host "Added API_COOKIE_SECURE=true to docker/.env."
  }
}

function Convert-ToDatabaseMode([string]$Name, [string]$Value) {
  switch ($Value) {
    { -not $_ } { return "internal" }
    "1" { return "internal" }
    "internal" { return "internal" }
    "Internal" { return "internal" }
    "INTERNAL" { return "internal" }
    "2" { return "external" }
    "external" { return "external" }
    "External" { return "external" }
    "EXTERNAL" { return "external" }
    default { Fail "Invalid value for ${Name}: $Value. Use internal or external." }
  }
}

function Convert-ToPostgresSslmode([string]$Name, [string]$Value) {
  if (-not $Value) {
    $Value = "prefer"
  }

  $NormalizedValue = $Value.ToLowerInvariant()
  if ($NormalizedValue -in @("disable", "allow", "prefer", "require", "verify-ca", "verify-full")) {
    return $NormalizedValue
  }

  Fail "Invalid value for ${Name}: $Value. Use disable, allow, prefer, require, verify-ca, or verify-full."
}

function Convert-ToDatabaseUrlValue([string]$Value) {
  return [System.Uri]::EscapeDataString($Value)
}

function Ensure-DatabaseModeDefault([string]$Path) {
  $CurrentValue = Read-EnvValue "DATABASE_MODE" $Path
  if (-not $CurrentValue) {
    Set-EnvValue "DATABASE_MODE" "internal" $Path
    Write-Host "Added DATABASE_MODE=internal to docker/.env."
  }
}

function Ensure-ExternalPostgresDefaults([string]$Path) {
  if (-not (Read-EnvValue "EXTERNAL_POSTGRES_PORT" $Path)) {
    Set-EnvValue "EXTERNAL_POSTGRES_PORT" "5432" $Path
  }
  if (-not (Read-EnvValue "EXTERNAL_POSTGRES_DB" $Path)) {
    Set-EnvValue "EXTERNAL_POSTGRES_DB" "1flowbase" $Path
  }
  if (-not (Read-EnvValue "EXTERNAL_POSTGRES_USER" $Path)) {
    Set-EnvValue "EXTERNAL_POSTGRES_USER" "postgres" $Path
  }
  if (-not (Read-EnvValue "EXTERNAL_POSTGRES_SSLMODE" $Path)) {
    Set-EnvValue "EXTERNAL_POSTGRES_SSLMODE" "prefer" $Path
  }
}

function Sync-ExternalDatabaseUrl([string]$Path) {
  Ensure-ExternalPostgresDefaults $Path

  $HostName = Read-EnvValue "EXTERNAL_POSTGRES_HOST" $Path
  $Port = Read-EnvValue "EXTERNAL_POSTGRES_PORT" $Path
  $DatabaseName = Read-EnvValue "EXTERNAL_POSTGRES_DB" $Path
  $User = Read-EnvValue "EXTERNAL_POSTGRES_USER" $Path
  $Password = Read-EnvValue "EXTERNAL_POSTGRES_PASSWORD" $Path
  $Sslmode = Convert-ToPostgresSslmode "EXTERNAL_POSTGRES_SSLMODE" (Read-EnvValue "EXTERNAL_POSTGRES_SSLMODE" $Path)

  if (-not $HostName) { Fail "EXTERNAL_POSTGRES_HOST is required when DATABASE_MODE=external." }
  if (-not $Port -or $Port -notmatch "^[0-9]+$") { Fail "EXTERNAL_POSTGRES_PORT must be a number." }
  if (-not $DatabaseName) { Fail "EXTERNAL_POSTGRES_DB is required when DATABASE_MODE=external." }
  if (-not $User) { Fail "EXTERNAL_POSTGRES_USER is required when DATABASE_MODE=external." }
  if (-not $Password) { Fail "EXTERNAL_POSTGRES_PASSWORD is required when DATABASE_MODE=external." }

  Set-EnvValue "EXTERNAL_POSTGRES_SSLMODE" $Sslmode $Path
  $EncodedUser = Convert-ToDatabaseUrlValue $User
  $EncodedPassword = Convert-ToDatabaseUrlValue $Password
  $EncodedDatabaseName = Convert-ToDatabaseUrlValue $DatabaseName
  Set-EnvValue "API_DATABASE_URL" "postgres://${EncodedUser}:${EncodedPassword}@${HostName}:${Port}/${EncodedDatabaseName}?sslmode=${Sslmode}" $Path
}

function Sync-DatabaseConfiguration([string]$Path) {
  $Mode = Convert-ToDatabaseMode "DATABASE_MODE" (Read-EnvValue "DATABASE_MODE" $Path)
  Set-EnvValue "DATABASE_MODE" $Mode $Path

  if ($Mode -eq "external") {
    Sync-ExternalDatabaseUrl $Path
  } else {
    Set-EnvValue "API_DATABASE_URL" "" $Path
  }
}

function Format-EnvDisplayValue([string]$Key, [string]$Value) {
  if (-not $Value) {
    return "<empty>"
  }

  switch ($Key) {
    "POSTGRES_PASSWORD" { return "<set>" }
    "EXTERNAL_POSTGRES_PASSWORD" { return "<set>" }
    "API_DATABASE_URL" { return "<set>" }
    "BOOTSTRAP_ROOT_PASSWORD" { return "<set>" }
    "API_PROVIDER_SECRET_MASTER_KEY" { return "<set>" }
    default { return $Value }
  }
}

function Show-EnvSummary([string]$Path) {
  Write-Host "Current docker/.env configuration:"
  foreach ($Key in @(
      "FLOWBASE_WEB_VERSION",
      "FLOWBASE_API_SERVER_VERSION",
      "FLOWBASE_PLUGIN_RUNNER_VERSION",
      "WEB_PORT",
      "DATABASE_MODE",
      "POSTGRES_DB",
      "POSTGRES_USER",
      "POSTGRES_PASSWORD",
      "EXTERNAL_POSTGRES_HOST",
      "EXTERNAL_POSTGRES_PORT",
      "EXTERNAL_POSTGRES_DB",
      "EXTERNAL_POSTGRES_USER",
      "EXTERNAL_POSTGRES_PASSWORD",
      "EXTERNAL_POSTGRES_SSLMODE",
      "BOOTSTRAP_ROOT_ACCOUNT",
      "BOOTSTRAP_ROOT_PASSWORD",
      "API_PROVIDER_SECRET_MASTER_KEY",
      "API_COOKIE_SECURE",
      "API_OFFICIAL_PLUGIN_GITHUB_PROXY_URL",
      "API_OFFICIAL_PLUGIN_SIGNATURE_REQUIRED"
    )) {
    $Value = Read-EnvValue $Key $Path
    Write-Host "  $Key=$(Format-EnvDisplayValue $Key $Value)"
  }
}

function Prompt-EnvValue([string]$Key, [string]$Label) {
  $CurrentValue = Read-EnvValue $Key ".\docker\.env"
  if ($CurrentValue) {
    $InputValue = Read-Host "$Label [$CurrentValue]"
  } else {
    $InputValue = Read-Host "$Label"
  }

  if ($InputValue) {
    Set-EnvValue $Key $InputValue ".\docker\.env"
    Write-Host "Updated $Key in docker/.env."
  } else {
    if ($CurrentValue) {
      Write-Host "Keeping ${Key}: $CurrentValue"
    } else {
      Write-Host "Keeping ${Key}: empty"
    }
  }
}

function Prompt-SecretEnvValue([string]$Key, [string]$Label) {
  $CurrentValue = Read-EnvValue $Key ".\docker\.env"
  if ($CurrentValue) {
    $InputValue = Read-Host "$Label [<set>]"
  } else {
    $InputValue = Read-Host "$Label"
  }

  if ($InputValue) {
    Set-EnvValue $Key $InputValue ".\docker\.env"
    Write-Host "Updated $Key in docker/.env."
  } else {
    if ($CurrentValue) {
      Write-Host "Keeping ${Key}: <set>"
    } else {
      Write-Host "Keeping ${Key}: empty"
    }
  }
}

function Convert-ToYesNo([string]$Value) {
  if ($Value -match "^(y|yes|true|1)$") {
    return $true
  }
  return $false
}

function Convert-ToTrueFalseEnvValue([string]$Name, [string]$Value) {
  if ($Value -match "^(y|yes|true|1)$") {
    return "true"
  }
  if ($Value -match "^(n|no|false|0)$") {
    return "false"
  }
  Fail "Invalid value for ${Name}: $Value. Use true or false."
}

function Invoke-NativeQuiet([scriptblock]$Command) {
  $PreviousErrorActionPreference = $ErrorActionPreference
  $ErrorActionPreference = "Continue"
  try {
    & $Command *> $null
    return $LASTEXITCODE -eq 0
  } finally {
    $ErrorActionPreference = $PreviousErrorActionPreference
  }
}

function Prompt-YesNo([string]$Question, [bool]$Default) {
  $Suffix = if ($Default) { "[Y/n]" } else { "[y/N]" }
  $InputValue = Read-Host "$Question $Suffix"
  if (-not $InputValue) {
    return $Default
  }
  return Convert-ToYesNo $InputValue
}

function Prompt-DatabaseConfiguration() {
  $CurrentMode = Convert-ToDatabaseMode "DATABASE_MODE" (Read-EnvValue "DATABASE_MODE" ".\docker\.env")
  $DefaultChoice = if ($CurrentMode -eq "external") { "2" } else { "1" }
  $InputValue = Read-Host "Database mode (1=internal PostgreSQL, 2=external PostgreSQL) [$DefaultChoice]"
  if (-not $InputValue) {
    $InputValue = $CurrentMode
  }

  $Mode = Convert-ToDatabaseMode "DATABASE_MODE" $InputValue
  Set-EnvValue "DATABASE_MODE" $Mode ".\docker\.env"
  Write-Host "Updated DATABASE_MODE=$Mode in docker/.env."

  if ($Mode -eq "external") {
    Ensure-ExternalPostgresDefaults ".\docker\.env"
    Prompt-EnvValue "EXTERNAL_POSTGRES_HOST" "External PostgreSQL host/IP"
    Prompt-EnvValue "EXTERNAL_POSTGRES_PORT" "External PostgreSQL port"
    Prompt-EnvValue "EXTERNAL_POSTGRES_DB" "External PostgreSQL database"
    Prompt-EnvValue "EXTERNAL_POSTGRES_USER" "External PostgreSQL user"
    Prompt-SecretEnvValue "EXTERNAL_POSTGRES_PASSWORD" "External PostgreSQL password"
    Prompt-EnvValue "EXTERNAL_POSTGRES_SSLMODE" "External PostgreSQL sslmode"
    Sync-ExternalDatabaseUrl ".\docker\.env"
  } else {
    Set-EnvValue "API_DATABASE_URL" "" ".\docker\.env"
    Prompt-EnvValue "POSTGRES_PASSWORD" "Database password"
  }
}

function Prompt-OfficialPluginGithubProxyUrl() {
  $CurrentValue = Read-EnvValue "API_OFFICIAL_PLUGIN_GITHUB_PROXY_URL" ".\docker\.env"
  $UseProxy = Prompt-YesNo "Use CN GitHub plugin download accelerator?" ([bool]$CurrentValue)

  if ($UseProxy) {
    $DefaultValue = if ($CurrentValue) { $CurrentValue } else { $DefaultOfficialPluginGithubProxyUrl }
    $InputValue = Read-Host "Official plugin GitHub raw proxy URL [$DefaultValue]"
    if ($InputValue) {
      Set-EnvValue "API_OFFICIAL_PLUGIN_GITHUB_PROXY_URL" $InputValue ".\docker\.env"
    } else {
      Set-EnvValue "API_OFFICIAL_PLUGIN_GITHUB_PROXY_URL" $DefaultValue ".\docker\.env"
    }
    Write-Host "Updated API_OFFICIAL_PLUGIN_GITHUB_PROXY_URL in docker/.env."
  } else {
    Set-EnvValue "API_OFFICIAL_PLUGIN_GITHUB_PROXY_URL" "" ".\docker\.env"
    Write-Host "Disabled API_OFFICIAL_PLUGIN_GITHUB_PROXY_URL in docker/.env."
  }
}

function Prompt-TrueFalseEnvValue([string]$Key, [string]$Label) {
  $CurrentValue = Read-EnvValue $Key ".\docker\.env"
  if ($CurrentValue) {
    $InputValue = Read-Host "$Label [$CurrentValue]"
  } else {
    $InputValue = Read-Host "$Label"
  }

  if ($InputValue) {
    Set-EnvValue $Key (Convert-ToTrueFalseEnvValue $Key $InputValue) ".\docker\.env"
    Write-Host "Updated $Key in docker/.env."
  } else {
    if ($CurrentValue) {
      Write-Host "Keeping ${Key}: $CurrentValue"
    } else {
      Write-Host "Keeping ${Key}: empty"
    }
  }
}

function Test-PostgresDataExists() {
  $PgDataPath = ".\docker\postgres\data\pgdata"
  if (-not (Test-Path $PgDataPath)) {
    return $false
  }
  if (Test-Path (Join-Path $PgDataPath "PG_VERSION")) {
    return $true
  }
  return [bool](Get-ChildItem -Path $PgDataPath -Force -ErrorAction SilentlyContinue | Select-Object -First 1)
}

function Convert-ToSqlLiteralValue([string]$Value) {
  return $Value.Replace("'", "''")
}

function Convert-ToSqlUserIdentifier([string]$Value) {
  if (-not $Value -or $Value -match "[^A-Za-z0-9_]") {
    return '"' + $Value.Replace('"', '""') + '"'
  }
  return $Value
}

function Normalize-DockerArchitecture([string]$Architecture) {
  switch ($Architecture.ToLowerInvariant()) {
    "amd64" { return "amd64" }
    "x86_64" { return "amd64" }
    "arm64" { return "arm64" }
    "aarch64" { return "arm64" }
    "arm64/v8" { return "arm64" }
    default { return $Architecture.ToLowerInvariant() }
  }
}

function Normalize-DockerPlatform([string]$Platform) {
  $Parts = $Platform -split "/", 2
  if ($Parts.Count -eq 2) {
    $OsName = $Parts[0].ToLowerInvariant()
    $ArchName = Normalize-DockerArchitecture $Parts[1]
  } else {
    $OsName = "linux"
    $ArchName = Normalize-DockerArchitecture $Platform
  }

  return "$OsName/$ArchName"
}

function Get-EffectiveDockerPlatform() {
  if ($env:DOCKER_DEFAULT_PLATFORM) {
    return Normalize-DockerPlatform $env:DOCKER_DEFAULT_PLATFORM
  }

  $RawPlatform = docker info --format "{{.OSType}}/{{.Architecture}}" 2>$null
  if ($LASTEXITCODE -ne 0 -or -not $RawPlatform) {
    Fail "Could not detect Docker server platform."
  }

  return Normalize-DockerPlatform $RawPlatform
}

function Get-EnvOrFileValue([string]$Key, [string]$Path, [string]$DefaultValue) {
  $EnvValue = [Environment]::GetEnvironmentVariable($Key)
  if ($EnvValue) {
    return $EnvValue
  }

  $FileValue = Read-EnvValue $Key $Path
  if ($FileValue) {
    return $FileValue
  }

  return $DefaultValue
}

function Get-FlowbaseImageRefs([string]$Path) {
  $WebVersion = Get-EnvOrFileValue "FLOWBASE_WEB_VERSION" $Path "latest"
  $ApiVersion = Get-EnvOrFileValue "FLOWBASE_API_SERVER_VERSION" $Path "latest"
  $PluginRunnerVersion = Get-EnvOrFileValue "FLOWBASE_PLUGIN_RUNNER_VERSION" $Path "latest"

  return @(
    "ghcr.io/taichuy/1flowbase-web:$WebVersion",
    "ghcr.io/taichuy/1flowbase-api-server:$ApiVersion",
    "ghcr.io/taichuy/1flowbase-plugin-runner:$PluginRunnerVersion"
  )
}

function Test-FlowbaseUsesLatestImageTags([string]$Path) {
  return (
    (Get-EnvOrFileValue "FLOWBASE_WEB_VERSION" $Path "latest") -eq "latest" -and
    (Get-EnvOrFileValue "FLOWBASE_API_SERVER_VERSION" $Path "latest") -eq "latest" -and
    (Get-EnvOrFileValue "FLOWBASE_PLUGIN_RUNNER_VERSION" $Path "latest") -eq "latest"
  )
}

function Test-LocalLatestFlowbaseImages([string]$Path) {
  if (-not (Test-FlowbaseUsesLatestImageTags $Path)) {
    return $false
  }

  foreach ($Image in Get-FlowbaseImageRefs $Path) {
    if (-not (Invoke-NativeQuiet { docker image inspect $Image })) {
      return $false
    }
  }

  return $true
}

function Test-ImageManifestPlatform([string]$Image, [string]$Platform) {
  $ManifestLines = docker manifest inspect $Image 2>$null
  if ($LASTEXITCODE -ne 0 -or -not $ManifestLines) {
    return $null
  }

  $Manifest = $ManifestLines -join "`n"
  $Parts = $Platform -split "/", 2
  $OsName = [regex]::Escape($Parts[0])
  $ArchName = [regex]::Escape($Parts[1])

  return (
    $Manifest -match "`"os`"\s*:\s*`"$OsName`"" -and
    $Manifest -match "`"architecture`"\s*:\s*`"$ArchName`""
  )
}

function Assert-FlowbaseImagePlatformSupport() {
  $Platform = Get-EffectiveDockerPlatform
  Write-Host "Detected Docker platform: $Platform"

  if ($Platform -ne "linux/amd64" -and $Platform -ne "linux/arm64") {
    Fail "This 1flowbase Docker package supports linux/amd64 and linux/arm64. Detected Docker platform: $Platform."
  }

  foreach ($Image in Get-FlowbaseImageRefs ".\.env") {
    $SupportsPlatform = Test-ImageManifestPlatform $Image $Platform
    if ($null -eq $SupportsPlatform) {
      Write-Host "Could not verify Docker image platform support for $Image; continuing to Docker pull."
    } elseif (-not $SupportsPlatform) {
      Fail "Docker image $Image does not publish $Platform. Rebuild/publish the 1flowbase multi-platform images, or set DOCKER_DEFAULT_PLATFORM=linux/amd64 as a temporary workaround on ARM machines."
    }
  }
}

if (-not (Get-Command docker -ErrorAction SilentlyContinue)) {
  Fail "Docker is required. Install Docker Desktop or Docker Engine first."
}

$UseDockerComposePlugin = $false
if (Invoke-NativeQuiet { docker compose version }) {
  $UseDockerComposePlugin = $true
} elseif (-not (Get-Command docker-compose -ErrorAction SilentlyContinue)) {
  Fail "Docker Compose is required. Install the Docker Compose plugin or docker-compose first."
}

function Invoke-ComposeCommand([string[]]$Arguments) {
  if ($UseDockerComposePlugin) {
    & docker compose @Arguments
  } else {
    & docker-compose @Arguments
  }
}

function Sync-PostgresPassword([string]$NewPassword) {
  $DbName = Read-EnvValue "POSTGRES_DB" ".\.env"
  $DbUser = Read-EnvValue "POSTGRES_USER" ".\.env"
  if (-not $DbName) { $DbName = "1flowbase" }
  if (-not $DbUser) { $DbUser = "postgres" }

  $EscapedPassword = Convert-ToSqlLiteralValue $NewPassword
  $DbUserSql = Convert-ToSqlUserIdentifier $DbUser

  Write-Host "Postgres password changed and existing pgdata was found; syncing database user password."
  Invoke-ComposeCommand @("up", "-d", "db")
  if ($LASTEXITCODE -ne 0) {
    Fail "Could not start Postgres for password sync."
  }

  $Ready = $false
  for ($Attempt = 1; $Attempt -le 30; $Attempt++) {
    if (Invoke-NativeQuiet { Invoke-ComposeCommand @("exec", "-T", "db", "pg_isready", "-U", $DbUser, "-d", $DbName) }) {
      $Ready = $true
      break
    }
    Start-Sleep -Seconds 2
  }

  if (-not $Ready) {
    Fail "Postgres did not become ready; could not sync POSTGRES_PASSWORD."
  }

  Invoke-ComposeCommand @(
    "exec", "-T", "db", "psql", "-U", $DbUser, "-d", $DbName,
    "-v", "ON_ERROR_STOP=1",
    "-c", "ALTER USER $DbUserSql WITH PASSWORD '$EscapedPassword';"
  )
  if ($LASTEXITCODE -ne 0) {
    Fail "Could not sync Postgres user password."
  }
  Write-Host "Synced Postgres user password for $DbUser."
}

if (Test-Path ".\docker") {
  Write-Host "Using existing ./docker directory."
} else {
  if (-not (Get-Command tar -ErrorAction SilentlyContinue)) {
    Fail "tar is required to unpack the 1flowbase archive."
  }

  $TempDir = Join-Path ([System.IO.Path]::GetTempPath()) ("1flowbase-" + [System.Guid]::NewGuid().ToString("N"))
  New-Item -ItemType Directory -Path $TempDir | Out-Null
  $Archive = Join-Path $TempDir "1flowbase.tar.gz"
  Write-Host "Downloading 1flowbase docker files."
  Invoke-WebRequest -Uri $FlowbaseArchiveUrl -OutFile $Archive
  tar -xzf $Archive -C $TempDir $FlowbaseArchiveDockerDir
  $ExtractedDockerDir = Join-Path $TempDir ($FlowbaseArchiveDockerDir -replace "/", [System.IO.Path]::DirectorySeparatorChar)
  Move-Item -Path $ExtractedDockerDir -Destination ".\docker"
  Remove-Item -Recurse -Force $TempDir
  Write-Host "Downloaded ./docker."
}

$PromptConfigValues = $false
$OldPostgresPassword = $null
$OldBootstrapRootAccount = $null
$OldBootstrapRootPassword = $null
$OldProviderSecret = $null
if (Test-Path ".\docker\.env") {
  $OldPostgresPassword = Read-EnvValue "POSTGRES_PASSWORD" ".\docker\.env"
  $OldBootstrapRootAccount = Read-EnvValue "BOOTSTRAP_ROOT_ACCOUNT" ".\docker\.env"
  $OldBootstrapRootPassword = Read-EnvValue "BOOTSTRAP_ROOT_PASSWORD" ".\docker\.env"
  $OldProviderSecret = Read-EnvValue "API_PROVIDER_SECRET_MASTER_KEY" ".\docker\.env"
}

if (-not (Test-Path ".\docker\.env")) {
  Copy-Item ".\docker\.env.example" ".\docker\.env"
  Write-Host "Created docker/.env from docker/.env.example."
  Ensure-DatabaseModeDefault ".\docker\.env"
  Ensure-OfficialPluginSignatureRequired
  Ensure-CookieSecureDefault
  $PromptConfigValues = $true
} else {
  Write-Host "Using existing docker/.env."
  Ensure-DatabaseModeDefault ".\docker\.env"
  Ensure-OfficialPluginSignatureRequired
  Ensure-CookieSecureDefault
  if ($ShouldPrompt) {
    Show-EnvSummary ".\docker\.env"
    $UpdateEnv = Prompt-YesNo "Update current docker/.env configuration?" $false
    if ($UpdateEnv) {
      $PromptConfigValues = $true
    } else {
      Write-Host "Keeping existing docker/.env."
    }
  }
}

if ($DbPassword) {
  Set-EnvValue "POSTGRES_PASSWORD" $DbPassword ".\docker\.env"
  Write-Host "Updated POSTGRES_PASSWORD in docker/.env."
}
if ($RootAccount) {
  Set-EnvValue "BOOTSTRAP_ROOT_ACCOUNT" $RootAccount ".\docker\.env"
  Write-Host "Updated BOOTSTRAP_ROOT_ACCOUNT in docker/.env."
}
if ($RootPassword) {
  Set-EnvValue "BOOTSTRAP_ROOT_PASSWORD" $RootPassword ".\docker\.env"
  Write-Host "Updated BOOTSTRAP_ROOT_PASSWORD in docker/.env."
}
if ($ProviderSecret) {
  Set-EnvValue "API_PROVIDER_SECRET_MASTER_KEY" $ProviderSecret ".\docker\.env"
  Write-Host "Updated API_PROVIDER_SECRET_MASTER_KEY in docker/.env."
}
if ($WebPort) {
  Set-EnvValue "WEB_PORT" $WebPort ".\docker\.env"
  Write-Host "Updated WEB_PORT in docker/.env."
}
if ($CookieSecure) {
  $CookieSecure = Convert-ToTrueFalseEnvValue "API_COOKIE_SECURE" $CookieSecure
  Set-EnvValue "API_COOKIE_SECURE" $CookieSecure ".\docker\.env"
  Write-Host "Updated API_COOKIE_SECURE in docker/.env."
}
if ($DatabaseMode) {
  $DatabaseMode = Convert-ToDatabaseMode "DATABASE_MODE" $DatabaseMode
  Set-EnvValue "DATABASE_MODE" $DatabaseMode ".\docker\.env"
  Write-Host "Updated DATABASE_MODE in docker/.env."
}
if ($ExternalPostgresHost) {
  Set-EnvValue "EXTERNAL_POSTGRES_HOST" $ExternalPostgresHost ".\docker\.env"
  Write-Host "Updated EXTERNAL_POSTGRES_HOST in docker/.env."
}
if ($ExternalPostgresPort) {
  Set-EnvValue "EXTERNAL_POSTGRES_PORT" $ExternalPostgresPort ".\docker\.env"
  Write-Host "Updated EXTERNAL_POSTGRES_PORT in docker/.env."
}
if ($ExternalPostgresDb) {
  Set-EnvValue "EXTERNAL_POSTGRES_DB" $ExternalPostgresDb ".\docker\.env"
  Write-Host "Updated EXTERNAL_POSTGRES_DB in docker/.env."
}
if ($ExternalPostgresUser) {
  Set-EnvValue "EXTERNAL_POSTGRES_USER" $ExternalPostgresUser ".\docker\.env"
  Write-Host "Updated EXTERNAL_POSTGRES_USER in docker/.env."
}
if ($ExternalPostgresPassword) {
  Set-EnvValue "EXTERNAL_POSTGRES_PASSWORD" $ExternalPostgresPassword ".\docker\.env"
  Write-Host "Updated EXTERNAL_POSTGRES_PASSWORD in docker/.env."
}
if ($ExternalPostgresSslmode) {
  $ExternalPostgresSslmode = Convert-ToPostgresSslmode "EXTERNAL_POSTGRES_SSLMODE" $ExternalPostgresSslmode
  Set-EnvValue "EXTERNAL_POSTGRES_SSLMODE" $ExternalPostgresSslmode ".\docker\.env"
  Write-Host "Updated EXTERNAL_POSTGRES_SSLMODE in docker/.env."
}
if ($PluginGithubProxyUrl) {
  Set-EnvValue "API_OFFICIAL_PLUGIN_GITHUB_PROXY_URL" $PluginGithubProxyUrl ".\docker\.env"
  Write-Host "Updated API_OFFICIAL_PLUGIN_GITHUB_PROXY_URL in docker/.env."
}
if ($OfficialPluginSignatureRequired) {
  $OfficialPluginSignatureRequired = Convert-ToTrueFalseEnvValue "API_OFFICIAL_PLUGIN_SIGNATURE_REQUIRED" $OfficialPluginSignatureRequired
  Set-EnvValue "API_OFFICIAL_PLUGIN_SIGNATURE_REQUIRED" $OfficialPluginSignatureRequired ".\docker\.env"
  Write-Host "Updated API_OFFICIAL_PLUGIN_SIGNATURE_REQUIRED in docker/.env."
}

if ($ShouldPrompt -and $PromptConfigValues) {
  Write-Host "Configure docker/.env. Press Enter to keep the value shown in brackets."
  Prompt-DatabaseConfiguration
  Prompt-EnvValue "BOOTSTRAP_ROOT_ACCOUNT" "Root account"
  Prompt-EnvValue "BOOTSTRAP_ROOT_PASSWORD" "Root password"
  Prompt-EnvValue "API_PROVIDER_SECRET_MASTER_KEY" "API provider secret master key"
  Prompt-EnvValue "WEB_PORT" "Web port"
  Prompt-TrueFalseEnvValue "API_COOKIE_SECURE" "Use secure session cookies (true/false)"
  Prompt-OfficialPluginGithubProxyUrl
  Prompt-TrueFalseEnvValue "API_OFFICIAL_PLUGIN_SIGNATURE_REQUIRED" "Require official plugin signatures (true/false)"
}

Ensure-ProviderSecretMasterKey
Sync-DatabaseConfiguration ".\docker\.env"

$NewPostgresPassword = Read-EnvValue "POSTGRES_PASSWORD" ".\docker\.env"
$NewBootstrapRootAccount = Read-EnvValue "BOOTSTRAP_ROOT_ACCOUNT" ".\docker\.env"
$NewBootstrapRootPassword = Read-EnvValue "BOOTSTRAP_ROOT_PASSWORD" ".\docker\.env"
$NewProviderSecret = Read-EnvValue "API_PROVIDER_SECRET_MASTER_KEY" ".\docker\.env"
$NewDatabaseMode = Read-EnvValue "DATABASE_MODE" ".\docker\.env"
$PostgresPasswordSyncRequired = $false

if ($NewDatabaseMode -eq "internal" -and (Test-PostgresDataExists)) {
  if ($NewPostgresPassword -and $OldPostgresPassword -ne $NewPostgresPassword) {
    $PostgresPasswordSyncRequired = $true
  }

  if ($OldBootstrapRootAccount -and $OldBootstrapRootAccount -ne $NewBootstrapRootAccount) {
    Write-Host "Warning: BOOTSTRAP_ROOT_ACCOUNT only affects initial bootstrap; existing root users are not renamed automatically."
  }
  if ($OldBootstrapRootPassword -and $OldBootstrapRootPassword -ne $NewBootstrapRootPassword) {
    Write-Host "Warning: BOOTSTRAP_ROOT_PASSWORD only affects initial bootstrap; existing root passwords are not reset automatically."
  }
  if ($OldProviderSecret -and $OldProviderSecret -ne $NewProviderSecret) {
    $Message = "API_PROVIDER_SECRET_MASTER_KEY changed while existing pgdata was found. Existing provider/data-source secrets may become unreadable without a key rotation."
    if ($ShouldPrompt) {
      $ContinueProviderSecretChange = Prompt-YesNo "$Message Continue?" $false
      if (-not $ContinueProviderSecretChange) {
        Set-EnvValue "API_PROVIDER_SECRET_MASTER_KEY" $OldProviderSecret ".\docker\.env"
        Fail "Restored the previous API_PROVIDER_SECRET_MASTER_KEY."
      }
    } else {
      Set-EnvValue "API_PROVIDER_SECRET_MASTER_KEY" $OldProviderSecret ".\docker\.env"
      Fail $Message
    }
  }
}

if ($null -eq $PullImages) {
  if ($ShouldPrompt) {
    if (Test-LocalLatestFlowbaseImages ".\docker\.env") {
      $PullImages = Prompt-YesNo "Local latest Docker images were found. Update Docker images?" $false
    } else {
      $PullImages = Prompt-YesNo "Pull Docker images?" $false
    }
  } else {
    $PullImages = $false
  }
}

if ($null -eq $StartContainers) {
  if ($ShouldPrompt) {
    $StartContainers = Prompt-YesNo "Start 1flowbase now?" $false
  } else {
    $StartContainers = $false
  }
}

if (-not $PostgresPasswordSyncRequired -and -not $PullImages -and -not $StartContainers) {
  Write-Host "Docker files are ready in ./docker."
  Write-Host "No images were pulled and no containers were started."
  if ($NewDatabaseMode -eq "external") {
    Write-Host "To start later, run: cd docker && docker compose -f docker-compose.external-db.yaml pull && docker compose -f docker-compose.external-db.yaml up -d"
  } else {
    Write-Host "To start later, run: cd docker && docker compose pull && docker compose up -d"
  }
  exit 0
}

if (-not (Invoke-NativeQuiet { docker info })) {
  Fail "Docker is installed but the daemon is not reachable. Start Docker and try again."
}

Set-Location ".\docker"

if ($PostgresPasswordSyncRequired) {
  Sync-PostgresPassword $NewPostgresPassword
}

if ($PullImages -or $StartContainers) {
  Assert-FlowbaseImagePlatformSupport
}

if ($PullImages) {
  if ($NewDatabaseMode -eq "external") {
    Invoke-ComposeCommand @("-f", "docker-compose.external-db.yaml", "pull")
  } else {
    Invoke-ComposeCommand @("pull")
  }
} else {
  Write-Host "Skipping image pull."
}

if ($StartContainers) {
  if ($NewDatabaseMode -eq "external") {
    Invoke-ComposeCommand @("-f", "docker-compose.external-db.yaml", "up", "-d")
  } else {
    Invoke-ComposeCommand @("up", "-d")
  }
} else {
  Write-Host "Skipping container startup."
  if ($NewDatabaseMode -eq "external") {
    Write-Host "To start later, run: cd docker && docker compose -f docker-compose.external-db.yaml up -d"
  } else {
    Write-Host "To start later, run: cd docker && docker compose up -d"
  }
  exit 0
}

$WebPort = Read-EnvValue "WEB_PORT" ".\.env"
$RootAccount = Read-EnvValue "BOOTSTRAP_ROOT_ACCOUNT" ".\.env"
$RootPassword = Read-EnvValue "BOOTSTRAP_ROOT_PASSWORD" ".\.env"

if (-not $WebPort) { $WebPort = "3100" }
if (-not $RootAccount) { $RootAccount = "root" }
if (-not $RootPassword) { $RootPassword = "1flowbase" }

Write-Host "1flowbase is starting. Web: http://127.0.0.1:$WebPort"
Write-Host "Initial root account: $RootAccount"
Write-Host "Initial root password: $RootPassword"
