//! Abstracts every bit of I/O a check might need (file reads, running commands,
//! WMI queries, registry reads, MAC/disk enumeration) behind a small trait per
//! platform. This is the *only* place `cfg(target_os = ...)` needs to appear
//! for evidence gathering: individual checks never branch on `cfg!` directly,
//! they just call methods on `Ctx`.

/// The result of shelling out to a command: exit status plus captured output.
#[derive(Debug, Clone, Default)]
pub struct CommandOutput {
    pub success: bool,
    pub stdout: String,
    pub stderr: String,
}

impl CommandOutput {
    /// A successful [`CommandOutput`] with the given `stdout` and empty `stderr`.
    pub fn ok(stdout: impl Into<String>) -> Self {
        Self {
            success: true,
            stdout: stdout.into(),
            stderr: String::new(),
        }
    }
}

/// Whether the current process already has root/administrator rights.
/// Used to skip both the CLI's "run elevated checks?" prompt and the
/// unnecessary `sudo` wrapper in [`linux::SystemEvidence::run_elevated`] when
/// there's no privilege left to escalate to (e.g. launched via `sudo
/// vm-check`). Always `false` on Windows, where no check needs elevation.
#[cfg(unix)]
pub fn running_as_root() -> bool {
    // SAFETY: geteuid() takes no arguments, has no preconditions, and cannot fail.
    unsafe { libc::geteuid() == 0 }
}

#[cfg(not(unix))]
pub fn running_as_root() -> bool {
    false
}

#[cfg(target_os = "linux")]
mod linux {
    use super::CommandOutput;

    /// Everything a Linux check can ask the host for. [`SystemEvidence`] is the
    /// real implementation; `MockLinuxEvidence` (test-only, see the `testing` module)
    /// fakes it so check logic can be tested without real host/VM state.
    pub trait LinuxEvidence {
        /// Reads a file's contents, e.g. from `/proc` or `/sys`.
        fn read_file(&self, path: &str) -> std::io::Result<String>;
        /// Runs `cmd` with `args` as an ordinary (unprivileged) user.
        fn run(&self, cmd: &str, args: &[&str]) -> std::io::Result<CommandOutput>;
        /// Runs `cmd` with `args`, escalating via `sudo` unless already root.
        fn run_elevated(&self, cmd: &str, args: &[&str]) -> std::io::Result<CommandOutput>;
        /// MAC addresses of the host's real network devices (see the
        /// `device` symlink filtering in [`SystemEvidence::network_mac_addresses`]).
        fn network_mac_addresses(&self) -> std::io::Result<Vec<String>>;
        /// Model strings of the host's block devices, from `/sys/block/*/device/model`.
        fn disk_models(&self) -> std::io::Result<Vec<String>>;
    }

    /// Handle passed to a Linux [`Check`](crate::check::Check)'s `run` fn.
    pub type Ctx<'a> = &'a dyn LinuxEvidence;

    /// Real evidence source backed by the actual host: `/proc`, `/sys`, and
    /// subprocesses. Used by the CLI and GUI; tests use `MockLinuxEvidence`
    /// instead so check logic can be exercised without real host/VM state.
    pub struct SystemEvidence;

    impl LinuxEvidence for SystemEvidence {
        fn read_file(&self, path: &str) -> std::io::Result<String> {
            std::fs::read_to_string(path)
        }

        fn run(&self, cmd: &str, args: &[&str]) -> std::io::Result<CommandOutput> {
            let output = std::process::Command::new(cmd).args(args).output()?;
            Ok(CommandOutput {
                success: output.status.success(),
                stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
                stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
            })
        }

        fn run_elevated(&self, cmd: &str, args: &[&str]) -> std::io::Result<CommandOutput> {
            run_elevated_with(super::running_as_root(), cmd, args, |c, a| self.run(c, a))
        }

        fn network_mac_addresses(&self) -> std::io::Result<Vec<String>> {
            collect_mac_addresses(std::path::Path::new("/sys/class/net"))
        }

        fn disk_models(&self) -> std::io::Result<Vec<String>> {
            Ok(collect_disk_models(std::path::Path::new("/sys/block")))
        }
    }

    /// The real traversal behind [`SystemEvidence::network_mac_addresses`],
    /// with the `/sys/class/net` root taken as a parameter so a test can
    /// point it at a throwaway directory built with `std::fs` instead of the
    /// real host's network devices — the empty/all-zero-MAC skip below
    /// depends on real hardware state that can't be relied on to exist (or
    /// not exist) on any given test machine otherwise.
    fn collect_mac_addresses(net_dir: &std::path::Path) -> std::io::Result<Vec<String>> {
        let mut macs = Vec::new();
        for entry in std::fs::read_dir(net_dir)? {
            let entry = entry?;
            // Software constructs the host itself creates (bridges such as
            // virbr*, docker0, lxcbr*; VMware Workstation's host-only/NAT
            // networks like vmnet*; veth pairs; loopback) have no backing
            // "device" symlink. A machine that merely *runs* virtualization software
            // (libvirt, VMware Workstation) as a host must not be flagged as
            // if it *were* a VM guest because of those interfaces' hypervisor
            // OUI prefixes. Physical NICs, and a guest's own virtual NIC as
            // seen from inside that guest, both have a "device" symlink, so
            // this distinguishes host-side virtual networking from a real
            // guest identity.
            if !entry.path().join("device").exists() {
                continue;
            }
            let addr_path = entry.path().join("address");
            if let Ok(addr) = std::fs::read_to_string(&addr_path) {
                let addr = addr.trim();
                if !addr.is_empty() && addr != "00:00:00:00:00:00" {
                    macs.push(addr.to_string());
                }
            }
        }
        Ok(macs)
    }

    /// The real traversal behind [`SystemEvidence::disk_models`]; see
    /// [`collect_mac_addresses`] for why the root directory is a parameter.
    /// Silently returns no models when `block_dir` itself is missing/unreadable
    /// (matches the original behavior, hence returning `Vec` rather than a
    /// `Result` callers would have to needlessly unwrap).
    fn collect_disk_models(block_dir: &std::path::Path) -> Vec<String> {
        let mut models = Vec::new();
        if let Ok(entries) = std::fs::read_dir(block_dir) {
            for entry in entries.flatten() {
                let model_path = entry.path().join("device").join("model");
                if let Ok(model) = std::fs::read_to_string(&model_path) {
                    let model = model.trim();
                    if !model.is_empty() {
                        models.push(model.to_string());
                    }
                }
            }
        }
        models
    }

    /// Decision logic for [`LinuxEvidence::run_elevated`], with the root check
    /// and the "already elevated" run path injected so both branches are
    /// exercisable in tests without needing real root or a controllable `sudo`.
    fn run_elevated_with(
        is_root: bool,
        cmd: &str,
        args: &[&str],
        run: impl FnOnce(&str, &[&str]) -> std::io::Result<CommandOutput>,
    ) -> std::io::Result<CommandOutput> {
        if is_root {
            return run(cmd, args);
        }
        let mut full_args = vec![cmd];
        full_args.extend_from_slice(args);
        let output = std::process::Command::new("sudo")
            .args(&full_args)
            .output()?;
        Ok(CommandOutput {
            success: output.status.success(),
            stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        })
    }

    #[cfg(test)]
    pub mod testing {
        use super::{CommandOutput, LinuxEvidence};
        use std::collections::HashMap;

        #[derive(Default)]
        pub struct MockLinuxEvidence {
            files: HashMap<String, String>,
            commands: HashMap<String, CommandOutput>,
            elevated_commands: HashMap<String, CommandOutput>,
            macs: Vec<String>,
            macs_err: bool,
            disk_models: Vec<String>,
            disk_models_err: bool,
        }

        fn command_key(cmd: &str, args: &[&str]) -> String {
            format!("{cmd} {}", args.join(" "))
        }

        impl MockLinuxEvidence {
            pub fn new() -> Self {
                Self::default()
            }

            pub fn with_file(mut self, path: &str, content: &str) -> Self {
                self.files.insert(path.to_string(), content.to_string());
                self
            }

            pub fn with_command(mut self, cmd: &str, args: &[&str], output: CommandOutput) -> Self {
                self.commands.insert(command_key(cmd, args), output);
                self
            }

            pub fn with_elevated_command(
                mut self,
                cmd: &str,
                args: &[&str],
                output: CommandOutput,
            ) -> Self {
                self.elevated_commands
                    .insert(command_key(cmd, args), output);
                self
            }

            pub fn with_mac(mut self, mac: &str) -> Self {
                self.macs.push(mac.to_string());
                self
            }

            pub fn with_disk_model(mut self, model: &str) -> Self {
                self.disk_models.push(model.to_string());
                self
            }

            pub fn with_mac_addresses_error(mut self) -> Self {
                self.macs_err = true;
                self
            }

            pub fn with_disk_models_error(mut self) -> Self {
                self.disk_models_err = true;
                self
            }
        }

        impl LinuxEvidence for MockLinuxEvidence {
            fn read_file(&self, path: &str) -> std::io::Result<String> {
                self.files.get(path).cloned().ok_or_else(|| {
                    std::io::Error::new(std::io::ErrorKind::NotFound, path.to_string())
                })
            }

            fn run(&self, cmd: &str, args: &[&str]) -> std::io::Result<CommandOutput> {
                self.commands
                    .get(&command_key(cmd, args))
                    .cloned()
                    .ok_or_else(|| {
                        std::io::Error::new(std::io::ErrorKind::NotFound, cmd.to_string())
                    })
            }

            fn run_elevated(&self, cmd: &str, args: &[&str]) -> std::io::Result<CommandOutput> {
                self.elevated_commands
                    .get(&command_key(cmd, args))
                    .cloned()
                    .ok_or_else(|| {
                        std::io::Error::new(std::io::ErrorKind::NotFound, cmd.to_string())
                    })
            }

            fn network_mac_addresses(&self) -> std::io::Result<Vec<String>> {
                if self.macs_err {
                    return Err(std::io::Error::other("network enumeration failed"));
                }
                Ok(self.macs.clone())
            }

            fn disk_models(&self) -> std::io::Result<Vec<String>> {
                if self.disk_models_err {
                    return Err(std::io::Error::other("block device enumeration failed"));
                }
                Ok(self.disk_models.clone())
            }
        }
    }

    #[cfg(test)]
    mod system_evidence_tests {
        use super::{
            collect_disk_models, collect_mac_addresses, run_elevated_with, CommandOutput,
            LinuxEvidence, SystemEvidence,
        };

        /// A fresh, empty scratch directory under the OS temp dir, removed on
        /// drop, so each test builds its own throwaway `/sys`-shaped tree
        /// instead of depending on (or risking damage to) the real one.
        struct TempDir(std::path::PathBuf);

        impl TempDir {
            fn new(name: &str) -> Self {
                let path = std::env::temp_dir().join(format!(
                    "vm-check-test-{name}-{}-{:?}",
                    std::process::id(),
                    std::thread::current().id()
                ));
                std::fs::create_dir_all(&path).unwrap();
                Self(path)
            }

            fn path(&self) -> &std::path::Path {
                &self.0
            }
        }

        impl Drop for TempDir {
            fn drop(&mut self) {
                std::fs::remove_dir_all(&self.0).ok();
            }
        }

        #[test]
        fn read_file_reads_real_file() {
            let path = std::env::temp_dir().join(format!("vm-check-test-{}", std::process::id()));
            std::fs::write(&path, "hello").unwrap();
            let result = SystemEvidence.read_file(path.to_str().unwrap());
            std::fs::remove_file(&path).ok();
            assert_eq!(result.unwrap(), "hello");
        }

        #[test]
        fn read_file_missing_returns_err() {
            assert!(SystemEvidence
                .read_file("/nonexistent/vm-check-test-path")
                .is_err());
        }

        #[test]
        fn run_executes_real_command() {
            let result = SystemEvidence.run("echo", &["hi"]).unwrap();
            assert!(result.success);
            assert_eq!(result.stdout.trim(), "hi");
        }

        #[test]
        fn run_missing_binary_returns_err() {
            assert!(SystemEvidence
                .run("vm-check-definitely-not-a-real-binary", &[])
                .is_err());
        }

        #[test]
        fn network_mac_addresses_does_not_error() {
            assert!(SystemEvidence.network_mac_addresses().is_ok());
        }

        #[test]
        fn disk_models_does_not_error() {
            assert!(SystemEvidence.disk_models().is_ok());
        }

        #[test]
        fn collect_mac_addresses_skips_empty_and_all_zero_addresses() {
            let dir = TempDir::new("net");

            let real = dir.path().join("eth0");
            std::fs::create_dir_all(real.join("device")).unwrap();
            std::fs::write(real.join("address"), "aa:bb:cc:dd:ee:ff\n").unwrap();

            let zero = dir.path().join("eth1");
            std::fs::create_dir_all(zero.join("device")).unwrap();
            std::fs::write(zero.join("address"), "00:00:00:00:00:00\n").unwrap();

            let empty = dir.path().join("eth2");
            std::fs::create_dir_all(empty.join("device")).unwrap();
            std::fs::write(empty.join("address"), "\n").unwrap();

            let host_only = dir.path().join("virbr0");
            std::fs::create_dir_all(&host_only).unwrap();
            std::fs::write(host_only.join("address"), "52:54:00:aa:bb:cc\n").unwrap();

            // Has a "device" symlink but no readable `address` file at all
            // (e.g. removed between listing the directory and reading it) —
            // must be skipped rather than erroring the whole collection.
            let unreadable = dir.path().join("eth3");
            std::fs::create_dir_all(unreadable.join("device")).unwrap();

            let macs = collect_mac_addresses(dir.path()).unwrap();
            assert_eq!(macs, vec!["aa:bb:cc:dd:ee:ff".to_string()]);
        }

        #[test]
        fn collect_disk_models_skips_empty_model_strings() {
            let dir = TempDir::new("block");

            let real = dir.path().join("sda");
            std::fs::create_dir_all(real.join("device")).unwrap();
            std::fs::write(real.join("device").join("model"), "Samsung SSD\n").unwrap();

            let blank = dir.path().join("sdb");
            std::fs::create_dir_all(blank.join("device")).unwrap();
            std::fs::write(blank.join("device").join("model"), "\n").unwrap();

            let no_model_file = dir.path().join("loop0");
            std::fs::create_dir_all(no_model_file.join("device")).unwrap();

            let models = collect_disk_models(dir.path());
            assert_eq!(models, vec!["Samsung SSD".to_string()]);
        }

        #[test]
        fn collect_disk_models_returns_empty_when_root_is_missing() {
            let missing = std::env::temp_dir().join("vm-check-test-definitely-missing-block-dir");
            assert!(collect_disk_models(&missing).is_empty());
        }

        #[test]
        fn root_check_runs_without_panicking() {
            // CI never runs as root; this just exercises the syscall wrapper.
            let _ = super::super::running_as_root();
        }

        #[test]
        fn run_elevated_skips_sudo_when_already_root() {
            let result = run_elevated_with(true, "echo", &["hi"], |cmd, args| {
                assert_eq!(cmd, "echo");
                assert_eq!(args, ["hi"]);
                Ok(CommandOutput::ok("hi"))
            })
            .unwrap();
            assert_eq!(result.stdout, "hi");
        }

        #[test]
        fn run_elevated_shells_out_to_sudo_when_not_root() {
            // No cached credentials and `output()` closes the child's stdin, so
            // `sudo` fails fast instead of prompting, which exercises the real
            // subprocess path without ever hanging the test suite.
            let result = run_elevated_with(false, "true", &[], |_, _| unreachable!());
            assert!(result.is_ok());
        }
    }
}

#[cfg(target_os = "linux")]
pub use linux::{Ctx, LinuxEvidence, SystemEvidence};

/// Test-only mock evidence sources, so check logic can be exercised without
/// touching real host/VM state.
#[cfg(test)]
pub mod testing {
    #[cfg(target_os = "linux")]
    pub use super::linux::testing::MockLinuxEvidence;
    #[cfg(target_os = "windows")]
    pub use super::windows::testing::MockWindowsEvidence;
}

#[cfg(target_os = "windows")]
mod windows {
    use super::CommandOutput;

    /// `Win32_ComputerSystem` fields relevant to VM detection.
    #[derive(Debug, Clone, Default)]
    pub struct ComputerSystemInfo {
        pub manufacturer: String,
        pub model: String,
    }

    /// `Win32_BIOS` fields relevant to VM detection.
    #[derive(Debug, Clone, Default)]
    pub struct BiosInfo {
        pub manufacturer: String,
        pub version: String,
        pub serial_number: String,
    }

    /// A registry hive to read from. Only `LocalMachine` exists because no
    /// current check needs HKCU/HKCR/etc.
    #[derive(Debug, Clone, Copy)]
    pub enum RegHive {
        LocalMachine,
    }

    /// Everything a Windows check can ask the host for. [`SystemEvidence`] is
    /// the real implementation; `MockWindowsEvidence` (test-only, see the
    /// `testing` module) fakes it so check logic can be tested without real
    /// host/VM state or a live WMI connection.
    pub trait WindowsEvidence {
        /// Queries `Win32_ComputerSystem` for manufacturer/model.
        fn wmi_computer_system(&self) -> anyhow::Result<ComputerSystemInfo>;
        /// Queries `Win32_BIOS` for manufacturer/version/serial number.
        fn wmi_bios(&self) -> anyhow::Result<BiosInfo>;
        /// Model strings of the host's disk drives, via `Win32_DiskDrive`.
        fn wmi_disk_models(&self) -> anyhow::Result<Vec<String>>;
        /// MAC addresses of the host's network adapters, via `Win32_NetworkAdapter`.
        fn wmi_mac_addresses(&self) -> anyhow::Result<Vec<String>>;
        /// Reads a single registry value at `hive`\\`path`\\`value`.
        fn registry_value(&self, hive: RegHive, path: &str, value: &str)
            -> std::io::Result<String>;
    }

    /// Handle passed to a Windows [`Check`](crate::check::Check)'s `run` fn.
    pub type Ctx<'a> = &'a dyn WindowsEvidence;

    /// Suppress "unused" warnings for the CommandOutput re-export path on this
    /// platform; kept for symmetry with the Linux side even though the current
    /// Windows checks don't need to shell out.
    #[allow(dead_code)]
    fn _unused(_: CommandOutput) {}

    pub struct SystemEvidence;

    impl WindowsEvidence for SystemEvidence {
        fn wmi_computer_system(&self) -> anyhow::Result<ComputerSystemInfo> {
            use serde::Deserialize;
            #[derive(Deserialize, Default)]
            struct Row {
                #[serde(rename = "Manufacturer")]
                manufacturer: String,
                #[serde(rename = "Model")]
                model: String,
            }
            let con = wmi::WMIConnection::new(wmi::COMLibrary::new()?)?;
            let rows: Vec<Row> =
                con.raw_query("SELECT Manufacturer, Model FROM Win32_ComputerSystem")?;
            let row = rows.into_iter().next().unwrap_or_default();
            Ok(ComputerSystemInfo {
                manufacturer: row.manufacturer,
                model: row.model,
            })
        }

        fn wmi_bios(&self) -> anyhow::Result<BiosInfo> {
            use serde::Deserialize;
            #[derive(Deserialize, Default)]
            struct Row {
                #[serde(rename = "Manufacturer")]
                manufacturer: String,
                #[serde(rename = "Version")]
                version: Option<String>,
                #[serde(rename = "SerialNumber")]
                serial_number: Option<String>,
            }
            let con = wmi::WMIConnection::new(wmi::COMLibrary::new()?)?;
            let rows: Vec<Row> =
                con.raw_query("SELECT Manufacturer, Version, SerialNumber FROM Win32_BIOS")?;
            let row = rows.into_iter().next().unwrap_or_default();
            Ok(BiosInfo {
                manufacturer: row.manufacturer,
                version: row.version.unwrap_or_default(),
                serial_number: row.serial_number.unwrap_or_default(),
            })
        }

        fn wmi_disk_models(&self) -> anyhow::Result<Vec<String>> {
            use serde::Deserialize;
            #[derive(Deserialize)]
            struct Row {
                #[serde(rename = "Model")]
                model: Option<String>,
            }
            let con = wmi::WMIConnection::new(wmi::COMLibrary::new()?)?;
            let rows: Vec<Row> = con.raw_query("SELECT Model FROM Win32_DiskDrive")?;
            Ok(rows.into_iter().filter_map(|r| r.model).collect())
        }

        fn wmi_mac_addresses(&self) -> anyhow::Result<Vec<String>> {
            use serde::Deserialize;
            #[derive(Deserialize)]
            struct Row {
                #[serde(rename = "MACAddress")]
                mac_address: Option<String>,
            }
            let con = wmi::WMIConnection::new(wmi::COMLibrary::new()?)?;
            let rows: Vec<Row> = con.raw_query(
                "SELECT MACAddress FROM Win32_NetworkAdapter WHERE MACAddress IS NOT NULL",
            )?;
            Ok(rows.into_iter().filter_map(|r| r.mac_address).collect())
        }

        fn registry_value(
            &self,
            hive: RegHive,
            path: &str,
            value: &str,
        ) -> std::io::Result<String> {
            use winreg::enums::HKEY_LOCAL_MACHINE;
            use winreg::RegKey;
            let root = match hive {
                RegHive::LocalMachine => RegKey::predef(HKEY_LOCAL_MACHINE),
            };
            let key = root.open_subkey(path)?;
            key.get_value(value)
        }
    }

    #[cfg(test)]
    pub mod testing {
        use super::{BiosInfo, ComputerSystemInfo, RegHive, WindowsEvidence};
        use std::collections::HashMap;

        #[derive(Default)]
        pub struct MockWindowsEvidence {
            computer_system: ComputerSystemInfo,
            bios: BiosInfo,
            disk_models: Vec<String>,
            macs: Vec<String>,
            registry_values: HashMap<String, String>,
        }

        fn registry_key(path: &str, value: &str) -> String {
            format!("{path}\\{value}")
        }

        impl MockWindowsEvidence {
            pub fn new() -> Self {
                Self::default()
            }

            pub fn with_computer_system(mut self, manufacturer: &str, model: &str) -> Self {
                self.computer_system = ComputerSystemInfo {
                    manufacturer: manufacturer.to_string(),
                    model: model.to_string(),
                };
                self
            }

            pub fn with_bios(mut self, manufacturer: &str, version: &str, serial: &str) -> Self {
                self.bios = BiosInfo {
                    manufacturer: manufacturer.to_string(),
                    version: version.to_string(),
                    serial_number: serial.to_string(),
                };
                self
            }

            pub fn with_disk_model(mut self, model: &str) -> Self {
                self.disk_models.push(model.to_string());
                self
            }

            pub fn with_mac(mut self, mac: &str) -> Self {
                self.macs.push(mac.to_string());
                self
            }

            pub fn with_registry_value(mut self, path: &str, value: &str, data: &str) -> Self {
                self.registry_values
                    .insert(registry_key(path, value), data.to_string());
                self
            }
        }

        impl WindowsEvidence for MockWindowsEvidence {
            fn wmi_computer_system(&self) -> anyhow::Result<ComputerSystemInfo> {
                Ok(self.computer_system.clone())
            }

            fn wmi_bios(&self) -> anyhow::Result<BiosInfo> {
                Ok(self.bios.clone())
            }

            fn wmi_disk_models(&self) -> anyhow::Result<Vec<String>> {
                Ok(self.disk_models.clone())
            }

            fn wmi_mac_addresses(&self) -> anyhow::Result<Vec<String>> {
                Ok(self.macs.clone())
            }

            fn registry_value(
                &self,
                _hive: RegHive,
                path: &str,
                value: &str,
            ) -> std::io::Result<String> {
                self.registry_values
                    .get(&registry_key(path, value))
                    .cloned()
                    .ok_or_else(|| {
                        std::io::Error::new(std::io::ErrorKind::NotFound, value.to_string())
                    })
            }
        }
    }
}

#[cfg(target_os = "windows")]
pub use windows::{BiosInfo, ComputerSystemInfo, Ctx, RegHive, SystemEvidence, WindowsEvidence};
