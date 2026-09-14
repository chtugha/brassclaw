# zencoder-auth.ps1 — obtain a Zencoder/Zenflow JWT on Windows.
#
# Usage:
#   .\scripts\zencoder-auth.ps1
#
# After running, copy the JWT from the output and store it:
#   brassclaw secret set zencoder_access_token <jwt>
#
# The token is valid for ~24 hours.  Re-run this script when you see a 401.
#
# Requirements:
#   - PowerShell 5.1+ (ships with Windows 10/11)
#   - ZENCODER_EMAIL and ZENCODER_PASSWORD env vars (optional; script prompts if absent)

$AuthUrl   = "https://auth.zencoder.ai/auth/realms/zencoder/protocol/openid-connect/token"
$ClientId  = "zencoder-public"

# Resolve credentials — env vars take precedence; fall back to interactive prompt.
$Email = $env:ZENCODER_EMAIL
if (-not $Email) {
    $Email = Read-Host "Zencoder email"
}

$PasswordPlain = $env:ZENCODER_PASSWORD
if (-not $PasswordPlain) {
    $SecurePass   = Read-Host "Zencoder password" -AsSecureString
    $BstrPtr      = [System.Runtime.InteropServices.Marshal]::SecureStringToBSTR($SecurePass)
    $PasswordPlain = [System.Runtime.InteropServices.Marshal]::PtrToStringBSTR($BstrPtr)
    [System.Runtime.InteropServices.Marshal]::ZeroFreeBSTR($BstrPtr)
}

$Body = @{
    client_id  = $ClientId
    grant_type = "password"
    username   = $Email
    password   = $PasswordPlain
}

try {
    $Response = Invoke-RestMethod -Uri $AuthUrl -Method Post -Body $Body `
        -ContentType "application/x-www-form-urlencoded"
} catch {
    Write-Error "Request failed: $_"
    exit 1
}

$Token = $Response.access_token
if (-not $Token) {
    Write-Error "Failed to obtain token. Check your credentials."
    exit 1
}

Write-Host ""
Write-Host "=== Zencoder JWT ==="
Write-Host $Token
Write-Host "===================="
Write-Host ""
Write-Host "Run the following command to store it in BrassClaw:"
Write-Host "  brassclaw secret set zencoder_access_token $Token"
