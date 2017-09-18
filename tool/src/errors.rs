error_chain! {
    links {
        CaSync(::casync_http::Error, ::casync_http::ErrorKind);
    }

    foreign_links {
        Git(::git2::Error);
        Io(::std::io::Error);
        Reqwest(::reqwest::Error);
        Utf8(::std::string::FromUtf8Error);
        Xdg(::xdg::BaseDirectoriesError);
    }
}
