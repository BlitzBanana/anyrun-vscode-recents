# Anyrun - VSCode Recents
Plugin for anyrun to show recently opened projects with VSCode.

## Configuration
### For VSCode
```
// <Anyrun config dir>/vscode.ron
Config(
  prefix: Some(":code "),
  command: "code",
  icon: "com.visualstudio.code",
  workspace: "~/.config/Code/User/workspaceStorage",
)
```
### For Codium
```
// <Anyrun config dir>/vscode.ron
Config(
  prefix: Some(":codium "),
  command: "codium",
  icon: "vscodium",
  workspace: "~/.config/VSCodium/User/workspaceStorage",
)
```

## Building
```bash
cargo build --release && cp target/release/libvscode_recents.so ~/.config/anyrun/plugins/
```