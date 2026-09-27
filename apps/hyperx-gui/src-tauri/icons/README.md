# Application icon

`icon.png` (RGBA) and `icon.icns` are mechanically generated from
`../../public/mark.svg` using the Tauri CLI; `icon.ico` was rendered from the
same source with ImageMagick. No AI-generated or manufacturer asset is used.

Generate the Tauri icon set in a temporary directory and copy only the desktop
icons needed by this repo (commands from `apps/hyperx-gui`):

```sh
task_icon_dir=$(mktemp -d)
npm run tauri -- icon public/mark.svg --output "$task_icon_dir"
cp "$task_icon_dir/icon.png" src-tauri/icons/icon.png
cp "$task_icon_dir/icon.icns" src-tauri/icons/icon.icns
convert public/mark.svg -define icon:auto-resize=256,128,64,32,16 src-tauri/icons/icon.ico
```

The SVG is the editable source. No third-party HyperX logo or photograph is used.
All three formats are listed in `tauri.conf.json`; non-Windows code generation
also needs the PNG even when packaging is disabled. See
[Tauri icon documentation](https://v2.tauri.app/develop/icons/).
