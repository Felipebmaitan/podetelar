function Assert-FreedomStreamRoot([string]$Root) {
    $full = [IO.Path]::GetFullPath($Root).TrimEnd('\', '/')
    if ($full -eq [IO.Path]::GetPathRoot($full).TrimEnd('\') -or $full -in @($env:USERPROFILE, $env:LOCALAPPDATA, $env:WINDIR) -or $full.Length -lt 12) { throw 'Refusing an unsafe installation directory.' }
    $marker = Join-Path $full '.freedomstream-install'
    if (-not (Test-Path -LiteralPath $marker -PathType Leaf) -or (Get-Content -LiteralPath $marker -Raw).Trim() -ne 'FreedomStream per-user installation v1') { throw 'This directory is not a FreedomStream installation.' }
    $node = Get-Item -LiteralPath $full
    while ($null -ne $node) {
        if (($node.Attributes -band [IO.FileAttributes]::ReparsePoint) -ne 0) { throw 'Installation paths must not contain junctions or symbolic links.' }
        $node = $node.Parent
    }
    if (Get-ChildItem -LiteralPath $full -Force -Recurse | Where-Object { ($_.Attributes -band [IO.FileAttributes]::ReparsePoint) -ne 0 } | Select-Object -First 1) { throw 'Installation contains a junction or symbolic link; inspect it manually.' }
}
