error_chain! {
    links {
        CaSync(::casync_http::Error, ::casync_http::ErrorKind);
    }

    foreign_links {
        Utf8(::std::string::FromUtf8Error);
        Io(::std::io::Error);
        Xdg(::xdg::BaseDirectoriesError);
    }
}
