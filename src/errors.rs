// Create the Error, ErrorKind, ResultExt, and Result types.
error_chain! {
    links { }
    foreign_links {
        Io(::std::io::Error);
    }
}
