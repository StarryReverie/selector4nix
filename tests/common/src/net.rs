use std::net::TcpListener;

pub fn allocate_port() -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").expect("failed to bind ephemeral port");
    listener
        .local_addr()
        .expect("failed to get local address")
        .port()
}
