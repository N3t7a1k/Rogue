# 🥷 Rogue

**Rogue** is a anti-forensics and artifact wiping tool written in Rust. It is designed to surgically manipulate system timestamps and sanitize USB connection history without leaving traces, utilizing advanced Windows internals manipulation.

---

## ✨ Key Features

* **Surgical Timestomping**: Modifies **Created**, **Accessed**, and **Modified** (MACE) timestamps with millisecond precision.
* **Directory Support**: Unlike standard tools, Rogue uses `FILE_FLAG_BACKUP_SEMANTICS` to manipulate timestamps of **directories** themselves, not just files.
* **USB Artifact Wiping**: Locates and removes USB storage history from the Registry and driver logs while preserving non-target entries.
* **Privilege Escalation**: Automatically registers temporary tasks to execute logic with `NT AUTHORITY\SYSTEM` privileges, bypassing standard file locks.

---

## 🎯 Targeted Artifacts

| Artifact | Purpose | Action Taken |
| --- | --- | --- |
| **$STANDARD_INFO** | File MACE Timestamps | Modified via API (Process Isolated) |
| **Directory Metadata** | Folder Timestamps | Modified via Backup Semantics |
| **USBSTOR** | Registry Device History | Key Deletion (`HKLM\...`) |
| **MountedDevices** | Drive Letter Mapping | Value Deletion |
| **DeviceClasses** | Device Interface GUIDs | Key Deletion |
| **setupapi.dev.log** | Driver Installation Log | Block-level Text Sanitization |

---

## 🛠 Installation

### Prerequisites

* **OS**: Windows 10 / 11 / Server
* **Privileges**: Administrator access is required for **USB artifact wiping** and modifying **system-protected files**. Standard user rights are sufficient for basic file timestomping.

### Build

```bash
git clone https://github.com/N3t7a1k/Rogue.git
cd Rogue
cargo build --release

```

---

## 💻 Usage

### 1. Timestomping (`time`)

Rogue supports absolute paths, relative paths, and wildcard patterns.

**Get Timestamps**

```powershell
# Get timestamps for all files in current folder
rogue time get "*"

# Get timestamps for a specific directory
rogue time get "C:\Users\Target\Documents"

```

**Set Timestamps**
Syntax: `rogue time set <subcommand> <pattern> <timestamp>`

```powershell
# Modify ALL timestamps (C/A/M) for a file
rogue time set all "C:\Secret\payload.exe" "2024-01-01 09:00:00"

# Modify a FOLDER's timestamp (Directory support)
rogue time set all "C:\Users\Public\Logs" "2023-12-25 12:00:00"

# Modify only ACCESSED time (Triggers Process Isolation)
rogue time set accessed "target.doc" "2025-02-09 15:00:00"

# Bulk update via Wildcards
rogue time set created "*.log" "2022-01-01 00:00:00"

```

### 2. USB Wiping (`usb`)

**List Devices**

```powershell
rogue usb list

```

**Delete Artifacts**

```powershell
# Delete by Serial Number (supports wildcards)
rogue usb delete serial "0000-1234*"

# Delete by Friendly Name
rogue usb delete name "SanDisk Ultra*"

```

> **Note**: Operations involving `Accessed` time or `Registry` deletion will automatically spawn a background task to ensure OS handles are flushed and permissions are bypassed.

---

## 🗺️ Roadmap & Features

### 🕒 Timestomping & Metadata

> Manipulation of file system time attributes to hide activity timelines.

* [x] **File MACE Stomping**: Millisecond-level manipulation of Created, Accessed, Modified timestamps.
* [x] **Directory Stomping**: Support for modifying directory entry timestamps using `FILE_FLAG_BACKUP_SEMANTICS`.
* [ ] **Advanced MFT Stomping**: Manipulation of the `$FILE_NAME` attribute via rename/restore techniques to match `$STANDARD_INFORMATION`.

### 🔌 Device & Peripheral Artifacts

> Cleaning traces of external devices connected to the system.

* [x] **USB Registry Wipe**: Surgical deletion of `USBSTOR`, `MountedDevices`, and `DeviceClasses` keys.
* [x] **Log Sanitization**: Parsing and removing specific device blocks from `setupapi.dev.log`.
* [ ] **Modern Device Wiping**:
* Cleaning `Enum\SCSI` for high-speed external SSDs (UASP protocol).
* Cleaning `Windows Portable Devices` registry keys for MTP devices (Smartphones, Cameras).



### 🏃 Execution & Access Traces

> Removing evidence of program execution and file access history.

* [ ] **Prefetch Wiping**: Parsing and deletion of `.pf` files to remove application execution history.
* [ ] **Shellbags Cleaning**: Removing folder access history and window size preferences from `UserClass.dat`.
* [ ] **Amcache/Shimcache Flushing**: Removal of application compatibility traces that persist after file deletion.

### 📜 System Logs & Journals

> Clearing OS-level recording mechanisms.

* [ ] **Event Log Cleaner**: Selective deletion of specific Event IDs (e.g., 4624, 4625) without clearing the entire log file.
* [ ] **USN Journal Wiping**: Clearing NTFS change journals to hide recent file system modifications.

---

## ⚖ License

Distributed under the MIT License.

> **⚠️ DISCLAIMER**: This tool is developed for educational purposes and authorized red teaming engagements only. The author is not responsible for any misuse or damage caused by this software. Use it at your own risk.
