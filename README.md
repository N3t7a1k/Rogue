# 🥷 Rogue

**Rogue** is an advanced anti-forensics and artifact wiping tool written in Rust. It is designed to surgically manipulate system timestamps, spoof file ownership, and sanitize USB connection history without leaving traces, utilizing low-level Windows API and direct kernel object manipulation.

> **v0.2.0 Update**: Now includes **File Ownership Spoofing** and **SCSI/MTP Device Wiping**.

---

## ✨ Key Features

* **🕵️‍♂️ Owner Spoofing**: Changes file ownership (Security Descriptor) to arbitrary users (e.g., `TrustedInstaller`, `SYSTEM`) to mask the true creator of a file.
* **⏰ Surgical Timestomping**: Modifies **Created**, **Accessed**, and **Modified** (MACE) timestamps with millisecond precision.
* **📂 Directory Support**: Uses `FILE_FLAG_BACKUP_SEMANTICS` to manipulate timestamps and ownership of **directories** themselves, not just files.
* **⚡ Privilege Escalation**: Automatically spawns background tasks with `NT AUTHORITY\SYSTEM` privileges to bypass file locks and modify protected system files (e.g., inside `System32`).
* **🔌 USB Artifact Wiping**: Locates and removes USB storage history from the Registry and driver logs while preserving non-target entries.

---

## 🎯 Targeted Artifacts

| Artifact | Purpose | Action Taken |
| --- | --- | --- |
| **$STANDARD_INFO** | File MACE Timestamps | Modified via `SetFileTime` |
| **Security Descriptor** | File Owner / Creator | Spoofed via `SetSecurityInfo` (Owner/Group) |
| **Directory Metadata** | Folder Attributes | Modified via Backup Semantics |
| **USBSTOR** | Registry Device History | Key Deletion (`HKLM\...`) |
| **MountedDevices** | Drive Letter Mapping | Value Deletion |
| **setupapi.dev.log** | Driver Installation Log | Block-level Text Sanitization |

---

## 🛠 Installation

### Prerequisites

* **OS**: Windows 10 / 11 / Server (x64)
* **Privileges**:
* **Administrator**: Required for **USB wiping**, **Owner Spoofing**, and modifying **System files**.
* **Standard User**: Sufficient for basic file timestomping on user-owned files.



### Build

```bash
git clone https://github.com/N3t7a1k/Rogue.git
cd Rogue
cargo build --release

```

---

## 💻 Usage

Rogue v0.2.0 organizes commands by target module (e.g., `file`, `usb`).

### 1. File Operations (`file`)

Manage file attributes, timestamps, and ownership. Supports wildcards (`*`, `?`) and recursive patterns.

#### 🕰️ Time Stomping (`file time`)

Manipulate file timestamps to hide activity timelines.

```powershell
# Get timestamps for all files in current folder
rogue file time get "*"

# Modify ALL timestamps (C/A/M) for a payload
rogue file time set all "C:\Secret\payload.exe" "2024-01-01 09:00:00"

# Modify only ACCESSED time (Useful for anti-forensics)
rogue file time set accessed "target.doc" "2025-02-09 15:00:00"

# Bulk update logs to look old
rogue file time set created "*.log" "2022-01-01 00:00:00"

```

#### 🕵️‍♂️ Owner Spoofing (`file own`)

Change the file owner to disguise the creator identity.
*Requires Administrator privileges.*

```powershell
# Check current owner
rogue file own get "C:\Windows\System32\drivers\etc\hosts"

# Impersonate SYSTEM (Make it look like a system file)
rogue file own set "payload.dll" "NT AUTHORITY\SYSTEM"

# Impersonate TrustedInstaller (High-value target spoofing)
rogue file own set "C:\Target\backdoor.exe" "NT SERVICE\TrustedInstaller"

# Restore to Administrators
rogue file own set "*.exe" "BUILTIN\Administrators"

```

> **Note**: If Rogue encounters "Access Denied" errors on system files, it attempts to self-elevate to `SYSTEM` via Task Scheduler automatically.

### 2. USB Wiping (`usb`)

Clean traces of external device connections.

```powershell
# List connected/history devices
rogue usb list

# Delete by Serial Number (supports wildcards)
rogue usb delete serial "0000-1234*"

# Delete by Friendly Name
rogue usb delete name "SanDisk Ultra*"

```

---

## 🗺️ Roadmap & Features

### 📁 File System Artifacts

* [x] **MACE Stomping**: Millisecond-level manipulation of Created, Accessed, Modified times.
* [x] **Owner Spoofing**: Changing file ownership (SID) to any system account.
* [x] **Directory Support**: Full support for folder metadata manipulation.
* [ ] **MFT Entry Stomping**: Manipulation of `$FILE_NAME` attribute via rename/restore techniques.

### 🔌 Device Artifacts

* [x] **USB Registry Wipe**: Surgical deletion of `USBSTOR`, `MountedDevices`, etc.
* [x] **Log Sanitization**: Parsing and removing blocks from `setupapi.dev.log`.

### 🛡️ Evasion & Persistence

* [x] **SYSTEM Escalation**: Leveraging Task Scheduler for automated SYSTEM-level execution.
* [ ] **Event Log Cleaning**: Selective deletion of Event ID 4624/4625.

### 👤 User Forensic Artifacts

* [ ] **PowerShell History**: Secure deletion of `ConsoleHost_history.txt` and command history.
* [ ] **Dialog MRU**: Clear Open/Save dialog history (`OpenSavePidlMRU`) and last visited paths.
* [ ] **Notepad State**: Cleanup Windows 11 Notepad session state, cache, and unsaved tabs.

---

## ⚖ License

Distributed under the MIT License.

> **⚠️ DISCLAIMER**: This tool is developed for **educational purposes and authorized red teaming engagements only**. The author is not responsible for any misuse or damage caused by this software. Do not use this on systems you do not own or have explicit permission to test.
