use std::net::IpAddr;

#[test]
fn gateway_default_bind_address_is_loopback_only() {
    let addr = gateway::web::local_bind_addr(4096);

    assert_eq!(addr.port(), 4096);
    assert_eq!(addr.ip(), IpAddr::from([127, 0, 0, 1]));
    assert!(addr.ip().is_loopback());
    assert!(!addr.ip().is_unspecified());
}
