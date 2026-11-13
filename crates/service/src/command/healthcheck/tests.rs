use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

use rstest::rstest;

use super::target_host;

#[rstest]
#[case::absent(None, Some(IpAddr::V4(Ipv4Addr::LOCALHOST)))]
#[case::unspecified_v4(Some("0.0.0.0"), Some(IpAddr::V4(Ipv4Addr::LOCALHOST)))]
#[case::unspecified_v6(Some("::"), Some(IpAddr::V6(Ipv6Addr::LOCALHOST)))]
#[case::loopback_v4(Some("127.0.0.1"), Some(IpAddr::V4(Ipv4Addr::LOCALHOST)))]
#[case::loopback_v6(Some("::1"), Some(IpAddr::V6(Ipv6Addr::LOCALHOST)))]
#[case::specific_v4(Some("10.0.3.7"), Some(IpAddr::V4(Ipv4Addr::new(10, 0, 3, 7))))]
#[case::specific_v6(
    Some("2001:db8::7"),
    Some(IpAddr::V6(Ipv6Addr::new(0x2001, 0xdb8, 0, 0, 0, 0, 0, 7)))
)]
#[case::not_an_address(Some("localhost"), None)]
#[case::empty(Some(""), None)]
fn the_probe_targets_the_address_the_service_listens_on(
    #[case] host: Option<&str>,
    #[case] expected: Option<IpAddr>,
) {
    assert_eq!(target_host(host), expected);
}
