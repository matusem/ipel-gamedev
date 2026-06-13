$ErrorActionPreference = 'Stop'

$packageArgs = @{
  packageName   = $env:ChocolateyPackageName
  unzipLocation = "$(Split-Path -Parent $MyInvocation.MyCommand.Definition)"
  fileType      = 'zip'
  url           = 'https://github.com/matusem/ipel-gamedev/releases/download/gamedev-cli-v0.1.0/gamedev-cli-windows-x86_64.zip'
  checksum      = 'REPLACE_WITH_SHA256'
  checksumType  = 'sha256'
}

Install-ChocolateyZipPackage @packageArgs

Install-BinFile -Name 'gamedev-cli' -Path (Join-Path $packageArgs.unzipLocation 'tools\gamedev.exe')
