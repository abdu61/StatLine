# Generates the Statline icon and MSIX tile assets from one vector drawing.
# Usage: powershell -ExecutionPolicy Bypass -File .\scripts\make-icon.ps1
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Drawing

$root = Split-Path -Parent $PSScriptRoot
$assetDir = Join-Path $root 'packaging\msix\Assets'
New-Item -ItemType Directory -Force $assetDir | Out-Null

function Add-RoundedRect {
    param($Path, [float]$X, [float]$Y, [float]$W, [float]$H, [float]$R)
    $d = $R * 2
    $Path.AddArc($X, $Y, $d, $d, 180, 90)
    $Path.AddArc($X + $W - $d, $Y, $d, $d, 270, 90)
    $Path.AddArc($X + $W - $d, $Y + $H - $d, $d, $d, 0, 90)
    $Path.AddArc($X, $Y + $H - $d, $d, $d, 90, 90)
    $Path.CloseFigure()
}

function Draw-Mark {
    param($Graphics, [float]$Size, [float]$Inset = 0.0)
    $Graphics.SmoothingMode = [System.Drawing.Drawing2D.SmoothingMode]::AntiAlias
    $Graphics.PixelOffsetMode = [System.Drawing.Drawing2D.PixelOffsetMode]::HighQuality
    $pad = [math]::Max(1.0, $Size * 0.06 + $Inset * $Size)
    $w = $Size - 2 * $pad
    $radius = $Size * 0.22

    $path = New-Object System.Drawing.Drawing2D.GraphicsPath
    Add-RoundedRect $path $pad $pad $w $w $radius
    $bg = New-Object System.Drawing.SolidBrush ([System.Drawing.Color]::FromArgb(255, 19, 23, 28))
    $Graphics.FillPath($bg, $path)
    $border = New-Object System.Drawing.Pen ([System.Drawing.Color]::FromArgb(255, 44, 56, 68)), ([math]::Max(1.0, $Size * 0.016))
    $Graphics.DrawPath($border, $path)

    $accent = [System.Drawing.Color]::FromArgb(255, 76, 194, 255)
    $pen = New-Object System.Drawing.Pen $accent, ([math]::Max(1.4, $Size * 0.052))
    $pen.StartCap = [System.Drawing.Drawing2D.LineCap]::Round
    $pen.EndCap = [System.Drawing.Drawing2D.LineCap]::Round
    $pen.LineJoin = [System.Drawing.Drawing2D.LineJoin]::Round

    $points = @(
        (New-Object System.Drawing.PointF (($pad + $w * 0.16), ($pad + $w * 0.54))),
        (New-Object System.Drawing.PointF (($pad + $w * 0.35), ($pad + $w * 0.54))),
        (New-Object System.Drawing.PointF (($pad + $w * 0.45), ($pad + $w * 0.28))),
        (New-Object System.Drawing.PointF (($pad + $w * 0.57), ($pad + $w * 0.73))),
        (New-Object System.Drawing.PointF (($pad + $w * 0.67), ($pad + $w * 0.43))),
        (New-Object System.Drawing.PointF (($pad + $w * 0.84), ($pad + $w * 0.43)))
    )
    $Graphics.DrawLines($pen, $points)

    $dot = New-Object System.Drawing.SolidBrush $accent
    $dotSize = [math]::Max(1.0, $Size * 0.045)
    $Graphics.FillEllipse($dot, ($pad + $w * 0.84 - $dotSize / 2), ($pad + $w * 0.43 - $dotSize / 2), $dotSize, $dotSize)

    $pen.Dispose(); $border.Dispose(); $bg.Dispose(); $dot.Dispose(); $path.Dispose()
}

function New-IconPng {
    param([int]$Size)
    $bmp = New-Object System.Drawing.Bitmap $Size, $Size
    $g = [System.Drawing.Graphics]::FromImage($bmp)
    $g.Clear([System.Drawing.Color]::Transparent)
    Draw-Mark $g $Size
    $g.Dispose()
    $ms = New-Object System.IO.MemoryStream
    $bmp.Save($ms, [System.Drawing.Imaging.ImageFormat]::Png)
    $bmp.Dispose()
    return $ms.ToArray()
}

# Multi-size ICO (PNG entries, Vista+).
$sizes = @(16, 20, 24, 32, 40, 48, 64, 96, 128, 256)
$images = @()
foreach ($s in $sizes) { $images += ,@($s, (New-IconPng $s)) }

$ico = New-Object System.IO.MemoryStream
$writer = New-Object System.IO.BinaryWriter $ico
$writer.Write([UInt16]0); $writer.Write([UInt16]1); $writer.Write([UInt16]$sizes.Count)
$offset = 6 + 16 * $sizes.Count
for ($i = 0; $i -lt $sizes.Count; $i++) {
    $size = $images[$i][0]; $bytes = $images[$i][1]
    $writer.Write([Byte]$(if ($size -ge 256) { 0 } else { $size }))
    $writer.Write([Byte]$(if ($size -ge 256) { 0 } else { $size }))
    $writer.Write([Byte]0); $writer.Write([Byte]0)
    $writer.Write([UInt16]1); $writer.Write([UInt16]32)
    $writer.Write([UInt32]$bytes.Length); $writer.Write([UInt32]$offset)
    $offset += $bytes.Length
}
foreach ($image in $images) { $writer.Write([Byte[]]$image[1]) }
$writer.Flush()
[System.IO.File]::WriteAllBytes((Join-Path $root 'packaging\statline.ico'), $ico.ToArray())
$ico.Dispose()

# MSIX tiles and a README preview.
function Save-Tile {
    param([int]$Width, [int]$Height, [string]$Name, [double]$Fill = 0.8)
    $bmp = New-Object System.Drawing.Bitmap $Width, $Height
    $g = [System.Drawing.Graphics]::FromImage($bmp)
    $g.Clear([System.Drawing.Color]::Transparent)
    $size = [math]::Min($Width, $Height) * $Fill
    $x = ($Width - $size) / 2; $y = ($Height - $size) / 2
    $g.TranslateTransform($x, $y)
    Draw-Mark $g $size
    $g.ResetTransform(); $g.Dispose()
    $bmp.Save((Join-Path $assetDir $Name), [System.Drawing.Imaging.ImageFormat]::Png)
    $bmp.Dispose()
}

Save-Tile 44 44 'Square44x44Logo.png' 0.92
Save-Tile 150 150 'Square150x150Logo.png' 0.86
Save-Tile 50 50 'StoreLogo.png' 0.92
Save-Tile 310 150 'Wide310x150Logo.png' 0.82
Save-Tile 620 300 'SplashScreen.png' 0.62

# 256 px preview for the README / release notes.
$preview = New-Object System.Drawing.Bitmap 256, 256
$g = [System.Drawing.Graphics]::FromImage($preview)
$g.Clear([System.Drawing.Color]::Transparent)
Draw-Mark $g 256
$g.Dispose()
$preview.Save((Join-Path $root 'docs\images\icon.png'), [System.Drawing.Imaging.ImageFormat]::Png)
$preview.Dispose()

Write-Output "icon: packaging\statline.ico"
Write-Output "tiles refreshed in packaging\msix\Assets"
