//! Changing the IP configuration (netsh, as SetIP.ps1 and NetMaint), restarting an
//! adapter and flushing the DNS cache. Everything here changes the system.

use std::net::Ipv4Addr;
use std::path::PathBuf;
use std::time::Duration;

use avlt_core::net::{parse_cidr, prefix_to_mask};

use crate::admin::require_admin;
use crate::process::{capture, output};
use crate::{Error, Result};

/// Static IPv4 address (`a.b.c.d/p`) and optional gateway on `adapter`; DNS goes back to
/// DHCP, as SetIP.ps1. Requires administrator rights.
pub fn set_static(adapter: &str, cidr: &str, gateway: Option<&str>) -> Result<()> {
    let args = static_args(adapter, cidr, gateway)?;
    require_admin()?;
    let netsh = system_tool("netsh")?;
    let refs: Vec<&str> = args.iter().map(String::as_str).collect();
    capture(&netsh, &refs)?;
    // As SetIP.ps1: DNS back to automatic; a failure here does not undo the address.
    let _ = output(
        &netsh,
        &dns_dhcp_args(adapter).each_ref().map(String::as_str),
    );
    Ok(())
}

/// Address and DNS from DHCP on `adapter`. Requires administrator rights.
///
/// "DHCP is already enabled" is not an error.
pub fn set_dhcp(adapter: &str) -> Result<()> {
    require_admin()?;
    let netsh = system_tool("netsh")?;
    let addr = [
        "interface".to_string(),
        "ip".into(),
        "set".into(),
        "address".into(),
        format!("name={adapter}"),
        "source=dhcp".into(),
    ];
    let (code, text) = output(&netsh, &addr.each_ref().map(String::as_str))?;
    let _ = output(
        &netsh,
        &dns_dhcp_args(adapter).each_ref().map(String::as_str),
    );
    if code == 0 || crate::net::find_adapter(adapter).is_ok_and(|a| a.dhcp) {
        Ok(())
    } else {
        Err(Error::Other(format!("netsh failed: {}", text.trim())))
    }
}

/// Disables and re-enables `adapter` (2 s apart, as NetTools.ps1 `reset-nic`).
/// Requires administrator rights.
pub fn restart_adapter(adapter: &str) -> Result<()> {
    require_admin()?;
    let netsh = system_tool("netsh")?;
    let set = |state: &str| {
        capture(
            &netsh,
            &[
                "interface",
                "set",
                "interface",
                adapter,
                &format!("admin={state}"),
            ],
        )
    };
    set("disabled")?;
    std::thread::sleep(Duration::from_secs(2));
    set("enabled").map_err(|e| {
        Error::Other(format!(
            "adapter '{adapter}' was disabled but could not be re-enabled: {e}"
        ))
    })?;
    Ok(())
}

/// Clears the DNS resolver cache (`DnsFlushResolverCache`, then `ipconfig /flushdns`).
pub fn flush_dns() -> Result<()> {
    if imp::dns_flush_resolver_cache() {
        return Ok(());
    }
    capture(&system_tool("ipconfig")?, &["/flushdns"]).map(|_| ())
}

/// netsh arguments for a static address; validates the input first.
fn static_args(adapter: &str, cidr: &str, gateway: Option<&str>) -> Result<Vec<String>> {
    let (ip, prefix) = parse_cidr(cidr)?;
    let mut args = vec![
        "interface".to_string(),
        "ip".into(),
        "set".into(),
        "address".into(),
        format!("name={adapter}"),
        "static".into(),
        ip.to_string(),
        prefix_to_mask(prefix).to_string(),
    ];
    if let Some(gw) = gateway {
        let gw: Ipv4Addr = gw
            .trim()
            .parse()
            .map_err(|_| Error::InvalidInput(format!("invalid gateway '{gw}'")))?;
        args.push(gw.to_string());
    }
    Ok(args)
}

fn dns_dhcp_args(adapter: &str) -> [String; 6] {
    [
        "interface".into(),
        "ip".into(),
        "set".into(),
        "dns".into(),
        format!("name={adapter}"),
        "source=dhcp".into(),
    ]
}

/// `%SystemRoot%\System32\<name>.exe`, so a same-named program on PATH is never run.
fn system_tool(name: &str) -> Result<PathBuf> {
    if !cfg!(windows) {
        return Err(Error::Unsupported);
    }
    let root = std::env::var_os("SystemRoot").unwrap_or_else(|| "C:\\Windows".into());
    let path = PathBuf::from(root)
        .join("System32")
        .join(format!("{name}.exe"));
    if path.is_file() {
        Ok(path)
    } else {
        Err(Error::Other(format!("{} not found", path.display())))
    }
}

#[cfg(windows)]
mod imp {
    #[link(name = "dnsapi")]
    unsafe extern "system" {
        /// Undocumented but stable dnsapi export used by `ipconfig /flushdns`;
        /// returns non-zero on success.
        fn DnsFlushResolverCache() -> i32;
    }

    pub fn dns_flush_resolver_cache() -> bool {
        // SAFETY: takes no arguments and touches no caller memory.
        unsafe { DnsFlushResolverCache() != 0 }
    }
}

#[cfg(not(windows))]
mod imp {
    pub fn dns_flush_resolver_cache() -> bool {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn static_args_match_setip_ps1() {
        assert_eq!(
            static_args("Ethernet 2", "192.168.1.25/24", Some("192.168.1.1")).unwrap(),
            [
                "interface",
                "ip",
                "set",
                "address",
                "name=Ethernet 2",
                "static",
                "192.168.1.25",
                "255.255.255.0",
                "192.168.1.1"
            ]
        );
        assert_eq!(
            static_args("Ethernet", "10.0.0.5/16", None).unwrap().len(),
            8
        );
    }

    #[test]
    fn invalid_input_is_rejected_before_anything_runs() {
        for (cidr, gw) in [
            ("192.168.1.25", None),
            ("192.168.1.300/24", None),
            ("192.168.1.25/24", Some("nope")),
        ] {
            let err = set_static("avlt-test-no-such-adapter", cidr, gw).unwrap_err();
            assert!(
                matches!(err, Error::InvalidInput(_)),
                "{cidr} {gw:?}: {err:?}"
            );
        }
    }

    #[test]
    fn system_tools_resolve_on_windows() {
        if cfg!(windows) {
            assert!(system_tool("netsh").unwrap().is_file());
        } else {
            assert!(matches!(system_tool("netsh"), Err(Error::Unsupported)));
        }
    }
}
