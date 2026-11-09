# Installing Hat Timelapse

Hat Timelapse is a command-line program. It needs three companion tools:

| Tool | Needed for | Required? |
| --- | --- | --- |
| **LightCraft** (`lightcraft-cli`) | Developing raw frames | Yes, unless you develop in Lightroom |
| **ffmpeg** | Encoding video | Yes, for video output |
| **exiftool** | Reading shutter, aperture and ISO from CR3, RAF and other newer raws | Recommended |

Once everything is installed, `hat_timelapse doctor` checks what it can find.

---

## macOS

### 1. Install Hat Timelapse

Open **Terminal** (Applications → Utilities) and run:

```bash
curl -fsSL https://raw.githubusercontent.com/neilmanfredit/hat_timelapse/main/install.sh | sh
```

This puts `hat_timelapse` in `~/bin`. If it says `~/bin` is not on your PATH,
run the line it prints, then open a new Terminal window.

<details>
<summary>Installing by hand instead</summary>

1. From the [latest release](https://github.com/neilmanfredit/hat_timelapse/releases/latest),
   download `hat_timelapse-<version>-macos-universal.tar.gz`. It runs on Apple
   silicon and Intel Macs.
2. In Terminal:

   ```bash
   mkdir -p ~/bin && tar -xzf ~/Downloads/hat_timelapse-*-macos-universal.tar.gz -C ~/Downloads && cp ~/Downloads/hat_timelapse-*-macos-universal/hat_timelapse ~/bin/ && chmod +x ~/bin/hat_timelapse && xattr -d com.apple.quarantine ~/bin/hat_timelapse 2>/dev/null; echo done
   ```

3. If you have not already, add `~/bin` to your PATH:

   ```bash
   echo 'export PATH="$HOME/bin:$PATH"' >> ~/.zshrc && source ~/.zshrc
   ```
</details>

### 2. Install the LightCraft command-line tool

The LightCraft app from the `.dmg` does **not** include the command-line
tool. It is a separate download from the same release.

1. In LightCraft, open **LightCraft → About LightCraft** and note the version.
2. On the [LightCraft releases page](https://github.com/storytold/lightcraft/releases),
   open that version. Under **Assets**, click **Show all assets** and download
   `lightcraft-cli-<version>-macos-universal.zip`.
3. Install it. This copies the program itself, not the folder it comes in:

   ```bash
   mkdir -p ~/bin && unzip -o ~/Downloads/lightcraft-cli-*-macos-universal.zip -d ~/Downloads/lccli && cp "$(find ~/Downloads/lccli -name lightcraft-cli -type f | head -1)" ~/bin/lightcraft-cli && chmod +x ~/bin/lightcraft-cli && rehash
   ```

4. Check it: `lightcraft-cli --version`.

If the release has no command-line download, build it from source. This needs
Rust; see *Building from source* below.

```bash
git clone --depth 1 --branch v0.20.0 https://github.com/storytold/lightcraft ~/src/lightcraft && cd ~/src/lightcraft && CARGO_BUILD_JOBS=2 cargo build --release -p lightcraft-cli && cp target/release/lightcraft-cli ~/bin/
```

Replace `v0.20.0` with your LightCraft version.

### 3. Install ffmpeg and exiftool

These come from [Homebrew](https://brew.sh). If `brew --version` says
"command not found", install Homebrew first:

```bash
/bin/bash -c "$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh)"
```

Then:

```bash
brew install ffmpeg exiftool
```

### 4. Check everything

```bash
hat_timelapse doctor
```

Every line should say `ok`. If AV1 shows as missing, the other formats still
work.

---

## Linux

### 1. Install Hat Timelapse

```bash
curl -fsSL https://raw.githubusercontent.com/neilmanfredit/hat_timelapse/main/install.sh | sh
```

Builds are provided for x86_64 and aarch64.

### 2. Install LightCraft

LightCraft's Linux packages (`.deb`, `.rpm`, AppImage, Flatpak, tarball)
include `lightcraft-cli`. For the AppImage and tarball, put `lightcraft-cli` on
your PATH (for example in `~/bin`). On Arch, the AUR package provides both.

### 3. Install ffmpeg and exiftool

| Distribution | Command |
| --- | --- |
| Debian / Ubuntu / Proxmox | `sudo apt install ffmpeg libimage-exiftool-perl` |
| Fedora | `sudo dnf install ffmpeg perl-Image-ExifTool` (ffmpeg comes from RPM Fusion) |
| Arch | `sudo pacman -S ffmpeg perl-image-exiftool` |

### 4. Check everything

```bash
hat_timelapse doctor
```

---

## Windows

1. From the [latest release](https://github.com/neilmanfredit/hat_timelapse/releases/latest),
   download `hat_timelapse-<version>-windows-x86_64.zip` and unzip it.
2. Create a folder for command-line tools, such as `C:\Tools`, and copy
   `hat_timelapse.exe` into it.
3. Add that folder to your PATH. Search the Start menu for **Edit environment
   variables for your account**, select **Path**, click **Edit → New**, and
   enter `C:\Tools`.
4. Install LightCraft from its `.msi`, and put `lightcraft-cli.exe` in the
   same folder if the installer did not add it to your PATH.
5. Install ffmpeg and exiftool. With winget, in PowerShell:

   ```powershell
   winget install Gyan.FFmpeg OliverBetz.ExifTool
   ```

6. Open a **new** PowerShell window and run `hat_timelapse doctor`.

---

## Building from source

This works on any platform with [Rust](https://rustup.rs) 1.88 or newer.

```bash
git clone https://github.com/neilmanfredit/hat_timelapse ~/src/hat_timelapse && cd ~/src/hat_timelapse && cargo build --release
```

The program is `target/release/hat_timelapse`. Copy it to `~/bin`, or anywhere
on your PATH.

---

## Updating and removing

- **Update:** run the install command again. It replaces the program with the
  latest release.
- **Remove:** delete `~/bin/hat_timelapse` (or `hat_timelapse.exe` on Windows).
  Projects keep their `hat_timelapse.json` and `.hat_timelapse` folder. Delete
  those too if you no longer need them.

## Troubleshooting

| Message | Fix |
| --- | --- |
| `command not found: hat_timelapse` | `~/bin` is not on your PATH. Run `echo 'export PATH="$HOME/bin:$PATH"' >> ~/.zshrc`, then open a new Terminal window. |
| `permission denied` | The file is not marked as a program, or you copied a folder rather than the file inside it. Check with `ls -la ~/bin`: a line starting with `d` is a folder. Copy the file from inside it, then run `chmod +x ~/bin/hat_timelapse`. |
| macOS: "cannot be opened because the developer cannot be verified" | Run `xattr -d com.apple.quarantine ~/bin/hat_timelapse`. |
| `could not run lightcraft-cli` | The LightCraft command-line tool is not installed or not on your PATH. See step 2 for your system. |
| `ffmpeg … was built without the libx265 encoder` | Install ffmpeg from Homebrew, your distribution or winget, as above. Those builds include every encoder Hat Timelapse uses. |
