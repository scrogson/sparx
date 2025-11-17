rustler::atoms! {
    ok,
    error,
    eof,

    // HTTP methods
    get,
    post,
    put,
    patch,
    delete,
    head,
    options,

    // HTTP versions
    http_1_0,
    http_1_1,
    http_2,

    // WebSocket
    text,
    binary,
    ping,
    pong,
    close,
}
