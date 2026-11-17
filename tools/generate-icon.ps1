param([Parameter(Mandatory=$true)][string]$OutputPath)

Add-Type -AssemblyName System.Drawing
$sizes = 16,24,32,48,64,128,256
$parts = [System.Collections.Generic.List[byte[]]]::new()
foreach ($size in $sizes) {
  $bitmap = [System.Drawing.Bitmap]::new($size,$size)
  $graphics = [System.Drawing.Graphics]::FromImage($bitmap)
  $graphics.SmoothingMode = [System.Drawing.Drawing2D.SmoothingMode]::AntiAlias
  $graphics.Clear([System.Drawing.Color]::FromArgb(242,246,252))
  $blue = [System.Drawing.Color]::FromArgb(54,118,232)
  $gray = [System.Drawing.Color]::FromArgb(76,91,112)
  $center = $size / 2.0; $scale = $size / 256.0
  $graphics.DrawEllipse([System.Drawing.Pen]::new($gray, [Math]::Max(1,$size/18)), $size*.10, $size*.10, $size*.80, $size*.80)
  $graphics.FillRectangle([System.Drawing.SolidBrush]::new($blue), $size*.31, $size*.24, $size*.38, $size*.52)
  $graphics.FillRectangle([System.Drawing.SolidBrush]::new([System.Drawing.Color]::White), $size*.40, $size*.36, $size*.20, $size*.30)
  $stream = [IO.MemoryStream]::new(); $bitmap.Save($stream, [System.Drawing.Imaging.ImageFormat]::Png); $parts.Add($stream.ToArray())
  $stream.Dispose(); $graphics.Dispose(); $bitmap.Dispose()
}
$output = [IO.File]::Open($OutputPath,[IO.FileMode]::Create,[IO.FileAccess]::Write)
$writer = [IO.BinaryWriter]::new($output); $writer.Write([UInt16]0); $writer.Write([UInt16]1); $writer.Write([UInt16]$sizes.Count)
$offset = 6 + 16 * $sizes.Count
for($i=0;$i -lt $sizes.Count;$i++) { $size=$sizes[$i]; $writer.Write([byte]($size % 256)); $writer.Write([byte]($size % 256)); $writer.Write([byte]0); $writer.Write([byte]0); $writer.Write([UInt16]1); $writer.Write([UInt16]32); $writer.Write([UInt32]$parts[$i].Length); $writer.Write([UInt32]$offset); $offset += $parts[$i].Length }
foreach($part in $parts) { $writer.Write($part) }
$writer.Dispose(); $output.Dispose()
