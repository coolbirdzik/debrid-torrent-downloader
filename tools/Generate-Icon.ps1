# Rebuild the Windows icon from the same vector geometry as Assets/AppIcon.svg.
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName PresentationCore, WindowsBase
$assetDirectory = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..\src\RdBatch.App\Assets'))
[IO.Directory]::CreateDirectory($assetDirectory) | Out-Null
function Render-Icon([int]$size) {
    $visual = [Windows.Media.DrawingVisual]::new()
    $drawing = $visual.RenderOpen()
    $drawing.PushTransform([Windows.Media.ScaleTransform]::new($size / 256.0, $size / 256.0))
    $brushes = [Windows.Media.BrushConverter]::new()
    $drawing.DrawRoundedRectangle($brushes.ConvertFromString('#3159D8'), $null, [Windows.Rect]::new(8,8,240,240), 56, 56)
    $drawing.DrawGeometry([Windows.Media.Brushes]::White, $null, [Windows.Media.Geometry]::Parse('M111,52 H145 V115 H178 L128,165 L78,115 H111 Z'))
    $drawing.DrawRoundedRectangle([Windows.Media.Brushes]::White, $null, [Windows.Rect]::new(60,181,136,18), 9, 9)
    $drawing.DrawRoundedRectangle($brushes.ConvertFromString('#BFCEFF'), $null, [Windows.Rect]::new(82,212,92,10), 5, 5)
    $drawing.Pop()
    $drawing.Close()
    $bitmap = [Windows.Media.Imaging.RenderTargetBitmap]::new($size,$size,96,96,[Windows.Media.PixelFormats]::Pbgra32)
    $bitmap.Render($visual)
    $encoder = [Windows.Media.Imaging.PngBitmapEncoder]::new()
    $encoder.Frames.Add([Windows.Media.Imaging.BitmapFrame]::Create($bitmap))
    $stream = [IO.MemoryStream]::new()
    try { $encoder.Save($stream); return ,$stream.ToArray() } finally { $stream.Dispose() }
}
[IO.File]::WriteAllBytes((Join-Path $assetDirectory 'AppIcon.png'), (Render-Icon 512))
$sizes = @(16,20,24,32,40,48,64,128,256)
$frames = @($sizes | ForEach-Object { ,(Render-Icon $_) })
$output = [IO.File]::Create((Join-Path $assetDirectory 'AppIcon.ico'))
$writer = [IO.BinaryWriter]::new($output)
try {
    $writer.Write([uint16]0); $writer.Write([uint16]1); $writer.Write([uint16]$sizes.Count)
    $offset = 6 + 16 * $sizes.Count
    for ($index = 0; $index -lt $sizes.Count; $index++) {
        $edge = if ($sizes[$index] -eq 256) { 0 } else { $sizes[$index] }
        $writer.Write([byte]$edge); $writer.Write([byte]$edge)
        $writer.Write([byte]0); $writer.Write([byte]0)
        $writer.Write([uint16]1); $writer.Write([uint16]32)
        $writer.Write([uint32]$frames[$index].Length); $writer.Write([uint32]$offset)
        $offset += $frames[$index].Length
    }
    foreach ($frame in $frames) { $writer.Write([byte[]]$frame) }
} finally { $writer.Dispose() }
Write-Host "Generated AppIcon.png and AppIcon.ico in $assetDirectory"
