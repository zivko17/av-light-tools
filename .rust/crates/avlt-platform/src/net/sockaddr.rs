//! Conversions between Winsock socket addresses and `std::net` types.

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

use windows::Win32::Networking::WinSock::{
    AF_INET, AF_INET6, SOCKADDR, SOCKADDR_IN, SOCKADDR_IN6, SOCKADDR_INET,
};

/// SAFETY: `sa` must be null or point to a valid `SOCKADDR_IN` / `SOCKADDR_IN6`
/// (as announced by its `sa_family`).
pub unsafe fn sockaddr_ip(sa: *const SOCKADDR) -> Option<IpAddr> {
    if sa.is_null() {
        return None;
    }
    // SAFETY: every socket address starts with its family.
    let family = unsafe { (*sa).sa_family };
    if family == AF_INET {
        // SAFETY: the family says this is a SOCKADDR_IN.
        let v4 = unsafe { &*sa.cast::<SOCKADDR_IN>() };
        Some(IpAddr::V4(in_addr(v4)))
    } else if family == AF_INET6 {
        // SAFETY: the family says this is a SOCKADDR_IN6.
        let v6 = unsafe { &*sa.cast::<SOCKADDR_IN6>() };
        // SAFETY: both union views are plain bytes.
        Some(IpAddr::V6(Ipv6Addr::from(unsafe { v6.sin6_addr.u.Byte })))
    } else {
        None
    }
}

/// IPv4 address of a `SOCKADDR_INET`, if it is one.
pub fn inet_ipv4(sa: &SOCKADDR_INET) -> Option<Ipv4Addr> {
    // SAFETY: `si_family` overlays the family field of both variants.
    if unsafe { sa.si_family } != AF_INET {
        return None;
    }
    // SAFETY: the family says the IPv4 variant is active.
    Some(in_addr(unsafe { &sa.Ipv4 }))
}

fn in_addr(v4: &SOCKADDR_IN) -> Ipv4Addr {
    // SAFETY: all union views are plain integers; S_addr is in network byte order.
    Ipv4Addr::from(unsafe { v4.sin_addr.S_un.S_addr }.to_ne_bytes())
}

/// `SOCKADDR_IN` for an IPv4 address (port 0).
pub fn sockaddr_in(addr: Ipv4Addr) -> SOCKADDR_IN {
    let mut sa = SOCKADDR_IN {
        sin_family: AF_INET,
        ..Default::default()
    };
    sa.sin_addr.S_un.S_addr = u32::from_ne_bytes(addr.octets());
    sa
}
