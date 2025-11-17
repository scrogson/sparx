rustler::atoms! {
    ok,
    error,
    eof,
    nil,

    // Error reasons
    not_found,
    timeout,
    invalid_request,
    server_error,
    already_started,
    not_started,
    connection_closed,

    // HTTP methods
    get,
    post,
    put,
    patch,
    delete,
    head,
    options,
    connect,
    trace,

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
